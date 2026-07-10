use crate::*;

fn feed(byte: u8) -> [u8; 32] {
    [byte; 32]
}

fn price(feed_id: [u8; 32], price_e8: u64, confidence_e8: u64, publish_time: i64) -> OraclePrice {
    OraclePrice {
        feed_id,
        price_e8,
        confidence_e8,
        publish_time,
    }
}

fn policy(feed_id: [u8; 32]) -> PricePolicy {
    PricePolicy {
        expected_feed_id: feed_id,
        max_staleness_seconds: 30,
        max_confidence_bps: 200,
    }
}

fn assert_protocol_matches_vaults(protocol: ProtocolAccounting, vaults: &[Vault]) {
    let total_debt = vaults
        .iter()
        .map(|vault| vault.total_debt().unwrap())
        .sum::<u128>();
    let total_uncollected_fees = vaults.iter().map(|vault| vault.accrued_fee).sum::<u128>();
    assert_eq!(protocol.total_debt, total_debt);
    assert_eq!(protocol.total_uncollected_fees, total_uncollected_fees);
    assert!(protocol.insurance_fund_nusd <= protocol.cdp_insurance_target().unwrap());
}

fn assert_staking_pool_invariants(pool: StakingPool, active_shares: u128, pending_shares: u128) {
    assert_eq!(pool.total_shares, active_shares + pending_shares);
    assert!(pool.staking_vault_nusd >= pool.unvested_revenue);
    assert!(pool.staking_vault_nusd >= pool.reserved_pending_claims);
    assert!(pool.accounted_assets().unwrap() <= pool.staking_vault_nusd);
}

#[test]
fn oracle_prices_raw_xstock_conservatively() {
    let now = 1_000;
    let inputs = PricingInputs {
        xstock_usd: price(feed(1), 100 * PRICE_SCALE as u64, PRICE_SCALE as u64, now),
        now,
        xstock_policy: policy(feed(1)),
    };

    let safe = safe_raw_token_price_e8(inputs).unwrap();
    assert_eq!(safe, 9_900_000_000);

    let value = collateral_value_usd(10 * 100_000_000, 8, safe).unwrap();
    assert_eq!(value, 990_000_000);

    let raw = collateral_raw_for_usd_value(99_000_000, 8, safe).unwrap();
    assert_eq!(raw, 100_000_000);
}

#[test]
fn oracle_prices_from_xstock_usd() {
    let now = 1_000;
    let inputs = PricingInputs {
        xstock_usd: price(feed(1), 130 * PRICE_SCALE as u64, 0, now),
        now,
        xstock_policy: policy(feed(1)),
    };

    let safe = safe_raw_token_price_e8(inputs).unwrap();
    assert_eq!(safe, 130 * PRICE_SCALE);

    let value = collateral_value_usd(2 * 100_000_000, 8, safe).unwrap();
    assert_eq!(value, 260_000_000);
}

#[test]
fn collateral_value_round_trip_is_conservative_across_decimals() {
    let prices_e8 = [
        1,               // Smallest positive e8 price.
        PRICE_SCALE / 3, // Repeating conversion into 6-decimal USD.
        PRICE_SCALE,
        12_345_678_901,
        250 * PRICE_SCALE,
    ];
    let amounts = [1, 7, 999, 1_000_000, 123_456_789, 9_876_543_210];

    for decimals in [1_u8, 2, 6, 8, 9, 18] {
        for price_e8 in prices_e8 {
            for raw_amount in amounts {
                let usd_value = collateral_value_usd(raw_amount, decimals, price_e8).unwrap();
                if usd_value == 0 {
                    continue;
                }

                let required_raw =
                    collateral_raw_for_usd_value(usd_value, decimals, price_e8).unwrap();
                let required_value =
                    collateral_value_usd(required_raw, decimals, price_e8).unwrap();
                assert!(required_value >= usd_value);
                assert!(required_raw <= raw_amount);

                if required_raw > 0 {
                    let one_less_value =
                        collateral_value_usd(required_raw - 1, decimals, price_e8).unwrap();
                    assert!(one_less_value < usd_value);
                }
            }
        }
    }
}

#[test]
fn oracle_rejects_wrong_feed_stale_and_wide_confidence() {
    let now = 1_000;
    let mut p = price(feed(1), PRICE_SCALE as u64, 0, now);
    assert_eq!(
        lower_confidence_bound(p, policy(feed(2)), now),
        Err(NestError::WrongFeed)
    );

    p.feed_id = feed(1);
    p.publish_time = now - 31;
    assert_eq!(
        lower_confidence_bound(p, policy(feed(1)), now),
        Err(NestError::OracleStale)
    );

    p.publish_time = now + 1;
    assert_eq!(
        lower_confidence_bound(p, policy(feed(1)), now),
        Ok(PRICE_SCALE)
    );

    p.publish_time = now + 61;
    assert_eq!(
        lower_confidence_bound(p, policy(feed(1)), now),
        Err(NestError::OracleStale)
    );

    p.publish_time = now;
    p.confidence_e8 = 3_000_000;
    assert_eq!(
        lower_confidence_bound(p, policy(feed(1)), now),
        Err(NestError::ConfidenceTooWide)
    );
}

#[test]
fn oracle_rejects_invalid_policy_bounds() {
    let now = 1_000;
    let p = price(feed(1), PRICE_SCALE as u64, 0, now);
    let mut invalid_policy = policy(feed(1));

    invalid_policy.max_staleness_seconds = 0;
    assert_eq!(
        lower_confidence_bound(p, invalid_policy, now),
        Err(NestError::InvalidParameter)
    );

    invalid_policy = policy(feed(1));
    invalid_policy.max_confidence_bps = 0;
    assert_eq!(
        lower_confidence_bound(p, invalid_policy, now),
        Err(NestError::InvalidParameter)
    );

    invalid_policy = policy(feed(1));
    invalid_policy.max_confidence_bps = BPS_DENOMINATOR as u16 + 1;
    assert_eq!(
        lower_confidence_bound(p, invalid_policy, now),
        Err(NestError::InvalidParameter)
    );
}

