use anchor_lang::solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program_option::COption,
    program_pack::Pack, system_program,
};
use anchor_lang::{
    prelude::*, AccountDeserialize, AccountSerialize, Discriminator, InstructionData, Space,
    ToAccountMetas,
};
use solana_program_test::{processor, BanksClientError, ProgramTest, ProgramTestContext};
use solana_sdk::{
    account::Account,
    instruction::{Instruction, InstructionError},
    signature::{Keypair, Signer},
    transaction::{Transaction, TransactionError},
};
use spl_token::state::{
    Account as SplTokenAccount, AccountState as SplAccountState, Mint as SplMint,
};
use spl_token_2022::{
    extension::{
        pausable::{PausableAccount, PausableConfig},
        permanent_delegate::PermanentDelegate,
        transfer_hook::{TransferHook, TransferHookAccount},
        BaseStateWithExtensionsMut, ExtensionType, StateWithExtensionsMut,
    },
    state::{
        Account as Token2022Account, AccountState as Token2022AccountState, Mint as Token2022Mint,
    },
};

use nest_core::{
    self, accounts as core_accounts, instruction as core_instruction, CollateralConfig, CoreError,
    MarketStateAccount, OraclePriceAccount, OracleSnapshot, Protocol, Vault,
};

const TEST_LAMPORTS: u64 = 10_000_000_000;
const COLLATERAL_DECIMALS: u8 = 6;
const NUSD_DECIMALS: u8 = 6;
const COLLATERAL_RAW: u64 = 100_000_000;
const MINT_AMOUNT: u64 = 1_000_000;
const PRICE_E8: u64 = 100_000_000;

struct Fixture {
    context: ProgramTestContext,
    owner: Keypair,
    protocol: Pubkey,
    collateral_config: Pubkey,
    vault: Pubkey,
    oracle: Pubkey,
    collateral_mint: Pubkey,
    collateral_vault: Pubkey,
    nusd_mint: Pubkey,
    owner_nusd_account: Pubkey,
}

