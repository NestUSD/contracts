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
const CORE_PROTOCOL_NUSD_MINT_OFFSET: usize = 72;
const CORE_PROTOCOL_TOTAL_DEBT_OFFSET: usize = 264;
const CORE_PROTOCOL_INSURANCE_FUND_NUSD_OFFSET: usize = 312;
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

    pub fn stake(ctx: Context<Stake>, amount: u64) -> Result<()> {
        instructions::stake(ctx, amount)
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