#[test]
fn borrow_repay_realizes_fees_before_principal() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 100,
        principal_debt: 100_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();

    let fee = accrue_stability_fee(
        &mut vault,
        &mut protocol,
        DEFAULT_STABILITY_FEE_BPS,
        31_536_000,
    )
    .unwrap();
    assert_eq!(fee, 1_500_000);
    assert_eq!(protocol.total_uncollected_fees, 1_500_000);

    let outcome = repay(&mut vault, &mut protocol, 2_000_000).unwrap();
    assert_eq!(outcome.fee_paid, 1_500_000);
    assert_eq!(outcome.principal_paid, 500_000);
    assert_eq!(outcome.bad_debt_repaid, 0);
    assert_eq!(vault.accrued_fee, 0);
    assert_eq!(vault.principal_debt, 99_500_000);
    assert_eq!(protocol.total_uncollected_fees, 0);
    assert_eq!(protocol.insurance_fund_nusd, 300_000);
    assert_eq!(protocol.realized_revenue_for_stakers, 1_200_000);
}

#[test]
fn repayment_burns_fees_against_bad_debt_before_routing_revenue() {
    let mut protocol = ProtocolAccounting::new();
    protocol.bad_debt_nusd = 2_000_000;
    protocol.total_debt = 103_000_000;
    protocol.total_uncollected_fees = 3_000_000;
    let mut vault = Vault {
        collateral_raw: 100,
        principal_debt: 100_000_000,
        accrued_fee: 3_000_000,
        last_accrual_ts: 0,
    };

    let outcome = repay(&mut vault, &mut protocol, 4_000_000).unwrap();

    assert_eq!(outcome.fee_paid, 3_000_000);
    assert_eq!(outcome.principal_paid, 1_000_000);
    assert_eq!(outcome.bad_debt_repaid, 2_000_000);
    assert_eq!(protocol.bad_debt_nusd, 0);
    assert_eq!(protocol.insurance_fund_nusd, 200_000);
    assert_eq!(protocol.realized_revenue_for_stakers, 800_000);
}

#[test]
fn repay_grid_preserves_debt_and_fee_accounting() {
    for principal in [1_u128, 1_000_000, 50_000_000, 1_000_000_000] {
        for accrued_fee in [0_u128, 1, 25_000, principal / 7 + 1] {
            for payment in [
                1_u128,
                accrued_fee,
                accrued_fee + 1,
                principal + accrued_fee + 99,
            ] {
                if payment == 0 {
                    continue;
                }
                let mut vault = Vault {
                    collateral_raw: 1_000_000,
                    principal_debt: principal,
                    accrued_fee,
                    last_accrual_ts: 0,
                };
                let mut protocol = ProtocolAccounting::new();
                protocol.total_debt = vault.total_debt().unwrap();
                protocol.total_uncollected_fees = accrued_fee;

                let debt_before = vault.total_debt().unwrap();
                let insurance_before = protocol.insurance_fund_nusd;
                let staker_before = protocol.realized_revenue_for_stakers;
                let outcome = repay(&mut vault, &mut protocol, payment).unwrap();
                let paid = outcome.fee_paid + outcome.principal_paid;

                assert!(outcome.fee_paid <= accrued_fee);
                assert!(outcome.principal_paid <= principal);
                assert_eq!(outcome.overpayment, payment - paid);
                assert_eq!(vault.total_debt().unwrap(), debt_before - paid);
                assert_eq!(protocol.total_debt, vault.total_debt().unwrap());
                assert_eq!(protocol.total_uncollected_fees, vault.accrued_fee);
                assert_eq!(
                    (protocol.insurance_fund_nusd - insurance_before)
                        + (protocol.realized_revenue_for_stakers - staker_before),
                    outcome.fee_paid
                );
            }
        }
    }
}

#[test]
fn stability_fee_accrual_rounds_positive_intervals_up_to_one_atomic_unit() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 100,
        principal_debt: 100_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();

    let too_early =
        accrue_stability_fee(&mut vault, &mut protocol, DEFAULT_STABILITY_FEE_BPS, 1).unwrap();
    assert_eq!(too_early, 1);
    assert_eq!(vault.last_accrual_ts, 1);
    assert_eq!(vault.accrued_fee, 1);
    assert_eq!(protocol.total_uncollected_fees, 1);

    let one_micro =
        accrue_stability_fee(&mut vault, &mut protocol, DEFAULT_STABILITY_FEE_BPS, 22).unwrap();
    assert_eq!(one_micro, 1);
    assert_eq!(vault.last_accrual_ts, 22);
    assert_eq!(vault.accrued_fee, 2);
    assert_eq!(protocol.total_uncollected_fees, 2);

    let second_micro =
        accrue_stability_fee(&mut vault, &mut protocol, DEFAULT_STABILITY_FEE_BPS, 23).unwrap();
    assert_eq!(second_micro, 1);
    assert_eq!(vault.last_accrual_ts, 23);
    assert_eq!(vault.accrued_fee, 3);
    assert_eq!(protocol.total_uncollected_fees, 3);
}

#[test]
fn rounded_fee_checkpoint_does_not_backdate_a_later_borrow() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_000_000,
        principal_debt: 1,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = 1;
    let thirty_days = 30 * 24 * 60 * 60;

    assert_eq!(
        accrue_stability_fee(
            &mut vault,
            &mut protocol,
            DEFAULT_STABILITY_FEE_BPS,
            thirty_days,
        )
        .unwrap(),
        1
    );
    let params = CollateralParams {
        per_vault_debt_cap: 200_000_000_000,
        protocol_debt_cap: 200_000_000_000,
        collateral_debt_cap: 200_000_000_000,
        ..CollateralParams::default()
    };
    borrow(
        &mut vault,
        &mut protocol,
        1_000_000_000_000,
        params,
        100_000_000_000,
    )
    .unwrap();

    let post_borrow_fee = accrue_stability_fee(
        &mut vault,
        &mut protocol,
        DEFAULT_STABILITY_FEE_BPS,
        thirty_days + 1,
    )
    .unwrap();
    assert_eq!(post_borrow_fee, 47);
    assert_eq!(vault.last_accrual_ts, thirty_days + 1);
}