impl Fixture {
    async fn new(vault_frozen: bool, mint_paused: bool, collateral_vault_raw: u64) -> Self {
        let owner = Keypair::new();
        let authority = Pubkey::new_unique();
        let issuer = Pubkey::new_unique();
        let collateral_config = Pubkey::new_unique();
        let collateral_mint = Pubkey::new_unique();
        let collateral_vault = Pubkey::new_unique();
        let insurance_collateral_vault = Pubkey::new_unique();
        let nusd_mint = Pubkey::new_unique();
        let owner_nusd_account = Pubkey::new_unique();
        let (protocol, protocol_bump) =
            Pubkey::find_program_address(&[b"protocol"], &nest_core::ID);
        let (vault, vault_bump) = Pubkey::find_program_address(
            &[
                b"vault",
                owner.pubkey().as_ref(),
                collateral_config.as_ref(),
            ],
            &nest_core::ID,
        );
        let (oracle, oracle_bump) =
            Pubkey::find_program_address(&[b"oracle", collateral_config.as_ref()], &nest_core::ID);

        let mut program_test = ProgramTest::new(
            "nest_core",
            nest_core::ID,
            processor!(process_nest_core_instruction),
        );
        program_test.add_program(
            "spl_token",
            spl_token::ID,
            processor!(spl_token::processor::Processor::process),
        );
        program_test.add_program(
            "spl_token_2022",
            spl_token_2022::ID,
            processor!(spl_token_2022::processor::Processor::process),
        );
        add_system_account(&mut program_test, owner.pubkey());

        let protocol_state = protocol_state(authority, nusd_mint, protocol_bump);
        let collateral_state = collateral_state(
            protocol,
            collateral_mint,
            collateral_vault,
            insurance_collateral_vault,
        );
        let vault_state = Vault {
            owner: owner.pubkey(),
            collateral_config,
            collateral_raw: COLLATERAL_RAW as u128,
            principal_debt: 0,
            accrued_fee: 0,
            last_accrual_ts: 0,
            bump: vault_bump,
        };
        let oracle_state = OracleSnapshot {
            protocol,
            collateral_config,
            xstock_usd: OraclePriceAccount {
                feed_id: [7; 32],
                price_e8: PRICE_E8,
                confidence_e8: 1,
                publish_time: 0,
            },
            reserved_underlying_usd: inactive_oracle_price(),
            reserved_redemption_rate: inactive_oracle_price(),
            reserved_market_state: MarketStateAccount::Regular,
            source_publish_time_us: 0,
            bump: oracle_bump,
        };

        program_test.add_account(
            protocol,
            anchor_account(&protocol_state, 8 + Protocol::INIT_SPACE, nest_core::ID),
        );
        program_test.add_account(
            collateral_config,
            anchor_account(
                &collateral_state,
                8 + CollateralConfig::INIT_SPACE,
                nest_core::ID,
            ),
        );
        program_test.add_account(
            vault,
            anchor_account(&vault_state, 8 + Vault::INIT_SPACE, nest_core::ID),
        );
        program_test.add_account(
            oracle,
            anchor_account(&oracle_state, 8 + OracleSnapshot::INIT_SPACE, nest_core::ID),
        );
        program_test.add_account(
            collateral_mint,
            token_2022_mint_account(issuer, mint_paused),
        );
        program_test.add_account(
            collateral_vault,
            token_2022_account(
                collateral_mint,
                protocol,
                collateral_vault_raw,
                vault_frozen,
            ),
        );
        program_test.add_account(
            insurance_collateral_vault,
            token_2022_account(collateral_mint, protocol, 0, false),
        );
        program_test.add_account(
            nusd_mint,
            spl_mint_account(Some(protocol), 0, NUSD_DECIMALS),
        );
        program_test.add_account(
            owner_nusd_account,
            spl_token_account(nusd_mint, owner.pubkey(), 0),
        );

        let mut context = program_test.start_with_context().await;
        let clock = context.banks_client.get_sysvar::<Clock>().await.unwrap();
        let mut oracle_account = context
            .banks_client
            .get_account(oracle)
            .await
            .unwrap()
            .unwrap();
        let mut oracle_state = decode_anchor_account::<OracleSnapshot>(&oracle_account);
        oracle_state.xstock_usd.publish_time = clock.unix_timestamp;
        oracle_state.source_publish_time_us = clock.unix_timestamp * 1_000_000;
        oracle_account.data =
            anchor_account(&oracle_state, 8 + OracleSnapshot::INIT_SPACE, nest_core::ID).data;
        context.set_account(&oracle, &oracle_account.into());

        Self {
            context,
            owner,
            protocol,
            collateral_config,
            vault,
            oracle,
            collateral_mint,
            collateral_vault,
            nusd_mint,
            owner_nusd_account,
        }
    }

    fn mint_instruction(&self) -> Instruction {
        Instruction {
            program_id: nest_core::ID,
            accounts: core_accounts::MintNusdWithOracle {
                protocol: self.protocol,
                collateral_config: self.collateral_config,
                vault: self.vault,
                oracle: self.oracle,
                collateral_mint: self.collateral_mint,
                collateral_vault: self.collateral_vault,
                nusd_mint: self.nusd_mint,
                owner_nusd_account: self.owner_nusd_account,
                owner: self.owner.pubkey(),
                collateral_token_program: spl_token_2022::ID,
                nusd_token_program: spl_token::ID,
            }
            .to_account_metas(None),
            data: core_instruction::MintNusdWithOracle {
                amount: MINT_AMOUNT,
            }
            .data(),
        }
    }

