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
    fn snapshot_pricing_uses_xstock_staleness_cap_only() {
        let mut config = lazer_config(1);
        config.max_staleness_seconds = 5;

        assert_eq!(
            policy(config.xstock_usd_feed_id, &config)
                .unwrap()
                .max_staleness_seconds,
            5
        );
        assert_eq!(
            inactive_oracle_price(config.underlying_usd_feed_id),
            OraclePriceAccount {
                feed_id: config.underlying_usd_feed_id,
                price_e8: 0,
                confidence_e8: 0,
                publish_time: 0,
            }
        );
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
            underlying_usd_feed_id: [0; 32],
            redemption_rate_feed_id: [0; 32],
            borrow_ltv_bps: 4_500,
            liquidation_threshold_bps: 5_500,
            liquidation_penalty_bps: 800,
            close_factor_bps: 5_000,
            max_confidence_bps: 200,
            max_staleness_seconds: 120,
            closed_market_max_staleness_seconds: 86_400,
            underlying_closed_market_max_staleness_seconds: 432_000,
            closed_market_haircut_bps: 9_500,
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
        let price = read_lazer_price(&sample_lazer_payload(), &lazer_config(1)).unwrap();
        let mut expected_feed_key = [0_u8; 32];
        expected_feed_key[..4].copy_from_slice(&1_u32.to_le_bytes());

        assert_eq!(price.feed_id, expected_feed_key);
        assert_eq!(price.price_e8, 6_713_436_287_632);
        assert_eq!(price.confidence_e8, 1_500_580_860);
        assert_eq!(price.publish_time, 1_771_339_368);
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
        let zero = [0_u8; 32];
        let mut lazer_feed = [0_u8; 32];
        lazer_feed[..4].copy_from_slice(&1_843_u32.to_le_bytes());
        let signed_feed = [7_u8; 32];

        assert!(validate_collateral_feed_config(&lazer_feed, &zero, &zero).is_ok());
        assert!(validate_collateral_feed_config(&signed_feed, &zero, &zero).is_ok());
        assert!(validate_collateral_feed_config(&zero, &zero, &zero).is_err());
        assert!(validate_collateral_feed_config(&lazer_feed, &lazer_feed, &zero).is_err());
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
            reserved: false,
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
    fn psm_outflow_breaker_allows_exact_limit_and_pauses_on_excess() {
        let mut protocol = protocol_for_psm_outflow_test();

        assert!(record_psm_outflow_or_pause(&mut protocol, 100, 1_000, 10).unwrap());
        assert!(!protocol.paused);
        assert_eq!(protocol.psm_outflow_window_usdc, 100);

        assert!(!record_psm_outflow_or_pause(&mut protocol, 1, 900, 11).unwrap());
        assert!(protocol.paused);
        assert_eq!(protocol.psm_outflow_window_usdc, 100);
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
