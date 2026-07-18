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
    instruction::Instruction,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token::state::{
    Account as SplTokenAccount, AccountState as SplAccountState, Mint as SplMint,
};

use nest_core::Protocol;
use nest_stake::{self, accounts as stake_accounts, instruction as stake_instruction, StakingState};

const TEST_LAMPORTS: u64 = 10_000_000_000;
const STAKED_RAW: u64 = 1_000_000_000;
const STABLECOIN_DECIMALS: u8 = 6;

struct Fixture {
    context: ProgramTestContext,
    owner: Keypair,
    protocol: Pubkey,
    staking_state: Pubkey,
    nusd_mint: Pubkey,
    snusd_mint: Pubkey,
    staking_nusd_vault: Pubkey,
    owner_nusd_account: Pubkey,
    owner_snusd_account: Pubkey,
}

impl Fixture {
    async fn new(target_apr_bps: u64) -> Self {
        let owner = Keypair::new();
        let authority = Keypair::new();
        let nusd_mint = Pubkey::new_unique();
        let snusd_mint = Pubkey::new_unique();
        let staking_nusd_vault = Pubkey::new_unique();
        let owner_nusd_account = Pubkey::new_unique();
        let owner_snusd_account = Pubkey::new_unique();
        let (protocol, protocol_bump) =
            Pubkey::find_program_address(&[b"protocol"], &nest_core::ID);
        let (staking_state, staking_bump) =
            Pubkey::find_program_address(&[b"staking"], &nest_stake::ID);

        let mut program_test = ProgramTest::new(
            "nest_stake",
            nest_stake::ID,
            processor!(process_nest_stake_instruction),
        );
        program_test.add_program(
            "nest_core",
            nest_core::ID,
            processor!(process_nest_core_instruction),
        );
        program_test.add_program(
            "spl_token",
            spl_token::ID,
            processor!(spl_token::processor::Processor::process),
        );
        add_system_account(&mut program_test, owner.pubkey());
        add_system_account(&mut program_test, authority.pubkey());
        program_test.add_account(
            protocol,
            anchor_account(
                &protocol_state(
                    authority.pubkey(),
                    nusd_mint,
                    staking_state,
                    target_apr_bps,
                    protocol_bump,
                ),
                8 + Protocol::INIT_SPACE,
                nest_core::ID,
            ),
        );
        program_test.add_account(
            staking_state,
            anchor_account(
                &staking_state_account(
                    authority.pubkey(),
                    nusd_mint,
                    snusd_mint,
                    staking_nusd_vault,
                    STAKED_RAW,
                    false,
                    staking_bump,
                ),
                8 + StakingState::INIT_SPACE,
                nest_stake::ID,
            ),
        );
        program_test.add_account(
            nusd_mint,
            spl_mint_account(Some(protocol), STAKED_RAW, STABLECOIN_DECIMALS),
        );
        program_test.add_account(
            snusd_mint,
            spl_mint_account(Some(staking_state), STAKED_RAW, STABLECOIN_DECIMALS),
        );
        program_test.add_account(
            staking_nusd_vault,
            spl_token_account(nusd_mint, staking_state, STAKED_RAW),
        );
        program_test.add_account(
            owner_nusd_account,
            spl_token_account(nusd_mint, owner.pubkey(), 0),
        );
        program_test.add_account(
            owner_snusd_account,
            spl_token_account(snusd_mint, owner.pubkey(), STAKED_RAW),
        );

        Self {
            context: program_test.start_with_context().await,
            owner,
            protocol,
            staking_state,
            nusd_mint,
            snusd_mint,
            staking_nusd_vault,
            owner_nusd_account,
            owner_snusd_account,
        }
    }

    fn unstake_instruction(&self, shares: u64, min_nusd_out: u64) -> Instruction {
        Instruction {
            program_id: nest_stake::ID,
            accounts: stake_accounts::Unstake {
                staking_state: self.staking_state,
                protocol: self.protocol,
                nest_core_program: nest_core::ID,
                nusd_mint: self.nusd_mint,
                snusd_mint: self.snusd_mint,
                owner_snusd_account: self.owner_snusd_account,
                staking_nusd_vault: self.staking_nusd_vault,
                owner_nusd_account: self.owner_nusd_account,
                owner: self.owner.pubkey(),
                nusd_token_program: spl_token::ID,
                snusd_token_program: spl_token::ID,
            }
            .to_account_metas(None),
            data: stake_instruction::Unstake {
                shares,
                min_nusd_out,
            }
            .data(),
        }
    }

}