    fn coverage_breaker_instruction(&self) -> Instruction {
        Instruction {
            program_id: nest_core::ID,
            accounts: core_accounts::TripCollateralVaultCoverageBreaker {
                protocol: self.protocol,
                collateral_config: self.collateral_config,
                collateral_mint: self.collateral_mint,
                collateral_vault: self.collateral_vault,
                collateral_token_program: spl_token_2022::ID,
            }
            .to_account_metas(None),
            data: core_instruction::TripCollateralVaultCoverageBreaker {}.data(),
        }
    }
}

#[tokio::test]
async fn xstock_style_token_2022_collateral_can_still_mint_nusd() {
    let mut fixture = Fixture::new(false, false, COLLATERAL_RAW).await;
    let instruction = fixture.mint_instruction();

    send_transaction(&mut fixture.context, vec![instruction], &[&fixture.owner])
        .await
        .unwrap();

    let vault = read_anchor_account::<Vault>(&mut fixture.context, fixture.vault).await;
    let protocol = read_anchor_account::<Protocol>(&mut fixture.context, fixture.protocol).await;
    let collateral =
        read_anchor_account::<CollateralConfig>(&mut fixture.context, fixture.collateral_config)
            .await;
    let nusd_mint = read_spl_mint(&mut fixture.context, fixture.nusd_mint).await;
    let owner_nusd = read_spl_token_account(&mut fixture.context, fixture.owner_nusd_account).await;

    assert_eq!(vault.principal_debt, MINT_AMOUNT as u128);
    assert_eq!(protocol.total_debt, MINT_AMOUNT as u128);
    assert_eq!(collateral.total_debt, MINT_AMOUNT as u128);
    assert_eq!(nusd_mint.supply, MINT_AMOUNT);
    assert_eq!(owner_nusd.amount, MINT_AMOUNT);
}

#[tokio::test]
async fn frozen_shared_collateral_vault_blocks_new_debt_atomically() {
    let mut fixture = Fixture::new(true, false, COLLATERAL_RAW).await;
    let instruction = fixture.mint_instruction();

    let result = send_transaction(&mut fixture.context, vec![instruction], &[&fixture.owner]).await;
    assert_custom_error(
        result,
        anchor_lang::error::ERROR_CODE_OFFSET + CoreError::CollateralCustodyFrozen as u32,
    );
    assert_no_debt_or_nusd(&mut fixture).await;
}

#[tokio::test]
async fn mint_wide_token_2022_pause_blocks_new_debt_atomically() {
    let mut fixture = Fixture::new(false, true, COLLATERAL_RAW).await;
    let instruction = fixture.mint_instruction();

    let result = send_transaction(&mut fixture.context, vec![instruction], &[&fixture.owner]).await;
    assert!(result.is_err());
    assert_no_debt_or_nusd(&mut fixture).await;
}

#[tokio::test]
async fn frozen_vault_can_latch_existing_market_breaker() {
    let mut fixture = Fixture::new(true, false, COLLATERAL_RAW).await;
    let instruction = fixture.coverage_breaker_instruction();

    send_transaction(&mut fixture.context, vec![instruction], &[])
        .await
        .unwrap();

    let collateral =
        read_anchor_account::<CollateralConfig>(&mut fixture.context, fixture.collateral_config)
            .await;
    assert!(collateral.deposits_paused);
    assert!(collateral.borrows_paused);
    assert!(collateral.withdraws_paused);
}

#[tokio::test]
async fn healthy_vault_cannot_latch_coverage_breaker() {
    let mut fixture = Fixture::new(false, false, COLLATERAL_RAW).await;
    let instruction = fixture.coverage_breaker_instruction();

    let result = send_transaction(&mut fixture.context, vec![instruction], &[]).await;
    assert_custom_error(
        result,
        anchor_lang::error::ERROR_CODE_OFFSET + CoreError::InvalidParameter as u32,
    );
}