#[test]
fn stability_fee_accrual_handles_stale_future_and_overflow_time_bounds() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 100,
        principal_debt: 100_000_000,
        accrued_fee: 123,
        last_accrual_ts: 100,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    protocol.total_uncollected_fees = vault.accrued_fee;

    let unchanged_vault = vault;
    let unchanged_protocol = protocol;
    let stale =
        accrue_stability_fee(&mut vault, &mut protocol, DEFAULT_STABILITY_FEE_BPS, 100).unwrap();
    assert_eq!(stale, 0);
    assert_eq!(vault, unchanged_vault);
    assert_eq!(protocol, unchanged_protocol);

    let future_last =
        accrue_stability_fee(&mut vault, &mut protocol, DEFAULT_STABILITY_FEE_BPS, 99).unwrap();
    assert_eq!(future_last, 0);
    assert_eq!(vault, unchanged_vault);
    assert_eq!(protocol, unchanged_protocol);

    vault.last_accrual_ts = i64::MIN;
    assert_eq!(
        accrue_stability_fee(
            &mut vault,
            &mut protocol,
            DEFAULT_STABILITY_FEE_BPS,
            i64::MAX
        ),
        Err(NestError::MathOverflow)
    );
}

#[test]
fn borrow_enforces_ltv_and_caps() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault::default();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        per_vault_debt_cap: 50_000_000,
        protocol_debt_cap: 500_000_000,
        ..CollateralParams::default()
    };

    borrow(&mut vault, &mut protocol, 100_000_000, params, 45_000_000).unwrap();
    assert_eq!(vault.principal_debt, 45_000_000);
    assert_eq!(
        borrow(&mut vault, &mut protocol, 100_000_000, params, 1),
        Err(NestError::InsufficientCollateral)
    );
}

#[test]
fn borrow_grid_never_exceeds_ltv_or_debt_caps() {
    for collateral_value in [1_u128, 999_999, 100_000_000, 9_876_543_210] {
        for borrow_ltv_bps in [1_u16, 2_500, 4_500, 7_500, 9_998] {
            let borrow_limit =
                mul_div_down(collateral_value, borrow_ltv_bps as u128, BPS_DENOMINATOR).unwrap();
            for existing_debt in [0_u128, borrow_limit / 2, borrow_limit] {
                let vault = Vault {
                    collateral_raw: 1_000_000,
                    principal_debt: existing_debt,
                    accrued_fee: 0,
                    last_accrual_ts: 0,
                };
                let mut protocol = ProtocolAccounting::new();
                protocol.total_debt = existing_debt;
                let params = CollateralParams {
                    borrow_ltv_bps,
                    liquidation_threshold_bps: borrow_ltv_bps + 1,
                    liquidation_penalty_bps: 800,
                    close_factor_bps: 5_000,
                    full_liquidation_threshold_bps: FULL_LIQUIDATION_HEALTH_FACTOR_BPS,
                    per_vault_debt_cap: borrow_limit,
                    protocol_debt_cap: borrow_limit,
                    collateral_debt_cap: borrow_limit,
                    collateral_debt_outstanding: existing_debt,
                    deposit_cap_raw: u128::MAX,
                    deposits_paused: false,
                    borrows_paused: false,
                    withdraws_paused: false,
                };

                let remaining_capacity = borrow_limit.saturating_sub(existing_debt);
                if remaining_capacity > 0 {
                    let mut allowed_vault = vault;
                    let mut allowed_protocol = protocol;
                    borrow(
                        &mut allowed_vault,
                        &mut allowed_protocol,
                        collateral_value,
                        params,
                        remaining_capacity,
                    )
                    .unwrap();
                    assert_eq!(allowed_vault.total_debt().unwrap(), borrow_limit);
                    assert_eq!(allowed_protocol.total_debt, borrow_limit);
                }

                assert_eq!(
                    can_borrow(
                        collateral_value,
                        vault,
                        params,
                        protocol,
                        remaining_capacity + 1
                    ),
                    Err(NestError::InsufficientCollateral)
                );
            }
        }
    }
}

#[test]
fn collateral_params_reject_out_of_range_liquidation_settings() {
    let vault = Vault::default();
    let protocol = ProtocolAccounting::new();
    let mut params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        ..CollateralParams::default()
    };

    params.liquidation_penalty_bps = BPS_DENOMINATOR as u16 + 1;
    assert_eq!(
        can_borrow(100_000_000, vault, params, protocol, 1),
        Err(NestError::InvalidParameter)
    );

    params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        full_liquidation_threshold_bps: BPS_DENOMINATOR as u16 + 1,
        ..CollateralParams::default()
    };
    assert_eq!(
        can_borrow(100_000_000, vault, params, protocol, 1),
        Err(NestError::InvalidParameter)
    );
}

#[test]
fn withdraw_must_preserve_borrow_ltv_not_only_liquidation_threshold() {
    let vault = Vault {
        collateral_raw: 0,
        principal_debt: 70_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    let params = CollateralParams {
        borrow_ltv_bps: 7_000,
        liquidation_threshold_bps: 8_000,
        ..CollateralParams::default()
    };
    let collateral_value_after_withdraw = 90_000_000;

    assert!(
        health_factor_bps(
            collateral_value_after_withdraw,
            vault.total_debt().unwrap(),
            params.liquidation_threshold_bps,
        )
        .unwrap()
            >= BPS_DENOMINATOR
    );
    assert_eq!(
        can_withdraw(collateral_value_after_withdraw, vault, params),
        Err(NestError::InsufficientCollateral)
    );
}

#[test]
fn borrow_enforces_per_collateral_debt_cap() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault::default();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        per_vault_debt_cap: 50_000_000,
        protocol_debt_cap: 500_000_000,
        collateral_debt_cap: 10_000_000,
        collateral_debt_outstanding: 10_000_000,
        ..CollateralParams::default()
    };

    assert_eq!(
        borrow(&mut vault, &mut protocol, 100_000_000, params, 1),
        Err(NestError::DebtCapExceeded)
    );
}