#[tokio::test]
async fn instant_unstake_burns_one_day_at_current_target_apr() {
    let mut fixture = Fixture::new(600).await;
    let instruction = fixture.unstake_instruction(STAKED_RAW, 999_835_616);

    send_transaction(&mut fixture.context, instruction, &fixture.owner)
        .await
        .unwrap();

    assert_settlement(&mut fixture, 999_835_616, 999_835_616).await;
}

#[tokio::test]
async fn instant_unstake_fee_changes_with_protocol_target_apr() {
    let mut fixture = Fixture::new(1_200).await;
    let instruction = fixture.unstake_instruction(STAKED_RAW, 999_671_232);

    send_transaction(&mut fixture.context, instruction, &fixture.owner)
        .await
        .unwrap();

    assert_settlement(&mut fixture, 999_671_232, 999_671_232).await;
}

#[tokio::test]
async fn minimum_output_failure_is_atomic() {
    let mut fixture = Fixture::new(600).await;
    let instruction = fixture.unstake_instruction(STAKED_RAW, 999_835_617);

    assert!(send_transaction(&mut fixture.context, instruction, &fixture.owner)
        .await
        .is_err());

    let staking_state =
        read_anchor_account::<StakingState>(&mut fixture.context, fixture.staking_state).await;
    let nusd_mint = read_spl_mint(&mut fixture.context, fixture.nusd_mint).await;
    let snusd_mint = read_spl_mint(&mut fixture.context, fixture.snusd_mint).await;
    let staking_vault =
        read_spl_token_account(&mut fixture.context, fixture.staking_nusd_vault).await;
    let owner_nusd =
        read_spl_token_account(&mut fixture.context, fixture.owner_nusd_account).await;
    let owner_snusd =
        read_spl_token_account(&mut fixture.context, fixture.owner_snusd_account).await;

    assert_eq!(staking_state.total_shares, STAKED_RAW as u128);
    assert_eq!(staking_state.total_user_shares, STAKED_RAW as u128);
    assert_eq!(staking_state.staking_vault_nusd, STAKED_RAW as u128);
    assert_eq!(nusd_mint.supply, STAKED_RAW);
    assert_eq!(snusd_mint.supply, STAKED_RAW);
    assert_eq!(staking_vault.amount, STAKED_RAW);
    assert_eq!(owner_nusd.amount, 0);
    assert_eq!(owner_snusd.amount, STAKED_RAW);
}

#[tokio::test]
async fn partial_unstake_cannot_redeem_other_users_shares() {
    let mut fixture = Fixture::new(600).await;
    let shares = 250_000_000;
    let expected_fee = 41_096;
    let expected_net = 249_958_904;
    let instruction = fixture.unstake_instruction(shares, expected_net);

    send_transaction(&mut fixture.context, instruction, &fixture.owner)
        .await
        .unwrap();

    let staking_state =
        read_anchor_account::<StakingState>(&mut fixture.context, fixture.staking_state).await;
    let nusd_mint = read_spl_mint(&mut fixture.context, fixture.nusd_mint).await;
    let snusd_mint = read_spl_mint(&mut fixture.context, fixture.snusd_mint).await;
    let staking_vault =
        read_spl_token_account(&mut fixture.context, fixture.staking_nusd_vault).await;
    let owner_nusd =
        read_spl_token_account(&mut fixture.context, fixture.owner_nusd_account).await;
    let owner_snusd =
        read_spl_token_account(&mut fixture.context, fixture.owner_snusd_account).await;

    assert_eq!(staking_state.total_shares, (STAKED_RAW - shares) as u128);
    assert_eq!(
        staking_state.total_user_shares,
        (STAKED_RAW - shares) as u128
    );
    assert_eq!(
        staking_state.staking_vault_nusd,
        (STAKED_RAW - shares) as u128
    );
    assert_eq!(nusd_mint.supply, STAKED_RAW - expected_fee);
    assert_eq!(snusd_mint.supply, STAKED_RAW - shares);
    assert_eq!(staking_vault.amount, STAKED_RAW - shares);
    assert_eq!(owner_nusd.amount, expected_net);
    assert_eq!(owner_snusd.amount, STAKED_RAW - shares);
}

