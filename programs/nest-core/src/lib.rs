use anchor_lang::prelude::*;
use anchor_lang::solana_program::program_option::COption;
use anchor_lang::solana_program::{
    ed25519_program,
    instruction::{AccountMeta, Instruction},
    program::invoke_signed,
    sysvar,
};
use anchor_spl::token::ID as SPL_TOKEN_PROGRAM_ID;
use anchor_spl::token_interface::{
    self, BurnChecked, Mint, MintToChecked, TokenAccount, TokenInterface, TransferChecked,
};
use nest_domain as domain;

declare_id!("HxbLPNuQD7KKDVQoSQgY1cLMLrsaoseT65Xoczh7zHQW");

const NEST_STAKE_PROGRAM_ID: Pubkey = pubkey!("EdYg6JsyntWpf3WGofNWKzBYnQPEvWFZSzUNim3PLbhB");
const TOKEN_2022_PROGRAM_ID: Pubkey = pubkey!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb");
const KLEND_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
const KLEND_STAGING_PROGRAM_ID: Pubkey = pubkey!("SLendK7ySfcEzyaFqy93gDnD3RtrpXJcnRwb6zFHJSh");
const KLEND_NULL_PUBKEY: Pubkey = Pubkey::new_from_array([
    11, 193, 238, 216, 208, 116, 241, 195, 55, 212, 76, 22, 75, 202, 40, 216, 76, 206, 27, 169,
    138, 64, 177, 28, 19, 90, 156, 0, 0, 0, 0, 0,
]);
const STABLECOIN_DECIMALS: u8 = 6;
const MAX_STABILITY_FEE_APR_BPS: u64 = 1_000;
const MAX_STAKER_TARGET_APR_BPS: u64 = 2_000;
const MAX_STAKER_CAPACITY_KAMINO_APR_BPS: u64 = 2_000;
const SECONDS_PER_DAY: i64 = 86_400;
const MAX_XSTOCK_PRICE_STALENESS_SECONDS: i64 = 120;
const STAKING_STATE_DISCRIMINATOR: [u8; 8] = [152, 226, 234, 201, 202, 8, 155, 60];
const STAKING_STATE_NUSD_MINT_OFFSET: usize = 40;
const STAKING_STATE_STAKING_VAULT_NUSD_OFFSET: usize = 216;
const PSM_KAMINO_TARGET_BPS: u128 = 9_000;
const PSM_KAMINO_BPS_DENOMINATOR: u128 = 10_000;
const MAX_PSM_OUTFLOW_WINDOW_SECONDS: i64 = 7 * SECONDS_PER_DAY;
const KLEND_REFRESH_RESERVE_DISCRIMINATOR: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];
const KLEND_DEPOSIT_RESERVE_LIQUIDITY_DISCRIMINATOR: [u8; 8] = [169, 201, 30, 126, 6, 205, 102, 68];
const KLEND_REDEEM_RESERVE_COLLATERAL_DISCRIMINATOR: [u8; 8] =
    [234, 117, 181, 125, 185, 142, 220, 29];
const KLEND_RESERVE_PUBKEY_FIELD_SIZE: usize = 32;
const KLEND_RESERVE_LENDING_MARKET_OFFSET: usize = 8 + 24;
const KLEND_RESERVE_LIQUIDITY_MINT_OFFSET: usize = 8 + 120;
const KLEND_RESERVE_LIQUIDITY_SUPPLY_OFFSET: usize = KLEND_RESERVE_LIQUIDITY_MINT_OFFSET + 32;
const KLEND_RESERVE_LIQUIDITY_TOKEN_PROGRAM_OFFSET: usize =
    KLEND_RESERVE_LIQUIDITY_MINT_OFFSET + 280;
const KLEND_RESERVE_COLLATERAL_MINT_OFFSET: usize = 8 + 2552;

mod ix;

#[program]
pub mod nest_core {
    use super::*;

    pub fn initialize_protocol(
        ctx: Context<InitializeProtocol>,
        params: InitializeProtocolParams,
    ) -> Result<()> {
        ix::setup::initialize_protocol(ctx, params)
    }

    pub fn add_collateral(ctx: Context<AddCollateral>, params: AddCollateralParams) -> Result<()> {
        ix::setup::add_collateral(ctx, params)
    }

    pub fn initialize_vault(ctx: Context<InitializeVault>) -> Result<()> {
        ix::setup::initialize_vault(ctx)
    }

