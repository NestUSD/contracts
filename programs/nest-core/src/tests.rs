    #[test]
    fn scales_lazer_prices_to_e8_with_floor_for_price_and_ceiling_for_confidence() {
        let price = lazer_price_to_oracle_price(
            pyth_lazer_solana_contract::protocol::PriceFeedId(7),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(1_234_567_899).unwrap(),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(101).unwrap(),
            -10,
            42,
        )
        .unwrap();
        let mut expected_feed_key = [0_u8; 32];
        expected_feed_key[..4].copy_from_slice(&7_u32.to_le_bytes());

        assert_eq!(price.feed_id, expected_feed_key);
        assert_eq!(price.price_e8, 12_345_678);
        assert_eq!(price.confidence_e8, 2);
        assert_eq!(price.publish_time, 42);
    }

    #[test]
    fn scales_lazer_prices_to_e8_when_exponent_needs_multiplication() {
        let price = lazer_price_to_oracle_price(
            pyth_lazer_solana_contract::protocol::PriceFeedId(7),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(123_456).unwrap(),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(7).unwrap(),
            -6,
            42,
        )
        .unwrap();

        assert_eq!(price.price_e8, 12_345_600);
        assert_eq!(price.confidence_e8, 700);
    }

    #[test]
    fn rejects_non_positive_lazer_prices() {
        assert!(pyth_lazer_solana_contract::protocol::Price::from_mantissa(0).is_err());

        assert!(lazer_price_to_oracle_price(
            pyth_lazer_solana_contract::protocol::PriceFeedId(7),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(-1).unwrap(),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(1).unwrap(),
            -8,
            42,
        )
        .is_err());
    }

    #[test]
    fn rejects_unrepresentable_oracle_scaling() {
        assert!(scale_oracle_price_to_e8(1, 31, false).is_err());
        assert!(scale_oracle_price_to_e8(1, -47, false).is_err());
    }

    #[test]
    fn snapshot_pricing_uses_configured_xstock_staleness() {
        let mut config = lazer_config(1);
        config.max_staleness_seconds = 5;

        assert_eq!(
            policy(config.xstock_usd_feed_id, &config)
                .unwrap()
                .max_staleness_seconds,
            5
        );
    }

    #[test]
    fn reserved_oracle_slots_preserve_deployed_account_sizes() {
        assert_eq!(CollateralConfig::INIT_SPACE, 393);
        assert_eq!(OracleSnapshot::INIT_SPACE, 242);
    }

    #[test]
    fn collateral_coverage_breaker_trips_for_shortfall_or_frozen_custody() {
        assert!(!collateral_vault_coverage_broken(1_000, 1_000, false));
        assert!(!collateral_vault_coverage_broken(1_001, 1_000, false));
        assert!(collateral_vault_coverage_broken(999, 1_000, false));
        assert!(collateral_vault_coverage_broken(1_000, 1_000, true));
        assert!(collateral_vault_coverage_broken(1_001, 1_000, true));
    }

    #[test]
    fn signed_price_message_uses_stable_borsh_layout() {
        let mut feed_id = [0_u8; 32];
        for (index, byte) in feed_id.iter_mut().enumerate() {
            *byte = index as u8;
        }
        let payload = SignedPricePayload {
            magic: SIGNED_PRICE_MAGIC,
            version: SIGNED_PRICE_VERSION,
            feed_id,
            price_e8: 12_345_678_901,
            confidence_e8: 123_456,
            publish_time: 1_782_700_000,
            expires_at: 1_782_700_120,
        };

        let message = signed_price_message(&payload).unwrap();
        assert_eq!(message.len(), 73);
        assert_eq!(&message[0..8], b"NESTPRC1");
        assert_eq!(message[8], 1);
        assert_eq!(&message[9..41], &feed_id);
        assert_eq!(
            u64::from_le_bytes(message[41..49].try_into().unwrap()),
            payload.price_e8
        );
        assert_eq!(
            u64::from_le_bytes(message[49..57].try_into().unwrap()),
            payload.confidence_e8
        );
        assert_eq!(
            i64::from_le_bytes(message[57..65].try_into().unwrap()),
            payload.publish_time
        );
        assert_eq!(
            i64::from_le_bytes(message[65..73].try_into().unwrap()),
            payload.expires_at
        );
    }

    fn lazer_config(feed_id: u32) -> CollateralConfig {
        let mut xstock_usd_feed_id = [0_u8; 32];
        xstock_usd_feed_id[..4].copy_from_slice(&feed_id.to_le_bytes());
        CollateralConfig {
            protocol: Pubkey::new_unique(),
            collateral_mint: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            insurance_collateral_vault: Pubkey::new_unique(),
            token_program: Pubkey::new_unique(),
            symbol: [0; 16],
            collateral_decimals: 6,
            xstock_usd_feed_id,
            reserved_underlying_usd_feed_id: [0; 32],
            reserved_redemption_rate_feed_id: [0; 32],
            borrow_ltv_bps: 4_500,
            liquidation_threshold_bps: 5_500,
            liquidation_penalty_bps: 800,
            close_factor_bps: 5_000,
            max_confidence_bps: 200,
            max_staleness_seconds: 120,
            reserved_closed_market_max_staleness_seconds: 0,
            reserved_underlying_closed_market_max_staleness_seconds: 0,
            reserved_closed_market_haircut_bps: 0,
            per_vault_debt_cap: 1,
            protocol_debt_cap: 1,
            deposit_cap_raw: 1,
            total_debt: 0,
            total_deposits_raw: 0,
            deposits_paused: false,
            borrows_paused: false,
            withdraws_paused: false,
            bump: 255,
        }
    }

    fn decode_hex(input: &str) -> Vec<u8> {
        let normalized: String = input
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        assert_eq!(normalized.len() % 2, 0, "hex fixture length");
        (0..normalized.len())
            .step_by(2)
            .map(|index| u8::from_str_radix(&normalized[index..index + 2], 16).unwrap())
            .collect()
    }

    fn sample_lazer_payload() -> Vec<u8> {
        let message = "b9011a82c7887f3aaa5845b20d6bf5ca6609953b57650fa4579a4b4d34a4980ba608a9f76a825a446\
            f3d6c1fd9daca1c5e3fc46980f14ef89c1a886c6e9e5c510872d30f80efc1f480c5615af3fb673d422\
            87e993da9fbc3506b6e41dfa32950820c2e6c620075d3c7934077d115064b06000301010000000d009\
            032fc171b060000014e3a0cff1a06000002bcda8a211b06000003120004f8ff05fc0b7159000000000\
            600070008000900000aa06616362e0600000b804f93312e0600000c014077d115064b0600";
        pyth_lazer_solana_contract::protocol::message::SolanaMessage::deserialize_slice(
            &decode_hex(message),
        )
        .unwrap()
        .payload
    }

    #[test]
    fn parses_lazer_solana_payload_with_little_endian_feed_id() {
        let (price, source_publish_time_us) =
            read_lazer_price(&sample_lazer_payload(), &lazer_config(1)).unwrap();
        let mut expected_feed_key = [0_u8; 32];
        expected_feed_key[..4].copy_from_slice(&1_u32.to_le_bytes());

        assert_eq!(price.feed_id, expected_feed_key);
        assert_eq!(price.price_e8, 6_713_436_287_632);
        assert_eq!(price.confidence_e8, 1_500_580_860);
        assert_eq!(price.publish_time, 1_771_339_368);
        assert_eq!(source_publish_time_us, 1_771_339_368_200_000);
    }

    #[test]
    fn rejects_lazer_config_with_nonzero_trailing_feed_bytes() {
        let mut config = lazer_config(1);
        config.xstock_usd_feed_id[4] = 1;

        assert!(lazer_feed_id_from_config(&config).is_err());
        assert!(read_lazer_price(&sample_lazer_payload(), &config).is_err());
    }

    #[test]
    fn collateral_feed_config_accepts_lazer_or_signed_xstock_feed_id() {
        let mut lazer_feed = [0_u8; 32];
        lazer_feed[..4].copy_from_slice(&1_843_u32.to_le_bytes());
        let signed_feed = [7_u8; 32];

        assert!(validate_collateral_feed_config(&lazer_feed).is_ok());
        assert!(validate_collateral_feed_config(&signed_feed).is_ok());
        assert!(validate_collateral_feed_config(&[0; 32]).is_err());
    }

    #[test]
    fn signed_oracle_feed_config_rejects_lazer_namespace() {
        let mut lazer_feed = [0_u8; 32];
        lazer_feed[..4].copy_from_slice(&1_843_u32.to_le_bytes());
        let signed_feed = [7_u8; 32];

        assert!(validate_lazer_feed_config(&lazer_feed).is_ok());
        assert!(validate_signed_feed_config(&lazer_feed).is_err());
        assert!(validate_lazer_feed_config(&signed_feed).is_err());
        assert!(validate_signed_feed_config(&signed_feed).is_ok());
    }

    #[test]
    fn oracle_snapshot_rejects_older_or_conflicting_updates() {
        let current_price = OraclePriceAccount {
            feed_id: [7; 32],
            price_e8: 100 * domain::PRICE_SCALE as u64,
            confidence_e8: 10,
            publish_time: 1_000,
        };
        let oracle = OracleSnapshot {
            protocol: Pubkey::new_unique(),
            collateral_config: Pubkey::new_unique(),
            xstock_usd: current_price,
            reserved_underlying_usd: inactive_oracle_price([0; 32]),
            reserved_redemption_rate: inactive_oracle_price([0; 32]),
            reserved_market_state: MarketStateAccount::Regular,
            source_publish_time_us: 1_000_500_000,
            bump: 255,
        };

        assert!(require_monotonic_oracle_update(&oracle, current_price, 1_000_499_999).is_err());
        assert!(require_monotonic_oracle_update(&oracle, current_price, 1_000_500_000).is_ok());

        let mut conflicting_price = current_price;
        conflicting_price.price_e8 += 1;
        assert!(
            require_monotonic_oracle_update(&oracle, conflicting_price, 1_000_500_000).is_err()
        );
        assert!(require_monotonic_oracle_update(&oracle, conflicting_price, 1_000_500_001).is_ok());

        conflicting_price.feed_id = [8; 32];
        assert!(require_monotonic_oracle_update(&oracle, conflicting_price, 1).is_ok());
    }

    fn protocol_for_psm_outflow_test() -> Protocol {
        Protocol {
            authority: Pubkey::new_unique(),
            liquidation_authority: Pubkey::new_unique(),
            nusd_mint: Pubkey::new_unique(),
            usdc_mint: Pubkey::new_unique(),
            psm_usdc_vault: Pubkey::new_unique(),
            insurance_nusd_vault: Pubkey::new_unique(),
            staker_revenue_nusd_vault: Pubkey::new_unique(),
            staker_revenue_authority: Pubkey::new_unique(),
            total_debt: 0,
            total_uncollected_fees: 0,
            realized_revenue_for_stakers: 0,
            insurance_fund_nusd: 0,
            bad_debt_nusd: 0,
            psm_usdc_liabilities: 1_000,
            psm_nusd_supply: 1_000,
            psm_idle_usdc: 1_000,
            psm_kamino_deployed_usdc: 0,
            psm_kamino_collateral_vault: Pubkey::default(),
            psm_cap: 1_000,
            protocol_debt_cap: 0,
            stability_fee_apr_bps: 0,
            psm_swap_in_fee_bps: 0,
            psm_swap_out_fee_bps: 0,
            kamino_program_id: KLEND_PROGRAM_ID,
            insurance_target_bps: 0,
            insurance_fee_share_bps: 0,
            staker_revenue_accounting_initialized: true,
            paused: false,
            bump: 255,
            protocol_revenue_nusd_vault: Pubkey::new_unique(),
            realized_revenue_for_protocol: 0,
            staker_target_revenue_due: 0,
            staker_target_last_accrual_ts: 0,
            staker_target_apr_bps: 0,
            staker_capacity_kamino_apr_bps: 0,
            pending_liquidation_principal: 0,
            pending_liquidation_fees: 0,
            psm_outflow_window_start_ts: 0,
            psm_outflow_window_usdc: 0,
            psm_outflow_limit_usdc: 100,
            psm_outflow_limit_bps: 0,
            psm_outflow_window_seconds: 60,
            psm_outflow_circuit_breaker_enabled: true,
            psm_outflow_window_basis_usdc: 0,
            buyback_authority: Pubkey::default(),
            buyback_usdc_account: Pubkey::default(),
        }
    }

    #[test]
    fn psm_outflow_breaker_allows_exact_limit_and_rejects_excess() {
        let mut protocol = protocol_for_psm_outflow_test();

        record_psm_outflow(&mut protocol, 100, 1_000, 10).unwrap();
        assert!(!protocol.paused);
        assert_eq!(protocol.psm_outflow_window_usdc, 100);

        assert!(record_psm_outflow(&mut protocol, 1, 900, 11).is_err());
        assert!(!protocol.paused);
        assert_eq!(protocol.psm_outflow_window_usdc, 100);
    }

    #[test]
    fn psm_redemption_is_blocked_until_bad_debt_is_covered() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.bad_debt_nusd = 1;
        assert!(ix::psm::require_psm_redemption_solvent(&protocol).is_err());

        protocol.bad_debt_nusd = 0;
        assert!(ix::psm::require_psm_redemption_solvent(&protocol).is_ok());
    }

    #[test]
    fn lossy_full_kamino_redemption_clears_deployed_principal_and_records_bad_debt() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.psm_kamino_deployed_usdc = 100;
        let idle_before = protocol.psm_idle_usdc;

        let redemption =
            ix::kamino::record_psm_kamino_redemption(&mut protocol, 100, 100, 80).unwrap();

        assert_eq!(redemption.principal_removed, 100);
        assert_eq!(redemption.principal_received, 80);
        assert_eq!(redemption.principal_loss, 20);
        assert_eq!(protocol.psm_kamino_deployed_usdc, 0);
        assert_eq!(protocol.psm_idle_usdc, idle_before + 80);
        assert_eq!(protocol.bad_debt_nusd, 20);
        assert!(ix::psm::require_psm_redemption_solvent(&protocol).is_err());
    }

    #[test]
    fn partial_kamino_redemption_removes_pro_rata_principal() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.psm_idle_usdc = 10;
        protocol.psm_kamino_deployed_usdc = 100;
        protocol.bad_debt_nusd = 2;

        let redemption =
            ix::kamino::record_psm_kamino_redemption(&mut protocol, 3, 1, 30).unwrap();

        assert_eq!(redemption.principal_removed, 34);
        assert_eq!(redemption.principal_received, 30);
        assert_eq!(redemption.principal_loss, 4);
        assert_eq!(protocol.psm_kamino_deployed_usdc, 66);
        assert_eq!(protocol.psm_idle_usdc, 40);
        assert_eq!(protocol.bad_debt_nusd, 6);
    }

    #[test]
    fn staking_loss_retires_only_recorded_bad_debt() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.bad_debt_nusd = 100;

        ix::revenue::record_staking_loss_absorption(&mut protocol, 40).unwrap();
        assert_eq!(protocol.bad_debt_nusd, 60);
        assert!(ix::revenue::record_staking_loss_absorption(&mut protocol, 61).is_err());
        assert_eq!(protocol.bad_debt_nusd, 60);
        assert!(ix::revenue::record_staking_loss_absorption(&mut protocol, 0).is_err());
    }

    #[test]
    fn staker_target_checkpoints_use_each_intervals_balance() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.staker_target_apr_bps = 1_000;
        protocol.staker_target_last_accrual_ts = 1;
        let half_year = domain::SECONDS_PER_YEAR as i64 / 2;

        sync_staker_target_revenue(&mut protocol, 100_000_000, 1 + half_year).unwrap();
        sync_staker_target_revenue(
            &mut protocol,
            200_000_000,
            1 + domain::SECONDS_PER_YEAR as i64,
        )
        .unwrap();

        assert_eq!(protocol.staker_target_revenue_due, 15_000_000);
    }

    #[test]
    fn protocol_revenue_retires_bad_debt_before_any_distribution() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.paused = true;
        protocol.bad_debt_nusd = 75;
        protocol.staker_target_apr_bps = 600;
        protocol.staker_target_last_accrual_ts = 10;
        protocol.staker_target_revenue_due = 25;

        let allocation =
            apply_protocol_revenue_accounting(&mut protocol, 1_000, 10, 50).unwrap();

        assert_eq!(allocation.bad_debt_repaid, 50);
        assert_eq!(allocation.insurance, 0);
        assert_eq!(allocation.stakers, 0);
        assert_eq!(allocation.protocol, 0);
        assert_eq!(protocol.bad_debt_nusd, 25);
        assert_eq!(protocol.staker_target_revenue_due, 25);
        assert!(protocol.paused);
    }

    #[test]
    fn protocol_revenue_routes_remainder_through_insurance_stakers_and_surplus() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.total_debt = 1_000;
        protocol.bad_debt_nusd = 10;
        protocol.insurance_target_bps = 1_000;
        protocol.insurance_fee_share_bps = 2_000;
        protocol.staker_target_apr_bps = 600;
        protocol.staker_target_last_accrual_ts = 10;
        protocol.staker_target_revenue_due = 30;

        let allocation =
            apply_protocol_revenue_accounting(&mut protocol, 1_000, 10, 100).unwrap();

        assert_eq!(allocation.bad_debt_repaid, 10);
        assert_eq!(allocation.insurance, 18);
        assert_eq!(allocation.stakers, 30);
        assert_eq!(allocation.protocol, 42);
        assert_eq!(protocol.bad_debt_nusd, 0);
        assert_eq!(protocol.insurance_fund_nusd, 18);
        assert_eq!(protocol.staker_target_revenue_due, 0);
        assert_eq!(protocol.realized_revenue_for_stakers, 30);
        assert_eq!(protocol.realized_revenue_for_protocol, 42);
    }

    #[test]
    fn protocol_revenue_skips_funded_insurance_and_fills_staker_target() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.total_debt = 1_000;
        protocol.insurance_target_bps = 1_000;
        protocol.insurance_fee_share_bps = 2_000;
        protocol.insurance_fund_nusd = 100;
        protocol.staker_target_apr_bps = 600;
        protocol.staker_target_last_accrual_ts = 10;
        protocol.staker_target_revenue_due = 100;

        let allocation =
            apply_protocol_revenue_accounting(&mut protocol, 1_000, 10, 40).unwrap();

        assert_eq!(allocation.bad_debt_repaid, 0);
        assert_eq!(allocation.insurance, 0);
        assert_eq!(allocation.stakers, 40);
        assert_eq!(allocation.protocol, 0);
        assert_eq!(protocol.staker_target_revenue_due, 60);
        assert_eq!(protocol.realized_revenue_for_stakers, 40);
    }

    #[test]
    fn protocol_revenue_requires_initialized_accounting_and_nonzero_amount() {
        let mut protocol = protocol_for_psm_outflow_test();
        assert!(apply_protocol_revenue_accounting(&mut protocol, 0, 10, 0).is_err());

        protocol.staker_revenue_accounting_initialized = false;
        assert!(apply_protocol_revenue_accounting(&mut protocol, 0, 10, 1).is_err());
        assert_eq!(protocol.realized_revenue_for_stakers, 0);
        assert_eq!(protocol.realized_revenue_for_protocol, 0);
    }

    #[test]
    fn psm_kamino_target_grid_never_requires_user_triggered_kamino_withdrawal() {
        for idle_usdc in [0_u128, 1, 9, 10, 100_000, 1_000_000, u64::MAX as u128] {
            for deployed_usdc in [0_u128, 1, 9, 10, 100_000, 1_000_000] {
                let total = idle_usdc.checked_add(deployed_usdc).unwrap();
                let expected_target = total
                    .checked_mul(PSM_KAMINO_TARGET_BPS)
                    .unwrap()
                    .checked_div(PSM_KAMINO_BPS_DENOMINATOR)
                    .unwrap();
                let target = psm_kamino_target_deployed(idle_usdc, deployed_usdc).unwrap();
                assert_eq!(target, expected_target);
                assert!(target <= total);

                if deployed_usdc < target {
                    let deploy_delta = target - deployed_usdc;
                    assert!(deploy_delta <= idle_usdc);
                    assert!(idle_usdc - deploy_delta <= total / 10 + 1);
                } else {
                    // When Kamino already holds at least the target, the public
                    // PSM path cannot pull it back; only the admin-gated
                    // Kamino redeem instruction can increase idle liquidity.
                    assert!(deployed_usdc >= target);
                }
            }
        }
    }

    #[test]
    fn liquidation_debt_settlement_bypasses_voluntary_psm_cap() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.pending_liquidation_principal = 250;
        protocol.pending_liquidation_fees = 17;

        ix::liquidation::two_step::record_liquidation_debt_settlement(&mut protocol, 250, 17)
            .unwrap();

        assert_eq!(protocol.pending_liquidation_principal, 0);
        assert_eq!(protocol.pending_liquidation_fees, 0);
        assert_eq!(protocol.psm_usdc_liabilities, 1_250);
        assert_eq!(protocol.psm_nusd_supply, 1_250);
        assert_eq!(protocol.psm_idle_usdc, 1_250);
        assert_eq!(protocol.psm_cap, 1_000);
    }

    #[test]
    fn liquidation_surplus_accounting_bypasses_voluntary_psm_cap_only_when_requested() {
        let mut capped_protocol = protocol_for_psm_outflow_test();
        assert!(apply_realized_psm_yield_accounting(
            &mut capped_protocol,
            1_250,
            250,
            0,
            10,
            true,
            true,
        )
        .is_err());

        let mut liquidation_protocol = protocol_for_psm_outflow_test();
        let (insurance_delta, staker_delta, protocol_delta, minted_nusd) =
            apply_realized_psm_yield_accounting(
                &mut liquidation_protocol,
                1_250,
                250,
                0,
                10,
                false,
                false,
            )
            .unwrap();

        assert_eq!(insurance_delta, 0);
        assert_eq!(staker_delta, 250);
        assert_eq!(protocol_delta, 0);
        assert_eq!(minted_nusd, 250);
        assert_eq!(liquidation_protocol.psm_usdc_liabilities, 1_250);
        assert_eq!(liquidation_protocol.psm_nusd_supply, 1_250);
        assert_eq!(liquidation_protocol.psm_idle_usdc, 1_250);
        assert_eq!(liquidation_protocol.realized_revenue_for_stakers, 250);
    }

    #[test]
    fn staker_revenue_liability_tracks_only_unharvested_revenue() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.realized_revenue_for_stakers = 100;

        assert!(require_staker_revenue_covered(&protocol, 100, 0).is_ok());
        assert!(require_staker_revenue_covered(&protocol, 99, 0).is_err());
        ix::revenue::record_staker_revenue_consumption(&mut protocol, 40).unwrap();
        assert_eq!(protocol.realized_revenue_for_stakers, 60);
        assert!(ix::revenue::record_staker_revenue_consumption(&mut protocol, 61).is_err());

        protocol.staker_revenue_accounting_initialized = false;
        assert!(require_staker_revenue_covered(&protocol, 25, 25).is_ok());
        assert!(require_staker_revenue_covered(&protocol, 24, 25).is_err());
        assert!(ix::revenue::record_staker_revenue_consumption(&mut protocol, 1).is_err());
    }

    #[test]
    fn routed_staker_revenue_increases_outstanding_liability() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.realized_revenue_for_stakers = 10_000;
        apply_realized_psm_yield_accounting(&mut protocol, 1_025, 25, 0, 10, false, false)
            .unwrap();

        assert_eq!(protocol.realized_revenue_for_stakers, 10_025);
    }

    #[test]
    fn legacy_staker_revenue_accounting_requires_paused_one_time_reconciliation() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.staker_revenue_accounting_initialized = false;
        protocol.realized_revenue_for_stakers = 10_000;

        assert!(ix::admin::initialize_staker_revenue_accounting_state(
            &mut protocol,
            75,
            100,
        )
        .is_err());
        protocol.paused = true;
        assert!(ix::admin::initialize_staker_revenue_accounting_state(
            &mut protocol,
            101,
            100,
        )
        .is_err());
        ix::admin::initialize_staker_revenue_accounting_state(&mut protocol, 75, 100).unwrap();
        assert_eq!(protocol.realized_revenue_for_stakers, 75);
        assert!(protocol.staker_revenue_accounting_initialized);
        assert!(ix::admin::initialize_staker_revenue_accounting_state(
            &mut protocol,
            75,
            100,
        )
        .is_err());
    }

    #[test]
    fn liquidation_bad_debt_recovery_bypasses_pause_and_psm_cap() {
        let mut protocol = protocol_for_psm_outflow_test();
        protocol.paused = true;
        protocol.bad_debt_nusd = 300;

        ix::liquidation::two_step::record_liquidation_bad_debt_recovery(
            &mut protocol,
            1_250,
            250,
        )
        .unwrap();

        assert_eq!(protocol.bad_debt_nusd, 50);
        assert_eq!(protocol.psm_usdc_liabilities, 1_250);
        assert_eq!(protocol.psm_nusd_supply, 1_250);
        assert_eq!(protocol.psm_idle_usdc, 1_250);
        assert_eq!(protocol.psm_cap, 1_000);
        assert!(protocol.paused);
    }

    #[test]
    fn liquidation_borrower_surplus_mints_backed_nusd_without_psm_cap() {
        let mut protocol = protocol_for_psm_outflow_test();

        ix::liquidation::two_step::record_liquidation_borrower_surplus(
            &mut protocol,
            1_250,
            250,
        )
        .unwrap();

        assert_eq!(protocol.psm_usdc_liabilities, 1_250);
        assert_eq!(protocol.psm_nusd_supply, 1_250);
        assert_eq!(protocol.psm_idle_usdc, 1_250);
        assert_eq!(protocol.psm_cap, 1_000);
    }

    #[test]
    fn two_step_full_liquidation_uses_debt_fee_and_penalty_as_settlement_target() {
        let (min_settlement, target_settlement) =
            ix::liquidation::two_step::full_liquidation_settlement_terms(
                9_000_000,
                550_000_000,
                0,
                6,
                100 * domain::PRICE_SCALE,
                800,
            )
            .unwrap();

        assert_eq!(min_settlement, 594_000_000);
        assert_eq!(target_settlement, 594_000_000);
    }

    #[test]
    fn two_step_full_liquidation_accepts_lower_floor_but_keeps_target_claim() {
        let (min_settlement, target_settlement) =
            ix::liquidation::two_step::full_liquidation_settlement_terms(
                1_000_000,
                200_000_000,
                0,
                6,
                100 * domain::PRICE_SCALE,
                800,
            )
            .unwrap();

        assert_eq!(min_settlement, 100_000_000);
        assert_eq!(target_settlement, 216_000_000);
    }

    #[test]
    fn two_step_full_liquidation_floor_never_exceeds_target_from_rounding() {
        let (min_settlement, target_settlement) =
            ix::liquidation::two_step::full_liquidation_settlement_terms(
                2,
                2_000_000,
                0,
                0,
                150_000_000,
                0,
            )
            .unwrap();

        assert_eq!(min_settlement, 2_000_000);
        assert_eq!(target_settlement, 2_000_000);
    }

    #[test]
    fn two_step_full_liquidation_only_returns_surplus_above_target() {
        let principal_debt = 95_000_000_u128;
        let target_settlement = 108_000_000_u128;
        let sale_proceeds = 110_000_000_u128;

        let protocol_settlement = core::cmp::min(sale_proceeds, target_settlement);
        let revenue_amount = protocol_settlement.checked_sub(principal_debt).unwrap();
        let borrower_surplus = sale_proceeds.checked_sub(protocol_settlement).unwrap();

        assert_eq!(protocol_settlement, 108_000_000);
        assert_eq!(revenue_amount, 13_000_000);
        assert_eq!(borrower_surplus, 2_000_000);
    }

    #[test]
    fn two_step_full_liquidation_keeps_partial_recovery_below_target() {
        let principal_debt = 95_000_000_u128;
        let target_settlement = 108_000_000_u128;
        let sale_proceeds = 104_000_000_u128;

        let protocol_settlement = core::cmp::min(sale_proceeds, target_settlement);
        let revenue_amount = protocol_settlement.checked_sub(principal_debt).unwrap();
        let borrower_surplus = sale_proceeds.checked_sub(protocol_settlement).unwrap();

        assert_eq!(protocol_settlement, 104_000_000);
        assert_eq!(revenue_amount, 9_000_000);
        assert_eq!(borrower_surplus, 0);
    }

    #[test]
    fn two_step_full_liquidation_settlement_terms_reject_empty_collateral() {
        assert!(ix::liquidation::two_step::full_liquidation_settlement_terms(
            0,
            9_000_000,
            550_000_000,
            6,
            100 * domain::PRICE_SCALE,
            800,
        )
        .is_err());
        assert!(ix::liquidation::two_step::full_liquidation_settlement_terms(
            1_000_000,
            0,
            0,
            6,
            100 * domain::PRICE_SCALE,
            800,
        )
        .is_err());
    }

    #[test]
    fn open_liquidation_can_settle_after_authority_rotation() {
        let original_liquidator = Pubkey::new_unique();
        let current_authority = Pubkey::new_unique();

        assert!(ix::liquidation::two_step::require_liquidation_settlement_authority(
            current_authority,
            original_liquidator,
            original_liquidator,
        )
        .is_ok());
        assert!(ix::liquidation::two_step::require_liquidation_settlement_authority(
            current_authority,
            original_liquidator,
            current_authority,
        )
        .is_ok());
        assert!(ix::liquidation::two_step::require_liquidation_settlement_authority(
            current_authority,
            original_liquidator,
            Pubkey::new_unique(),
        )
        .is_err());
    }

    #[test]
    fn staking_state_reader_offsets_match_serialized_layout() {
        #[derive(AnchorSerialize)]
        struct StakingStateLayout {
            authority: Pubkey,
            nusd_mint: Pubkey,
            snusd_mint: Pubkey,
            staking_nusd_vault: Pubkey,
            revenue_nusd_vault: Pubkey,
            revenue_baseline_nusd: u128,
            total_shares: u128,
            total_user_shares: u128,
            staking_vault_nusd: u128,
            realized_loss_nusd: u128,
            unvested_revenue: u128,
            reserved_pending_claims: u128,
            vesting_start_ts: i64,
            vesting_end_ts: i64,
            last_vesting_sync_ts: i64,
            cooldown_seconds: i64,
            revenue_vesting_seconds: i64,
            paused: bool,
            bump: u8,
        }

        let nusd_mint = Pubkey::new_from_array([11; 32]);
        let staking_vault_nusd = 0x0102_0304_0506_0708_1112_1314_1516_1718_u128;
        let state = StakingStateLayout {
            authority: Pubkey::new_from_array([10; 32]),
            nusd_mint,
            snusd_mint: Pubkey::new_from_array([12; 32]),
            staking_nusd_vault: Pubkey::new_from_array([13; 32]),
            revenue_nusd_vault: Pubkey::new_from_array([14; 32]),
            revenue_baseline_nusd: 1,
            total_shares: 2,
            total_user_shares: 3,
            staking_vault_nusd,
            realized_loss_nusd: 4,
            unvested_revenue: 5,
            reserved_pending_claims: 6,
            vesting_start_ts: 7,
            vesting_end_ts: 8,
            last_vesting_sync_ts: 9,
            cooldown_seconds: 10,
            revenue_vesting_seconds: 11,
            paused: false,
            bump: 255,
        };
        let mut data = STAKING_STATE_DISCRIMINATOR.to_vec();
        state.serialize(&mut data).unwrap();

        assert_eq!(
            data.get(0..8),
            Some(&STAKING_STATE_DISCRIMINATOR[..])
        );
        assert_eq!(
            read_pubkey_at(&data, STAKING_STATE_NUSD_MINT_OFFSET).unwrap(),
            nusd_mint
        );
        assert_eq!(
            read_u128_at(&data, STAKING_STATE_STAKING_VAULT_NUSD_OFFSET).unwrap(),
            staking_vault_nusd
        );
    }

    #[test]
    fn rejects_lazer_payload_for_unexpected_feed_or_channel() {
        assert!(read_lazer_price(&sample_lazer_payload(), &lazer_config(2)).is_err());

        let mut payload = sample_lazer_payload();
        // Payload layout is magic u32, timestamp u64, then channel u8.
        payload[12] = 255;
        assert!(read_lazer_price(&payload, &lazer_config(1)).is_err());
    }

    #[test]
    fn rejects_negative_lazer_confidence() {
        assert!(lazer_price_to_oracle_price(
            pyth_lazer_solana_contract::protocol::PriceFeedId(1),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(100).unwrap(),
            pyth_lazer_solana_contract::protocol::Price::from_mantissa(-1).unwrap(),
            -8,
            42,
        )
        .is_err());
    }