async fn assert_settlement(fixture: &mut Fixture, expected_nusd: u64, expected_nusd_supply: u64) {
    let staking_state =
        read_anchor_account::<StakingState>(&mut fixture.context, fixture.staking_state).await;
    let nusd_mint = read_spl_mint(&mut fixture.context, fixture.nusd_mint).await;
    let snusd_mint = read_spl_mint(&mut fixture.context, fixture.snusd_mint).await;
    let staking_vault =
        read_spl_token_account(&mut fixture.context, fixture.staking_nusd_vault).await;
    let owner_nusd =
        read_spl_token_account(&mut fixture.context, fixture.owner_nusd_account).await;
    let owner_snusd =
        read_spl_token_account(&mut fixture.context, fixture.owner_snusd_account).await;

    assert_eq!(staking_state.total_shares, 0);
    assert_eq!(staking_state.total_user_shares, 0);
    assert_eq!(staking_state.staking_vault_nusd, 0);
    assert_eq!(nusd_mint.supply, expected_nusd_supply);
    assert_eq!(snusd_mint.supply, 0);
    assert_eq!(staking_vault.amount, 0);
    assert_eq!(owner_nusd.amount, expected_nusd);
    assert_eq!(owner_snusd.amount, 0);
}

async fn send_transaction(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    owner: &Keypair,
) -> std::result::Result<(), BanksClientError> {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let mut transaction =
        Transaction::new_with_payer(&[instruction], Some(&context.payer.pubkey()));
    transaction.sign(&[&context.payer, owner], blockhash);
    context.banks_client.process_transaction(transaction).await
}

fn process_nest_stake_instruction<'a, 'b, 'c, 'd>(
    program_id: &'a Pubkey,
    accounts: &'b [AccountInfo<'c>],
    instruction_data: &'d [u8],
) -> ProgramResult {
    let anchor_accounts: &'c [AccountInfo<'c>] = unsafe { std::mem::transmute(accounts) };
    nest_stake::entry(program_id, anchor_accounts, instruction_data)
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

fn staking_state_account(
    authority: Pubkey,
    nusd_mint: Pubkey,
    snusd_mint: Pubkey,
    staking_nusd_vault: Pubkey,
    total_user_shares: u64,
    paused: bool,
    bump: u8,
) -> StakingState {
    StakingState {
        authority,
        nusd_mint,
        snusd_mint,
        staking_nusd_vault,
        revenue_nusd_vault: Pubkey::new_unique(),
        revenue_baseline_nusd: 0,
        total_shares: STAKED_RAW as u128,
        total_user_shares: total_user_shares as u128,
        staking_vault_nusd: STAKED_RAW as u128,
        realized_loss_nusd: 0,
        unvested_revenue: 0,
        reserved_pending_claims: 0,
        vesting_start_ts: 0,
        vesting_end_ts: 0,
        last_vesting_sync_ts: 0,
        cooldown_seconds: nest_domain::DEFAULT_COOLDOWN_SECONDS,
        revenue_vesting_seconds: nest_domain::DEFAULT_REVENUE_VESTING_SECONDS,
        paused,
        bump,
    }
}

fn protocol_state(
    authority: Pubkey,
    nusd_mint: Pubkey,
    staking_state: Pubkey,
    target_apr_bps: u64,
    bump: u8,
) -> Protocol {
    Protocol {
        authority,
        liquidation_authority: Pubkey::default(),
        nusd_mint,
        usdc_mint: Pubkey::default(),
        psm_usdc_vault: Pubkey::default(),
        insurance_nusd_vault: Pubkey::default(),
        staker_revenue_nusd_vault: Pubkey::default(),
        staker_revenue_authority: staking_state,
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
        protocol_debt_cap: 0,
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
        staker_target_apr_bps: target_apr_bps,
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

async fn read_spl_token_account(
    context: &mut ProgramTestContext,
    key: Pubkey,
) -> SplTokenAccount {
    let account = context
        .banks_client
        .get_account(key)
        .await
        .unwrap()
        .unwrap();
    SplTokenAccount::unpack(&account.data).unwrap()
}
