use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_spl::token::ID as SPL_TOKEN_PROGRAM_ID;
use anchor_spl::token_interface::{
    self, BurnChecked, Mint, MintToChecked, TokenAccount, TokenInterface, TransferChecked,
};
use nest_domain as domain;

declare_id!("EdYg6JsyntWpf3WGofNWKzBYnQPEvWFZSzUNim3PLbhB");

const NEST_CORE_PROGRAM_ID: Pubkey = pubkey!("HxbLPNuQD7KKDVQoSQgY1cLMLrsaoseT65Xoczh7zHQW");
const STABLECOIN_DECIMALS: u8 = 6;
const CORE_PROTOCOL_DISCRIMINATOR: [u8; 8] = [45, 39, 101, 43, 115, 72, 131, 40];
const CORE_PROTOCOL_AUTHORITY_OFFSET: usize = 8;
const CORE_PROTOCOL_NUSD_MINT_OFFSET: usize = 72;
const CORE_PROTOCOL_TOTAL_DEBT_OFFSET: usize = 264;
const CORE_PROTOCOL_INSURANCE_FUND_NUSD_OFFSET: usize = 312;
const CORE_PROTOCOL_BAD_DEBT_NUSD_OFFSET: usize = 328;
const CORE_PROTOCOL_PSM_IDLE_USDC_OFFSET: usize = 376;
const CORE_PROTOCOL_PSM_KAMINO_DEPLOYED_USDC_OFFSET: usize = 392;
const CORE_PROTOCOL_STABILITY_FEE_APR_BPS_OFFSET: usize = 472;
const CORE_PROTOCOL_INSURANCE_TARGET_BPS_OFFSET: usize = 516;
const CORE_PROTOCOL_INSURANCE_FEE_SHARE_BPS_OFFSET: usize = 518;
const CORE_PROTOCOL_STAKER_TARGET_APR_BPS_OFFSET: usize = 595;
const CORE_PROTOCOL_STAKER_CAPACITY_KAMINO_APR_BPS_OFFSET: usize = 603;

mod instructions;

#[program]
pub mod nest_stake {
    use super::*;

    pub fn initialize_staking(
        ctx: Context<InitializeStaking>,
        params: InitializeStakingParams,
    ) -> Result<()> {
        instructions::initialize_staking(ctx, params)
    }

    pub fn stake(ctx: Context<Stake>, amount: u64, min_shares_out: u64) -> Result<()> {
        instructions::stake(ctx, amount, min_shares_out)
    }

    pub fn harvest(ctx: Context<Harvest>, amount: u64) -> Result<()> {
        instructions::harvest(ctx, amount)
    }

    pub fn request_unstake(ctx: Context<RequestUnstake>, shares: u64) -> Result<()> {
        instructions::request_unstake(ctx, shares)
    }

    pub fn complete_unstake(ctx: Context<CompleteUnstake>) -> Result<()> {
        instructions::complete_unstake(ctx)
    }

    pub fn cancel_expired_unstake(ctx: Context<CancelExpiredUnstake>) -> Result<()> {
        instructions::cancel_expired_unstake(ctx)
    }

    pub fn realize_loss(ctx: Context<RealizeLoss>, amount: u64) -> Result<()> {
        instructions::realize_loss(ctx, amount)
    }

    pub fn set_paused(ctx: Context<MutateStake>, paused: bool) -> Result<()> {
        instructions::set_paused(ctx, paused)
    }

    pub fn set_staking_authority(ctx: Context<MutateStake>, authority: Pubkey) -> Result<()> {
        instructions::set_staking_authority(ctx, authority)
    }
}