#[test]
fn zero_value_debt_operations_are_rejected() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_000_000_000,
        principal_debt: 60_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    assert_eq!(
        borrow(
            &mut Vault::default(),
            &mut ProtocolAccounting::new(),
            100_000_000,
            params,
            0
        ),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        repay(&mut vault, &mut protocol, 0),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        liquidate(
            &mut vault,
            &mut protocol,
            90_000_000,
            9 * PRICE_SCALE,
            8,
            params,
            0,
        ),
        Err(NestError::InvalidParameter)
    );
}

#[test]
fn liquidation_routes_penalty_to_stakers() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_000_000_000,
        principal_debt: 60_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        90_000_000,
        9 * PRICE_SCALE,
        8,
        params,
        30_000_000,
    )
    .unwrap();

    assert_eq!(out.repaid_debt, 30_000_000);
    assert!(out.keeper_collateral_raw > 0);
    assert_eq!(out.insurance_collateral_raw, 0);
    assert_eq!(out.staker_penalty_nusd, 2_400_000);
    assert_eq!(
        protocol.realized_revenue_for_stakers,
        out.staker_penalty_nusd
    );
    assert_eq!(protocol.total_debt, 30_000_000);
    assert!(vault.collateral_raw < 1_000_000_000);
}

#[test]
fn underwater_liquidation_only_routes_realized_penalty_to_stakers() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_000_000_000,
        principal_debt: 60_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };
    let insurance_nusd_before = protocol.insurance_fund_nusd;

    let out = liquidate(
        &mut vault,
        &mut protocol,
        90_000_000,
        9 * PRICE_SCALE,
        8,
        params,
        30_000_000,
    )
    .unwrap();

    assert_eq!(out.fee_paid, 0);
    assert!(out.keeper_collateral_raw > 0);
    assert_eq!(out.insurance_collateral_raw, 0);
    assert_eq!(
        protocol.realized_revenue_for_stakers,
        out.staker_penalty_nusd
    );
    assert_eq!(protocol.insurance_fund_nusd, insurance_nusd_before);
}

#[test]
fn partial_liquidation_caps_repay_at_close_factor() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 10_000_000,
        principal_debt: 450_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        800_000_000,
        80 * PRICE_SCALE,
        6,
        params,
        400_000_000,
    )
    .unwrap();

    assert!(!out.full_liquidation);
    assert_eq!(out.repaid_debt, 225_000_000);
    assert_eq!(protocol.total_debt, 225_000_000);
    assert_eq!(vault.principal_debt, 225_000_000);
    assert!(vault.collateral_raw > 0);
}

#[test]
fn liquidation_reports_actual_fee_and_principal_components() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 10_000_000,
        principal_debt: 100_000_000,
        accrued_fee: 5_000_000,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    protocol.total_uncollected_fees = vault.accrued_fee;
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        190_000_000,
        19 * PRICE_SCALE,
        6,
        params,
        50_000_000,
    )
    .unwrap();

    assert!(!out.full_liquidation);
    assert_eq!(out.repaid_debt, 50_000_000);
    assert_eq!(out.fee_paid, 5_000_000);
    assert_eq!(out.principal_paid, 45_000_000);
    assert_eq!(out.repaid_debt, out.fee_paid + out.principal_paid);
    assert_eq!(vault.accrued_fee, 0);
    assert_eq!(vault.principal_debt, 55_000_000);
    assert_eq!(protocol.total_uncollected_fees, 0);
    assert_eq!(protocol.total_debt, 55_000_000);
}

#[test]
fn full_liquidation_when_close_factor_would_leave_vault_unhealthy() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 9_000_000,
        principal_debt: 550_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 1_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        900_000_000,
        100 * PRICE_SCALE,
        6,
        params,
        550_000_000,
    )
    .unwrap();

    assert!(out.full_liquidation);
    assert_eq!(out.repaid_debt, 550_000_000);
    assert_eq!(protocol.total_debt, 0);
    assert_eq!(vault.total_debt().unwrap(), 0);
    assert!(vault.collateral_raw > 0);
}

#[test]
fn full_liquidation_rejects_too_small_repay_that_would_not_exhaust_collateral() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 2_000_000,
        principal_debt: 200_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    assert_eq!(
        liquidate(
            &mut vault,
            &mut protocol,
            108_000_000,
            100 * PRICE_SCALE,
            6,
            params,
            10_000_000,
        ),
        Err(NestError::InvalidParameter)
    );

    assert_eq!(vault.collateral_raw, 2_000_000);
    assert_eq!(vault.total_debt().unwrap(), 200_000_000);
    assert_eq!(protocol.total_debt, 200_000_000);
}

#[test]
fn full_liquidation_records_bad_debt_when_repay_exactly_exhausts_collateral() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_080_000,
        principal_debt: 200_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        108_000_000,
        100 * PRICE_SCALE,
        6,
        params,
        100_000_000,
    )
    .unwrap();

    assert!(out.full_liquidation);
    assert_eq!(
        out.keeper_collateral_raw + out.insurance_collateral_raw,
        1_080_000
    );
    assert_eq!(vault.collateral_raw, 0);
    assert_eq!(vault.total_debt().unwrap(), 0);
    assert_eq!(out.bad_debt, 100_000_000);
    assert_eq!(out.bad_debt_repaid, 8_000_000);
    assert_eq!(protocol.bad_debt_nusd, 92_000_000);
    assert_eq!(protocol.total_debt, 0);
}