#[tokio::test]
async fn permanent_delegate_shortfall_blocks_new_debt_and_can_latch_breaker() {
    let mut fixture = Fixture::new(false, false, COLLATERAL_RAW - 1).await;
    let mint_instruction = fixture.mint_instruction();

    let result = send_transaction(
        &mut fixture.context,
        vec![mint_instruction],
        &[&fixture.owner],
    )
    .await;
    assert_custom_error(
        result,
        anchor_lang::error::ERROR_CODE_OFFSET + CoreError::InvalidParameter as u32,
    );
    assert_no_debt_or_nusd(&mut fixture).await;

    let breaker_instruction = fixture.coverage_breaker_instruction();
    send_transaction(&mut fixture.context, vec![breaker_instruction], &[])
        .await
        .unwrap();

    let collateral =
        read_anchor_account::<CollateralConfig>(&mut fixture.context, fixture.collateral_config)
            .await;
    assert!(collateral.deposits_paused);
    assert!(collateral.borrows_paused);
    assert!(collateral.withdraws_paused);
}

async fn assert_no_debt_or_nusd(fixture: &mut Fixture) {
    let vault = read_anchor_account::<Vault>(&mut fixture.context, fixture.vault).await;
    let protocol = read_anchor_account::<Protocol>(&mut fixture.context, fixture.protocol).await;
    let collateral =
        read_anchor_account::<CollateralConfig>(&mut fixture.context, fixture.collateral_config)
            .await;
    let nusd_mint = read_spl_mint(&mut fixture.context, fixture.nusd_mint).await;
    let owner_nusd = read_spl_token_account(&mut fixture.context, fixture.owner_nusd_account).await;

    assert_eq!(vault.principal_debt, 0);
    assert_eq!(protocol.total_debt, 0);
    assert_eq!(collateral.total_debt, 0);
    assert_eq!(nusd_mint.supply, 0);
    assert_eq!(owner_nusd.amount, 0);
}

async fn send_transaction(
    context: &mut ProgramTestContext,
    instructions: Vec<Instruction>,
    extra_signers: &[&Keypair],
) -> std::result::Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let mut transaction = Transaction::new_with_payer(&instructions, Some(&context.payer.pubkey()));
    let mut signers: Vec<&dyn Signer> = vec![&context.payer];
    signers.extend(
        extra_signers
            .iter()
            .copied()
            .map(|signer| signer as &dyn Signer),
    );
    transaction.sign(&signers, blockhash);
    context.banks_client.process_transaction(transaction).await
}

fn assert_custom_error(result: std::result::Result<(), BanksClientError>, expected: u32) {
    match result.unwrap_err() {
        BanksClientError::TransactionError(TransactionError::InstructionError(
            _,
            InstructionError::Custom(actual),
        )) => assert_eq!(actual, expected),
        error => panic!("expected custom program error {expected}, got {error:?}"),
    }
}

fn process_nest_core_instruction<'a, 'b, 'c, 'd>(
    program_id: &'a Pubkey,
    accounts: &'b [AccountInfo<'c>],
    instruction_data: &'d [u8],
) -> ProgramResult {
    let anchor_accounts: &'c [AccountInfo<'c>] = unsafe { std::mem::transmute(accounts) };
    nest_core::entry(program_id, anchor_accounts, instruction_data)
}

fn add_system_account(program_test: &mut ProgramTest, key: Pubkey) {
    program_test.add_account(
        key,
        Account {
            lamports: TEST_LAMPORTS,
            data: Vec::new(),
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
        },
    );
}

fn anchor_account<T: AccountSerialize + Discriminator>(
    value: &T,
    size: usize,
    owner: Pubkey,
) -> Account {
    let mut data = vec![0; size];
    value.try_serialize(&mut data.as_mut_slice()).unwrap();
    Account {
        lamports: TEST_LAMPORTS,
        data,
        owner,
        executable: false,
        rent_epoch: 0,
    }
}