include!("accounts.rs");
include!("state.rs");
include!("params.rs");
include!("errors.rs");
include!("helpers.rs");

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(AnchorSerialize)]
    struct ProtocolLayout {
        authority: Pubkey,
        liquidation_authority: Pubkey,
        nusd_mint: Pubkey,
        usdc_mint: Pubkey,
        psm_usdc_vault: Pubkey,
        insurance_nusd_vault: Pubkey,
        staker_revenue_nusd_vault: Pubkey,
        staker_revenue_authority: Pubkey,
        total_debt: u128,
        total_uncollected_fees: u128,
        realized_revenue_for_stakers: u128,
        insurance_fund_nusd: u128,
        bad_debt_nusd: u128,
        psm_usdc_liabilities: u128,
        psm_nusd_supply: u128,
        psm_idle_usdc: u128,
        psm_kamino_deployed_usdc: u128,
        psm_kamino_collateral_vault: Pubkey,
        psm_cap: u128,
        protocol_debt_cap: u128,
        stability_fee_apr_bps: u64,
        psm_swap_in_fee_bps: u16,
        psm_swap_out_fee_bps: u16,
        kamino_program_id: Pubkey,
        insurance_target_bps: u16,
        insurance_fee_share_bps: u16,
        staker_revenue_accounting_initialized: bool,
        paused: bool,
        bump: u8,
        protocol_revenue_nusd_vault: Pubkey,
        realized_revenue_for_protocol: u128,
        staker_target_revenue_due: u128,
        staker_target_last_accrual_ts: i64,
        staker_target_apr_bps: u64,
        staker_capacity_kamino_apr_bps: u64,
    }

    #[test]
    fn core_protocol_reader_offsets_match_serialized_layout() {
        let authority = Pubkey::new_from_array([1; 32]);
        let nusd_mint = Pubkey::new_from_array([3; 32]);
        let total_debt = 0x0102_0304_0506_0708_1112_1314_1516_1718_u128;
        let insurance_fund_nusd = 0x2122_2324_2526_2728_3132_3334_3536_3738_u128;
        let psm_idle_usdc = 0x4142_4344_4546_4748_5152_5354_5556_5758_u128;
        let psm_kamino_deployed_usdc = 0x6162_6364_6566_6768_7172_7374_7576_7778_u128;
        let stability_fee_apr_bps = 123_u64;
        let insurance_target_bps = 456_u16;
        let insurance_fee_share_bps = 789_u16;
        let staker_target_apr_bps = 1_234_u64;
        let staker_capacity_kamino_apr_bps = 5_678_u64;
        let protocol = ProtocolLayout {
            authority,
            liquidation_authority: Pubkey::new_from_array([2; 32]),
            nusd_mint,
            usdc_mint: Pubkey::new_from_array([4; 32]),
            psm_usdc_vault: Pubkey::new_from_array([5; 32]),
            insurance_nusd_vault: Pubkey::new_from_array([6; 32]),
            staker_revenue_nusd_vault: Pubkey::new_from_array([7; 32]),
            staker_revenue_authority: Pubkey::new_from_array([8; 32]),
            total_debt,
            total_uncollected_fees: 9,
            realized_revenue_for_stakers: 10,
            insurance_fund_nusd,
            bad_debt_nusd: 12,
            psm_usdc_liabilities: 13,
            psm_nusd_supply: 14,
            psm_idle_usdc,
            psm_kamino_deployed_usdc,
            psm_kamino_collateral_vault: Pubkey::new_from_array([15; 32]),
            psm_cap: 16,
            protocol_debt_cap: 17,
            stability_fee_apr_bps,
            psm_swap_in_fee_bps: 18,
            psm_swap_out_fee_bps: 19,
            kamino_program_id: Pubkey::new_from_array([20; 32]),
            insurance_target_bps,
            insurance_fee_share_bps,
            staker_revenue_accounting_initialized: false,
            paused: true,
            bump: 21,
            protocol_revenue_nusd_vault: Pubkey::new_from_array([22; 32]),
            realized_revenue_for_protocol: 23,
            staker_target_revenue_due: 24,
            staker_target_last_accrual_ts: 25,
            staker_target_apr_bps,
            staker_capacity_kamino_apr_bps,
        };
        let mut data = CORE_PROTOCOL_DISCRIMINATOR.to_vec();
        protocol.serialize(&mut data).unwrap();

        assert_eq!(data.get(0..8), Some(&CORE_PROTOCOL_DISCRIMINATOR[..]));
        assert_eq!(
            read_pubkey_at(&data, CORE_PROTOCOL_AUTHORITY_OFFSET).unwrap(),
            authority
        );
        assert_eq!(
            read_pubkey_at(&data, CORE_PROTOCOL_NUSD_MINT_OFFSET).unwrap(),
            nusd_mint
        );
        assert_eq!(
            read_u128_at(&data, CORE_PROTOCOL_TOTAL_DEBT_OFFSET).unwrap(),
            total_debt
        );
        assert_eq!(
            read_u128_at(&data, CORE_PROTOCOL_INSURANCE_FUND_NUSD_OFFSET).unwrap(),
            insurance_fund_nusd
        );
        assert_eq!(
            read_u128_at(&data, CORE_PROTOCOL_BAD_DEBT_NUSD_OFFSET).unwrap(),
            12
        );
        assert_eq!(
            read_u128_at(&data, CORE_PROTOCOL_PSM_IDLE_USDC_OFFSET).unwrap(),
            psm_idle_usdc
        );
        assert_eq!(
            read_u128_at(&data, CORE_PROTOCOL_PSM_KAMINO_DEPLOYED_USDC_OFFSET).unwrap(),
            psm_kamino_deployed_usdc
        );
        assert_eq!(
            read_u64_at(&data, CORE_PROTOCOL_STABILITY_FEE_APR_BPS_OFFSET).unwrap(),
            stability_fee_apr_bps
        );
        assert_eq!(
            read_u16_at(&data, CORE_PROTOCOL_INSURANCE_TARGET_BPS_OFFSET).unwrap(),
            insurance_target_bps
        );
        assert_eq!(
            read_u16_at(&data, CORE_PROTOCOL_INSURANCE_FEE_SHARE_BPS_OFFSET).unwrap(),
            insurance_fee_share_bps
        );
        assert_eq!(
            read_u64_at(&data, CORE_PROTOCOL_STAKER_TARGET_APR_BPS_OFFSET).unwrap(),
            staker_target_apr_bps
        );
        assert_eq!(
            read_u64_at(&data, CORE_PROTOCOL_STAKER_CAPACITY_KAMINO_APR_BPS_OFFSET).unwrap(),
            staker_capacity_kamino_apr_bps
        );
    }

    #[test]
    fn pending_withdrawal_layout_remains_mainnet_compatible() {
        assert_eq!(PendingWithdrawalAccount::INIT_SPACE, 106);
    }
}