#[test]
fn full_liquidation_cancels_fees_without_recording_them_as_bad_debt() {
    let mut protocol = ProtocolAccounting::new();
    protocol.insurance_fund_nusd = 50_000_000;
    protocol.total_uncollected_fees = 10_000_000;
    let mut vault = Vault {
        collateral_raw: 10_800,
        principal_debt: 100_000_000,
        accrued_fee: 10_000_000,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        1_080_000,
        100 * PRICE_SCALE,
        6,
        params,
        1_000_000,
    )
    .unwrap();

    assert!(out.full_liquidation);
    assert_eq!(out.fee_paid, 1_000_000);
    assert_eq!(out.fee_cancelled, 9_000_000);
    assert_eq!(out.insurance_nusd_burned, 50_000_000);
    assert_eq!(out.bad_debt, 50_000_000);
    assert_eq!(out.bad_debt_repaid, 1_080_000);
    assert_eq!(protocol.bad_debt_nusd, 48_920_000);
    assert_eq!(protocol.realized_revenue_for_stakers, 0);
    assert_eq!(protocol.total_uncollected_fees, 0);
    assert_eq!(protocol.total_debt, 0);
    assert_eq!(vault.total_debt().unwrap(), 0);
}

#[test]
fn mixed_cdp_operations_preserve_protocol_debt_invariants() {
    let mut protocol = ProtocolAccounting::new();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        protocol_debt_cap: 2_000_000_000,
        collateral_debt_cap: 2_000_000_000,
        ..CollateralParams::default()
    };
    let mut partial_vault = Vault {
        collateral_raw: 10_000_000,
        principal_debt: 0,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    let mut full_vault = Vault {
        collateral_raw: 5_000_000,
        principal_debt: 0,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    let mut repaid_vault = Vault {
        collateral_raw: 3_000_000,
        principal_debt: 0,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };

    borrow(
        &mut partial_vault,
        &mut protocol,
        1_000_000_000,
        params,
        450_000_000,
    )
    .unwrap();
    borrow(
        &mut full_vault,
        &mut protocol,
        500_000_000,
        params,
        200_000_000,
    )
    .unwrap();
    borrow(
        &mut repaid_vault,
        &mut protocol,
        300_000_000,
        params,
        100_000_000,
    )
    .unwrap();
    assert_protocol_matches_vaults(protocol, &[partial_vault, full_vault, repaid_vault]);

    accrue_stability_fee(
        &mut partial_vault,
        &mut protocol,
        DEFAULT_STABILITY_FEE_BPS,
        SECONDS_PER_YEAR as i64,
    )
    .unwrap();
    accrue_stability_fee(
        &mut full_vault,
        &mut protocol,
        DEFAULT_STABILITY_FEE_BPS,
        SECONDS_PER_YEAR as i64,
    )
    .unwrap();
    accrue_stability_fee(
        &mut repaid_vault,
        &mut protocol,
        DEFAULT_STABILITY_FEE_BPS,
        SECONDS_PER_YEAR as i64,
    )
    .unwrap();
    assert_protocol_matches_vaults(protocol, &[partial_vault, full_vault, repaid_vault]);

    let repay_outcome = repay(&mut repaid_vault, &mut protocol, 2_000_000).unwrap();
    assert_eq!(repay_outcome.fee_paid, 1_500_000);
    assert_eq!(repay_outcome.principal_paid, 500_000);
    assert_protocol_matches_vaults(protocol, &[partial_vault, full_vault, repaid_vault]);

    let partial = liquidate(
        &mut partial_vault,
        &mut protocol,
        800_000_000,
        80 * PRICE_SCALE,
        6,
        params,
        400_000_000,
    )
    .unwrap();
    assert!(!partial.full_liquidation);
    assert_eq!(partial.repaid_debt, 228_375_000);
    assert!(partial_vault.total_debt().unwrap() > 0);
    assert!(partial_vault.collateral_raw > 0);
    assert_protocol_matches_vaults(protocol, &[partial_vault, full_vault, repaid_vault]);

    let full = liquidate(
        &mut full_vault,
        &mut protocol,
        25_000_000,
        5 * PRICE_SCALE,
        6,
        params,
        100_000_000,
    )
    .unwrap();
    assert!(full.full_liquidation);
    assert!(full.insurance_nusd_burned > 0);
    assert!(full.bad_debt > 0);
    assert_eq!(full_vault.total_debt().unwrap(), 0);
    assert_eq!(full_vault.collateral_raw, 0);
    assert_eq!(protocol.insurance_fund_nusd, 0);
    assert_protocol_matches_vaults(protocol, &[partial_vault, full_vault, repaid_vault]);
}

#[test]
fn full_liquidation_closes_underwater_vault_and_tracks_bad_debt() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_000_000_000,
        principal_debt: 400_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        200_000_000,
        20 * PRICE_SCALE,
        8,
        params,
        190_000_000,
    )
    .unwrap();

    assert!(out.full_liquidation);
    assert_eq!(out.repaid_debt, 190_000_000);
    assert_eq!(vault.collateral_raw, 0);
    assert_eq!(vault.total_debt().unwrap(), 0);
    assert_eq!(out.bad_debt, 210_000_000);
    assert_eq!(out.bad_debt_repaid, 10_000_000);
    assert_eq!(protocol.bad_debt_nusd, 200_000_000);
    assert_eq!(protocol.total_debt, 0);
}

#[test]
fn full_liquidation_uses_insurance_before_recording_bad_debt() {
    let mut protocol = ProtocolAccounting::new();
    let mut vault = Vault {
        collateral_raw: 1_000_000_000,
        principal_debt: 400_000_000,
        accrued_fee: 0,
        last_accrual_ts: 0,
    };
    protocol.total_debt = vault.total_debt().unwrap();
    protocol.insurance_fund_nusd = 50_000_000;
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        ..CollateralParams::default()
    };

    let out = liquidate(
        &mut vault,
        &mut protocol,
        200_000_000,
        20 * PRICE_SCALE,
        8,
        params,
        190_000_000,
    )
    .unwrap();

    assert_eq!(out.insurance_nusd_burned, 50_000_000);
    assert_eq!(out.bad_debt, 160_000_000);
    assert_eq!(out.bad_debt_repaid, 10_000_000);
    assert_eq!(protocol.insurance_fund_nusd, 0);
    assert_eq!(protocol.bad_debt_nusd, 150_000_000);
    assert_eq!(vault.total_debt().unwrap(), 0);
}