    pub fn refresh_lazer_oracle(
        ctx: Context<RefreshLazerOracle>,
        message_data: Vec<u8>,
        ed25519_instruction_index: u16,
        signature_index: u8,
    ) -> Result<()> {
        ix::oracle::refresh_lazer_oracle(
            ctx,
            message_data,
            ed25519_instruction_index,
            signature_index,
        )
    }

    pub fn set_nest_price_signer(ctx: Context<SetNestPriceSigner>, signer: Pubkey) -> Result<()> {
        ix::admin::set_nest_price_signer(ctx, signer)
    }

    pub fn refresh_signed_oracle(
        ctx: Context<RefreshSignedOracle>,
        payload: SignedPricePayload,
        ed25519_instruction_index: u16,
    ) -> Result<()> {
        ix::oracle::refresh_signed_oracle(ctx, payload, ed25519_instruction_index)
    }

    pub fn deposit(ctx: Context<Deposit>, amount: u64) -> Result<()> {
        ix::cdp::deposit(ctx, amount)
    }

    pub fn withdraw_with_oracle(ctx: Context<WithdrawWithOracle>, amount: u64) -> Result<()> {
        ix::cdp::withdraw_with_oracle(ctx, amount)
    }

    pub fn accrue_fee(ctx: Context<MutateVault>) -> Result<()> {
        ix::cdp::accrue_fee(ctx)
    }

    pub fn mint_nusd_with_oracle(ctx: Context<MintNusdWithOracle>, amount: u64) -> Result<()> {
        ix::cdp::mint_nusd_with_oracle(ctx, amount)
    }

    pub fn repay_nusd(ctx: Context<RepayNusd>, amount: u64) -> Result<()> {
        ix::cdp::repay_nusd(ctx, amount)
    }

    pub fn liquidate_with_oracle(
        ctx: Context<LiquidateWithOracle>,
        requested_repay: u64,
    ) -> Result<()> {
        ix::liquidation::instant::liquidate_with_oracle(ctx, requested_repay)
    }

    pub fn start_liquidation_with_oracle(ctx: Context<StartLiquidationWithOracle>) -> Result<()> {
        ix::liquidation::two_step::start_liquidation_with_oracle(ctx)
    }

    pub fn settle_liquidation_proceeds(
        ctx: Context<SettleLiquidationProceeds>,
        sale_proceeds_usdc: u64,
    ) -> Result<()> {
        ix::liquidation::two_step::settle_liquidation_proceeds(ctx, sale_proceeds_usdc)
    }

    pub fn withdraw_insurance_collateral(
        ctx: Context<WithdrawInsuranceCollateral>,
        amount: u64,
    ) -> Result<()> {
        ix::liquidation::insurance::withdraw_insurance_collateral(ctx, amount)
    }

    pub fn release_buyback_surplus(
        ctx: Context<ReleaseBuybackSurplus>,
        amount: u64,
        min_psm_idle_after: u64,
    ) -> Result<()> {
        ix::liquidation::buyback::release_buyback_surplus(ctx, amount, min_psm_idle_after)
    }

    pub fn psm_swap_in(ctx: Context<PsmSwapIn>, amount: u64) -> Result<()> {
        ix::psm::psm_swap_in(ctx, amount)
    }

    pub fn psm_swap_out(ctx: Context<PsmSwapOut>, amount: u64) -> Result<()> {
        ix::psm::psm_swap_out(ctx, amount)
    }

    pub fn rebalance_psm_usdc_to_kamino(ctx: Context<PsmKamino>, max_amount: u64) -> Result<()> {
        ix::kamino::rebalance_psm_usdc_to_kamino(ctx, max_amount)
    }

    pub fn redeem_psm_usdc_from_kamino(
        ctx: Context<PsmKamino>,
        collateral_amount: u64,
    ) -> Result<()> {
        ix::kamino::redeem_psm_usdc_from_kamino(ctx, collateral_amount)
    }

    pub fn realize_psm_kamino_yield(
        ctx: Context<RealizePsmKaminoYield>,
        collateral_amount: u64,
        min_yield: u64,
    ) -> Result<()> {
        ix::kamino::realize_psm_kamino_yield(ctx, collateral_amount, min_yield)
    }

    pub fn realize_psm_yield(ctx: Context<RealizePsmYield>, amount: u64) -> Result<()> {
        ix::revenue::realize_psm_yield(ctx, amount)
    }

    pub fn cover_bad_debt(ctx: Context<CoverBadDebt>, amount: u64) -> Result<()> {
        ix::revenue::cover_bad_debt(ctx, amount)
    }