fn token_2022_mint_account(issuer: Pubkey, paused: bool) -> Account {
    let extension_types = [
        ExtensionType::PermanentDelegate,
        ExtensionType::TransferHook,
        ExtensionType::Pausable,
    ];
    let mint_len =
        ExtensionType::try_calculate_account_len::<Token2022Mint>(&extension_types).unwrap();
    let mut data = vec![0; mint_len];
    let mut mint =
        StateWithExtensionsMut::<Token2022Mint>::unpack_uninitialized(&mut data).unwrap();
    mint.init_extension::<PermanentDelegate>(true)
        .unwrap()
        .delegate = Some(issuer).try_into().unwrap();
    mint.init_extension::<TransferHook>(true)
        .unwrap()
        .program_id = Some(Pubkey::new_unique()).try_into().unwrap();
    let pausable = mint.init_extension::<PausableConfig>(true).unwrap();
    pausable.authority = Some(issuer).try_into().unwrap();
    pausable.paused = paused.into();
    mint.base = Token2022Mint {
        mint_authority: COption::Some(issuer),
        supply: COLLATERAL_RAW,
        decimals: COLLATERAL_DECIMALS,
        is_initialized: true,
        freeze_authority: COption::Some(issuer),
    };
    mint.pack_base();
    mint.init_account_type().unwrap();
    drop(mint);
    Account {
        lamports: TEST_LAMPORTS,
        data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn token_2022_account(mint: Pubkey, authority: Pubkey, amount: u64, frozen: bool) -> Account {
    let extension_types = [
        ExtensionType::TransferHookAccount,
        ExtensionType::PausableAccount,
    ];
    let account_len =
        ExtensionType::try_calculate_account_len::<Token2022Account>(&extension_types).unwrap();
    let mut data = vec![0; account_len];
    let mut account =
        StateWithExtensionsMut::<Token2022Account>::unpack_uninitialized(&mut data).unwrap();
    account.init_extension::<TransferHookAccount>(true).unwrap();
    account.init_extension::<PausableAccount>(true).unwrap();
    account.base = Token2022Account {
        mint,
        owner: authority,
        amount,
        delegate: COption::None,
        state: if frozen {
            Token2022AccountState::Frozen
        } else {
            Token2022AccountState::Initialized
        },
        is_native: COption::None,
        delegated_amount: 0,
        close_authority: COption::None,
    };
    account.pack_base();
    account.init_account_type().unwrap();
    drop(account);
    Account {
        lamports: TEST_LAMPORTS,
        data,
        owner: spl_token_2022::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn spl_mint_account(authority: Option<Pubkey>, supply: u64, decimals: u8) -> Account {
    let mut data = vec![0; SplMint::LEN];
    SplMint::pack(
        SplMint {
            mint_authority: authority.map(COption::Some).unwrap_or(COption::None),
            supply,
            decimals,
            is_initialized: true,
            freeze_authority: COption::None,
        },
        &mut data,
    )
    .unwrap();
    Account {
        lamports: TEST_LAMPORTS,
        data,
        owner: spl_token::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn spl_token_account(mint: Pubkey, authority: Pubkey, amount: u64) -> Account {
    let mut data = vec![0; SplTokenAccount::LEN];
    SplTokenAccount::pack(
        SplTokenAccount {
            mint,
            owner: authority,
            amount,
            delegate: COption::None,
            state: SplAccountState::Initialized,
            is_native: COption::None,
            delegated_amount: 0,
            close_authority: COption::None,
        },
        &mut data,
    )
    .unwrap();
    Account {
        lamports: TEST_LAMPORTS,
        data,
        owner: spl_token::ID,
        executable: false,
        rent_epoch: 0,
    }
}

fn protocol_state(authority: Pubkey, nusd_mint: Pubkey, bump: u8) -> Protocol {
    Protocol {
        authority,
        liquidation_authority: Pubkey::default(),
        nusd_mint,
        usdc_mint: Pubkey::default(),
        psm_usdc_vault: Pubkey::default(),
        insurance_nusd_vault: Pubkey::default(),
        staker_revenue_nusd_vault: Pubkey::default(),
        staker_revenue_authority: Pubkey::default(),
        total_debt: 0,
        total_uncollected_fees: 0,
        realized_revenue_for_stakers: 0,
        insurance_fund_nusd: 0,
        bad_debt_nusd: 0,
        psm_usdc_liabilities: 0,
        psm_nusd_supply: 0,
        psm_idle_usdc: 0,
        psm_kamino_deployed_usdc: 0,
        psm_kamino_collateral_vault: Pubkey::default(),
        psm_cap: 0,
        protocol_debt_cap: 1_000_000_000_000,
        stability_fee_apr_bps: 0,
        psm_swap_in_fee_bps: 0,
        psm_swap_out_fee_bps: 0,
        kamino_program_id: Pubkey::default(),
        insurance_target_bps: 0,
        insurance_fee_share_bps: 0,
        staker_revenue_accounting_initialized: true,
        paused: false,
        bump,
        protocol_revenue_nusd_vault: Pubkey::default(),
        realized_revenue_for_protocol: 0,
        staker_target_revenue_due: 0,
        staker_target_last_accrual_ts: 0,
        staker_target_apr_bps: 0,
        staker_capacity_kamino_apr_bps: 0,
        pending_liquidation_principal: 0,
        pending_liquidation_fees: 0,
        psm_outflow_window_start_ts: 0,
        psm_outflow_window_usdc: 0,
        psm_outflow_limit_usdc: 0,
        psm_outflow_limit_bps: 0,
        psm_outflow_window_seconds: 0,
        psm_outflow_circuit_breaker_enabled: false,
        psm_outflow_window_basis_usdc: 0,
        buyback_authority: Pubkey::default(),
        buyback_usdc_account: Pubkey::default(),
    }
}

fn collateral_state(
    protocol: Pubkey,
    collateral_mint: Pubkey,
    collateral_vault: Pubkey,
    insurance_collateral_vault: Pubkey,
) -> CollateralConfig {
    CollateralConfig {
        protocol,
        collateral_mint,
        collateral_vault,
        insurance_collateral_vault,
        token_program: spl_token_2022::ID,
        symbol: [0; 16],
        collateral_decimals: COLLATERAL_DECIMALS,
        xstock_usd_feed_id: [7; 32],
        reserved_underlying_usd_feed_id: [0; 32],
        reserved_redemption_rate_feed_id: [0; 32],
        borrow_ltv_bps: 5_000,
        liquidation_threshold_bps: 6_000,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        max_confidence_bps: 100,
        max_staleness_seconds: 120,
        reserved_closed_market_max_staleness_seconds: 0,
        reserved_underlying_closed_market_max_staleness_seconds: 0,
        reserved_closed_market_haircut_bps: 0,
        per_vault_debt_cap: 1_000_000_000_000,
        protocol_debt_cap: 1_000_000_000_000,
        deposit_cap_raw: 1_000_000_000_000,
        total_debt: 0,
        total_deposits_raw: COLLATERAL_RAW as u128,
        deposits_paused: false,
        borrows_paused: false,
        withdraws_paused: false,
        bump: 0,
    }
}

fn inactive_oracle_price() -> OraclePriceAccount {
    OraclePriceAccount {
        feed_id: [0; 32],
        price_e8: 0,
        confidence_e8: 0,
        publish_time: 0,
    }
}

async fn read_anchor_account<T: AccountDeserialize>(
    context: &mut ProgramTestContext,
    key: Pubkey,
) -> T {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();
    decode_anchor_account(&account)
}

fn decode_anchor_account<T: AccountDeserialize>(account: &Account) -> T {
    let mut data = account.data.as_slice();
    T::try_deserialize(&mut data).unwrap()
}

async fn read_spl_mint(context: &mut ProgramTestContext, key: Pubkey) -> SplMint {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();
    SplMint::unpack(&account.data).unwrap()
}

async fn read_spl_token_account(context: &mut ProgramTestContext, key: Pubkey) -> SplTokenAccount {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();
    SplTokenAccount::unpack(&account.data).unwrap()
}