#[test]
fn liquidation_grid_preserves_collateral_and_debt_accounting() {
    let params = CollateralParams {
        borrow_ltv_bps: 4_500,
        liquidation_threshold_bps: 5_500,
        liquidation_penalty_bps: 800,
        close_factor_bps: 5_000,
        protocol_debt_cap: 10_000_000_000,
        collateral_debt_cap: 10_000_000_000,
        ..CollateralParams::default()
    };

    for collateral_raw in [500_000_u128, 1_000_000, 5_000_000, 10_000_000] {
        for principal in [50_000_000_u128, 100_000_000, 250_000_000] {
            for accrued_fee in [0_u128, 250_000, 1_000_000] {
                for price_e8 in [5 * PRICE_SCALE, 12 * PRICE_SCALE, 80 * PRICE_SCALE] {
                    let collateral_value =
                        collateral_value_usd(collateral_raw, 6, price_e8).unwrap();
                    if health_factor_bps(
                        collateral_value,
                        principal + accrued_fee,
                        params.liquidation_threshold_bps,
                    )
                    .unwrap()
                        >= BPS_DENOMINATOR
                    {
                        continue;
                    }

                    let mut vault = Vault {
                        collateral_raw,
                        principal_debt: principal,
                        accrued_fee,
                        last_accrual_ts: 0,
                    };
                    let mut protocol = ProtocolAccounting::new();
                    protocol.total_debt = vault.total_debt().unwrap();
                    protocol.total_uncollected_fees = accrued_fee;
                    protocol.insurance_fund_nusd = principal / 10;

                    let debt_before = vault.total_debt().unwrap();
                    let collateral_before = vault.collateral_raw;
                    let insurance_before = protocol.insurance_fund_nusd;
                    let requested_repay = debt_before / 2 + 1;
                    let result = liquidate(
                        &mut vault,
                        &mut protocol,
                        collateral_value,
                        price_e8,
                        6,
                        params,
                        requested_repay,
                    );

                    let Ok(out) = result else {
                        continue;
                    };
                    let seized = out.keeper_collateral_raw + out.insurance_collateral_raw;
                    let paid_or_written_off = out.fee_paid
                        + out.principal_paid
                        + out.insurance_nusd_burned
                        + out.fee_cancelled
                        + out.bad_debt;

                    assert!(seized <= collateral_before);
                    assert_eq!(vault.collateral_raw, collateral_before - seized);
                    assert_eq!(protocol.total_debt, vault.total_debt().unwrap());
                    assert_eq!(protocol.total_uncollected_fees, vault.accrued_fee);
                    assert_eq!(
                        debt_before - vault.total_debt().unwrap(),
                        paid_or_written_off
                    );
                    assert!(out.insurance_nusd_burned <= insurance_before);
                    assert_eq!(
                        protocol.bad_debt_nusd,
                        out.bad_debt.saturating_sub(out.bad_debt_repaid)
                    );
                    assert!(out.staker_penalty_nusd <= out.repaid_debt * 800 / BPS_DENOMINATOR + 1);
                }
            }
        }
    }
}

#[test]
fn bad_debt_can_be_recapitalized_after_surplus_arrives() {
    let mut protocol = ProtocolAccounting::new();
    protocol.bad_debt_nusd = 160_000_000;

    assert_eq!(protocol.cover_bad_debt(0), Err(NestError::InvalidParameter));
    assert_eq!(
        protocol.cover_bad_debt(160_000_001),
        Err(NestError::InvalidParameter)
    );

    protocol.cover_bad_debt(40_000_000).unwrap();
    assert_eq!(protocol.bad_debt_nusd, 120_000_000);

    protocol.cover_bad_debt(120_000_000).unwrap();
    assert_eq!(protocol.bad_debt_nusd, 0);
}

#[test]
fn staking_vests_revenue_and_enforces_cooldown() {
    let mut pool = StakingPool::default();
    let shares = pool.stake(100_000_000, 0, 0).unwrap();
    assert_eq!(shares, 100_000_000);

    pool.harvest(7_000_000, 10).unwrap();
    assert_eq!(pool.accounted_assets().unwrap(), 100_000_000);

    pool.sync_vesting(10 + DEFAULT_REVENUE_VESTING_SECONDS / 2)
        .unwrap();
    assert!(pool.accounted_assets().unwrap() > 100_000_000);
    assert!(pool.accounted_assets().unwrap() < 107_000_000);

    let pending = pool.request_unstake(10_000_000, 20).unwrap();
    assert_eq!(
        pool.complete_unstake(pending, 20 + DEFAULT_COOLDOWN_SECONDS - 1),
        Err(NestError::CooldownActive)
    );
    let assets = pool
        .complete_unstake(pending, 20 + DEFAULT_COOLDOWN_SECONDS)
        .unwrap();
    assert!(assets >= 10_000_000);
}

#[test]
fn staking_grid_never_over_redeems_accounted_assets() {
    for first_stake in [1_000_000_u128, 100_000_000, 1_000_000_000] {
        for second_stake in [1_u128, first_stake / 3 + 1, first_stake] {
            for revenue in [0_u128, 1, first_stake / 10] {
                for loss in [0_u128, 1, first_stake / 20] {
                    let mut pool = StakingPool::default();
                    let first_shares = pool.stake(first_stake, 0, 0).unwrap();
                    let second_shares = pool.stake(second_stake, 0, 1).unwrap();
                    assert!(second_shares <= second_stake);

                    if revenue > 0 {
                        pool.harvest(revenue, 2).unwrap();
                        pool.sync_vesting(2 + DEFAULT_REVENUE_VESTING_SECONDS)
                            .unwrap();
                    }
                    if loss > 0 && loss < pool.staking_vault_nusd {
                        pool.realize_loss(loss).unwrap();
                    }

                    let accounted_before = pool.accounted_assets().unwrap();
                    let pending = pool.request_unstake(first_shares, 3).unwrap();
                    let assets = pool
                        .complete_unstake(pending, 3 + DEFAULT_COOLDOWN_SECONDS)
                        .unwrap();

                    assert!(assets <= accounted_before);
                    assert_eq!(pool.total_shares, second_shares);
                    assert_eq!(pool.accounted_assets().unwrap(), accounted_before - assets);
                    assert_staking_pool_invariants(pool, second_shares, 0);
                }
            }
        }
    }
}