    pub fn checkpoint_staker_target_revenue(
        ctx: Context<CheckpointStakerTargetRevenue>,
    ) -> Result<()> {
        ix::revenue::checkpoint_staker_target_revenue(ctx)
    }

    pub fn set_paused(ctx: Context<MutateProtocol>, paused: bool) -> Result<()> {
        ix::admin::set_paused(ctx, paused)
    }

    pub fn set_psm_outflow_circuit_breaker(
        ctx: Context<MutateProtocol>,
        enabled: bool,
        limit_usdc: u128,
        limit_bps: u16,
        window_seconds: i64,
    ) -> Result<()> {
        ix::admin::set_psm_outflow_circuit_breaker(
            ctx,
            enabled,
            limit_usdc,
            limit_bps,
            window_seconds,
        )
    }

    pub fn set_liquidation_authority(
        ctx: Context<MutateProtocol>,
        liquidation_authority: Pubkey,
    ) -> Result<()> {
        ix::admin::set_liquidation_authority(ctx, liquidation_authority)
    }

    pub fn set_protocol_authority(ctx: Context<MutateProtocol>, authority: Pubkey) -> Result<()> {
        ix::admin::set_protocol_authority(ctx, authority)
    }

    pub fn set_staker_yield_params(
        ctx: Context<SetStakerYieldParams>,
        staker_target_apr_bps: u64,
        staker_capacity_kamino_apr_bps: u64,
    ) -> Result<()> {
        ix::admin::set_staker_yield_params(
            ctx,
            staker_target_apr_bps,
            staker_capacity_kamino_apr_bps,
        )
    }

    pub fn set_psm_kamino_collateral_vault(
        ctx: Context<SetPsmKaminoCollateralVault>,
    ) -> Result<()> {
        ix::admin::set_psm_kamino_collateral_vault(ctx)
    }

    pub fn set_buyback_config(
        ctx: Context<SetBuybackConfig>,
        buyback_authority: Pubkey,
    ) -> Result<()> {
        ix::admin::set_buyback_config(ctx, buyback_authority)
    }

    pub fn set_collateral_paused(
        ctx: Context<MutateCollateral>,
        deposits_paused: bool,
        borrows_paused: bool,
        withdraws_paused: bool,
    ) -> Result<()> {
        ix::admin::set_collateral_paused(ctx, deposits_paused, borrows_paused, withdraws_paused)
    }

    pub fn trip_collateral_vault_coverage_breaker(
        ctx: Context<TripCollateralVaultCoverageBreaker>,
    ) -> Result<()> {
        ix::admin::trip_collateral_vault_coverage_breaker(ctx)
    }

    pub fn trip_collateral_emergency_breaker(
        ctx: Context<TripCollateralEmergencyBreaker>,
    ) -> Result<()> {
        ix::admin::trip_collateral_emergency_breaker(ctx)
    }

    pub fn set_collateral_risk_params(
        ctx: Context<MutateCollateral>,
        borrow_ltv_bps: u16,
        liquidation_threshold_bps: u16,
        liquidation_penalty_bps: u16,
        close_factor_bps: u16,
    ) -> Result<()> {
        ix::admin::set_collateral_risk_params(
            ctx,
            borrow_ltv_bps,
            liquidation_threshold_bps,
            liquidation_penalty_bps,
            close_factor_bps,
        )
    }

    pub fn set_collateral_caps(
        ctx: Context<MutateCollateral>,
        per_vault_debt_cap: u128,
        protocol_debt_cap: u128,
        deposit_cap_raw: u128,
    ) -> Result<()> {
        ix::admin::set_collateral_caps(ctx, per_vault_debt_cap, protocol_debt_cap, deposit_cap_raw)
    }

    pub fn set_collateral_oracle_params(
        ctx: Context<MutateCollateral>,
        xstock_usd_feed_id: [u8; 32],
        max_confidence_bps: u16,
        max_staleness_seconds: i64,
    ) -> Result<()> {
        ix::admin::set_collateral_oracle_params(
            ctx,
            xstock_usd_feed_id,
            max_confidence_bps,
            max_staleness_seconds,
        )
    }
}

include!("accounts.rs");
include!("state.rs");
include!("params.rs");
include!("errors.rs");
include!("helpers/auth.rs");
include!("helpers/domain.rs");
include!("helpers/oracle.rs");
include!("helpers/token.rs");
include!("helpers/revenue.rs");
include!("helpers/staking_reader.rs");
include!("helpers/psm.rs");
include!("helpers/kamino.rs");
include!("helpers/errors.rs");

#[cfg(test)]
mod tests {
    use super::*;

    include!("tests.rs");
}