#[test]
fn staking_complete_unstake_updates_pool_at_exact_cooldown_boundary() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();

    let pending = pool.request_unstake(25_000_000, 10).unwrap();
    assert_eq!(pool.total_shares, 100_000_000);
    assert_eq!(pool.staking_vault_nusd, 100_000_000);

    assert_eq!(
        pool.complete_unstake(pending, 10 + DEFAULT_COOLDOWN_SECONDS - 1),
        Err(NestError::CooldownActive)
    );
    assert_eq!(pool.total_shares, 100_000_000);
    assert_eq!(pool.staking_vault_nusd, 100_000_000);

    let assets = pool
        .complete_unstake(pending, 10 + DEFAULT_COOLDOWN_SECONDS)
        .unwrap();
    assert_eq!(assets, 25_000_000);
    assert_eq!(pool.total_shares, 75_000_000);
    assert_eq!(pool.staking_vault_nusd, 75_000_000);
}

#[test]
fn staking_rejects_zero_harvest() {
    let mut pool = StakingPool::default();
    assert_eq!(
        pool.harvest(1_000_000, 10),
        Err(NestError::InvalidParameter)
    );
    pool.stake(100_000_000, 0, 0).unwrap();
    assert_eq!(pool.harvest(0, 10), Err(NestError::InvalidParameter));
}

#[test]
fn staking_rejects_dust_bootstrap_and_bounds_first_staker_rounding() {
    let mut pool = StakingPool::default();
    assert_eq!(
        pool.stake(MIN_INITIAL_STAKE_NUSD - 1, 0, 0),
        Err(NestError::InvalidParameter)
    );

    let attacker_stake = MIN_INITIAL_STAKE_NUSD;
    let donated_revenue = 50_000_000;
    let attacker_shares = pool.stake(attacker_stake, 0, 0).unwrap();
    pool.harvest(donated_revenue, 1).unwrap();
    let victim_shares = pool.stake(100_000_000, 0, 2).unwrap();
    assert!(victim_shares > 1_900_000);

    let vesting_done = 1 + DEFAULT_REVENUE_VESTING_SECONDS;
    pool.sync_vesting(vesting_done).unwrap();
    let pending = pool.request_unstake(attacker_shares, vesting_done).unwrap();
    let attacker_redeemed = pool
        .complete_unstake(pending, vesting_done + DEFAULT_COOLDOWN_SECONDS)
        .unwrap();

    let attacker_cost = attacker_stake + donated_revenue;
    let rounding_bound = attacker_cost / attacker_shares + 1;
    assert!(attacker_redeemed.saturating_sub(attacker_cost) <= rounding_bound);
}

#[test]
fn staking_losses_socialize_to_pending_shares() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();
    let pending = pool.request_unstake(50_000_000, 1).unwrap();
    assert_eq!(pool.total_shares, 100_000_000);
    pool.realize_loss(20_000_000).unwrap();
    let assets = pool
        .complete_unstake(pending, 1 + DEFAULT_COOLDOWN_SECONDS)
        .unwrap();
    assert_eq!(assets, 40_000_000);
}

#[test]
fn staking_pending_shares_remain_in_revenue_denominator() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();
    let pending = pool.request_unstake(50_000_000, 1).unwrap();
    assert_eq!(pool.total_shares, 100_000_000);

    pool.harvest(10_000_000, 2).unwrap();
    let assets = pool
        .complete_unstake(pending, 2 + DEFAULT_REVENUE_VESTING_SECONDS)
        .unwrap();

    assert_eq!(assets, 55_000_000);
    assert_eq!(pool.total_shares, 50_000_000);
    assert_eq!(pool.staking_vault_nusd, 55_000_000);
}

#[test]
fn staking_rejects_harvest_while_previous_revenue_is_vesting() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();
    pool.harvest(10_000_000, 10).unwrap();

    assert_eq!(
        pool.harvest(1, 10 + DEFAULT_REVENUE_VESTING_SECONDS / 2),
        Err(NestError::InvalidParameter)
    );

    pool.harvest(1_000_000, 10 + DEFAULT_REVENUE_VESTING_SECONDS)
        .unwrap();
    assert_eq!(pool.unvested_revenue, 1_000_000);
}

#[test]
fn staking_new_stakers_do_not_capture_unvested_revenue() {
    let mut pool = StakingPool::default();
    let first_shares = pool.stake(100_000_000, 0, 0).unwrap();
    assert_eq!(first_shares, 100_000_000);

    pool.harvest(100_000_000, 10).unwrap();
    assert_eq!(pool.accounted_assets().unwrap(), 100_000_000);
    assert_eq!(pool.stake_entry_assets(0).unwrap(), 200_000_000);

    let second_shares = pool.stake(100_000_000, 0, 10).unwrap();
    assert_eq!(second_shares, 50_000_000);
    assert_eq!(pool.total_shares, 150_000_000);

    pool.sync_vesting(10 + DEFAULT_REVENUE_VESTING_SECONDS)
        .unwrap();
    let accounted = pool.accounted_assets().unwrap();
    assert_eq!(accounted, 300_000_000);
    assert_eq!(
        mul_div_down(first_shares, accounted, pool.total_shares).unwrap(),
        200_000_000
    );
    assert_eq!(
        mul_div_down(second_shares, accounted, pool.total_shares).unwrap(),
        100_000_000
    );
}

#[test]
fn staking_new_stakers_do_not_capture_unharvested_revenue() {
    let mut pool = StakingPool::default();
    let first_shares = pool.stake(100_000_000, 0, 0).unwrap();

    let second_shares = pool.stake(100_000_000, 100_000_000, 10).unwrap();
    assert_eq!(second_shares, 50_000_000);

    pool.harvest(100_000_000, 10).unwrap();
    pool.sync_vesting(10 + DEFAULT_REVENUE_VESTING_SECONDS)
        .unwrap();

    let accounted = pool.accounted_assets().unwrap();
    assert_eq!(accounted, 300_000_000);
    assert_eq!(
        mul_div_down(first_shares, accounted, pool.total_shares).unwrap(),
        200_000_000
    );
    assert_eq!(
        mul_div_down(second_shares, accounted, pool.total_shares).unwrap(),
        100_000_000
    );
}

#[test]
fn staking_rejects_new_stake_when_existing_shares_have_zero_assets() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();
    pool.realize_loss(100_000_000).unwrap();

    assert_eq!(pool.total_shares, 100_000_000);
    assert_eq!(pool.staking_vault_nusd, 0);
    assert_eq!(
        pool.stake(100_000_000, 0, 1),
        Err(NestError::InvalidParameter)
    );
}

#[test]
fn final_pending_unstake_waits_for_unvested_revenue() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();
    let pending = pool.request_unstake(100_000_000, 1).unwrap();

    let late_harvest_ts = 1 + DEFAULT_COOLDOWN_SECONDS - 10;
    pool.harvest(10_000_000, late_harvest_ts).unwrap();

    assert_eq!(
        pool.complete_unstake(pending, 1 + DEFAULT_COOLDOWN_SECONDS),
        Err(NestError::CooldownActive)
    );

    let assets = pool
        .complete_unstake(pending, late_harvest_ts + DEFAULT_REVENUE_VESTING_SECONDS)
        .unwrap();
    assert_eq!(assets, 110_000_000);
    assert_eq!(pool.total_shares, 0);
    assert_eq!(pool.staking_vault_nusd, 0);
}

#[test]
fn mixed_staking_operations_preserve_share_and_asset_invariants() {
    let mut pool = StakingPool::default();
    let mut active_shares = pool.stake(100_000_000, 0, 0).unwrap();
    let mut pending_shares = 0;
    assert_eq!(active_shares, 100_000_000);
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    pool.harvest(20_000_000, 10).unwrap();
    assert_eq!(pool.accounted_assets().unwrap(), 100_000_000);
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    let half_vested_ts = 10 + DEFAULT_REVENUE_VESTING_SECONDS / 2;
    pool.sync_vesting(half_vested_ts).unwrap();
    assert_eq!(pool.accounted_assets().unwrap(), 110_000_000);
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    let second_stake_shares = pool.stake(55_000_000, 0, half_vested_ts).unwrap();
    assert_eq!(second_stake_shares, 45_833_333);
    active_shares += second_stake_shares;
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    let request_ts = half_vested_ts + 1;
    let pending = pool.request_unstake(30_000_000, request_ts).unwrap();
    active_shares -= pending.shares;
    pending_shares += pending.shares;
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    pool.realize_loss(15_000_000).unwrap();
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    let first_vesting_end_ts = 10 + DEFAULT_REVENUE_VESTING_SECONDS;
    pool.harvest(30_000_000, first_vesting_end_ts).unwrap();
    assert_staking_pool_invariants(pool, active_shares, pending_shares);

    assert_eq!(
        pool.complete_unstake(pending, request_ts + DEFAULT_COOLDOWN_SECONDS),
        Err(NestError::CooldownActive)
    );

    let second_vesting_end_ts = first_vesting_end_ts + DEFAULT_REVENUE_VESTING_SECONDS;
    let assets = pool
        .complete_unstake(pending, second_vesting_end_ts)
        .unwrap();
    assert_eq!(assets, 39_085_714);
    pending_shares -= pending.shares;
    assert_staking_pool_invariants(pool, active_shares, pending_shares);
    assert_eq!(pool.total_shares, 115_833_333);
    assert_eq!(pool.staking_vault_nusd, 150_914_286);
    assert_eq!(pool.unvested_revenue, 0);
}

#[test]
fn kamino_profit_slice_allows_positive_profit_without_touching_principal() {
    let proof =
        verify_kamino_profit_slice(10_000_000, 100_000_000, 10_000_000, 100_000_000).unwrap();

    assert_eq!(proof.remaining_value_lower_bound, 100_000_000);
}

#[test]
fn kamino_profit_slice_rejects_no_profit_redeem() {
    assert_eq!(
        verify_kamino_profit_slice(10_000_000, 90_000_000, 10_000_000, 100_000_000),
        Err(NestError::PrincipalNotCovered)
    );
}

#[test]
fn kamino_profit_slice_uses_floor_rounding_for_remaining_value() {
    assert_eq!(
        verify_kamino_profit_slice(3, 29, 10, 97),
        Err(NestError::PrincipalNotCovered)
    );

    let proof = verify_kamino_profit_slice(3, 29, 10, 96).unwrap();
    assert_eq!(proof.remaining_value_lower_bound, 96);
}

#[test]
fn kamino_profit_slice_rejects_zero_inputs() {
    assert_eq!(
        verify_kamino_profit_slice(0, 100, 10, 100),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        verify_kamino_profit_slice(10, 0, 10, 100),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        verify_kamino_profit_slice(10, 100, 0, 100),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        verify_kamino_profit_slice(10, 100, 10, 0),
        Err(NestError::InvalidParameter)
    );
}

#[test]
fn staking_rejects_negative_timestamps() {
    let mut pool = StakingPool::default();

    assert_eq!(
        pool.stake(100_000_000, 0, -1),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        pool.harvest(1_000_000, -1),
        Err(NestError::InvalidParameter)
    );

    pool.stake(100_000_000, 0, 0).unwrap();
    assert_eq!(
        pool.request_unstake(1_000_000, -1),
        Err(NestError::InvalidParameter)
    );
    assert_eq!(
        pool.complete_unstake(
            PendingWithdrawal {
                shares: 1_000_000,
                request_ts: -1,
            },
            DEFAULT_COOLDOWN_SECONDS,
        ),
        Err(NestError::InvalidParameter)
    );
}

#[test]
fn staking_rejects_cooldown_unlock_timestamp_overflow() {
    let mut pool = StakingPool::default();
    pool.stake(100_000_000, 0, 0).unwrap();

    assert_eq!(
        pool.complete_unstake(
            PendingWithdrawal {
                shares: 1_000_000,
                request_ts: i64::MAX,
            },
            i64::MAX,
        ),
        Err(NestError::MathOverflow)
    );
}
