    const FEED_ID: [u8; 32] = [7; 32];

    #[test]
    fn scales_pyth_prices_to_e8_with_floor_for_price_and_ceiling_for_confidence() {
        let price = pyth_price_to_oracle_price(
            FEED_ID,
            Price {
                price: 1_234_567_899,
                conf: 101,
                exponent: -10,
                publish_time: 42,
            },
        )
        .unwrap();

        assert_eq!(price.feed_id, FEED_ID);
        assert_eq!(price.price_e8, 12_345_678);
        assert_eq!(price.confidence_e8, 2);
        assert_eq!(price.publish_time, 42);
    }

    #[test]
    fn scales_pyth_prices_to_e8_when_exponent_needs_multiplication() {
        let price = pyth_price_to_oracle_price(
            FEED_ID,
            Price {
                price: 123_456,
                conf: 7,
                exponent: -6,
                publish_time: 42,
            },
        )
        .unwrap();

        assert_eq!(price.price_e8, 12_345_600);
        assert_eq!(price.confidence_e8, 700);
    }

    #[test]
    fn rejects_non_positive_pyth_prices() {
        assert!(pyth_price_to_oracle_price(
            FEED_ID,
            Price {
                price: 0,
                conf: 0,
                exponent: -8,
                publish_time: 42,
            },
        )
        .is_err());

        assert!(pyth_price_to_oracle_price(
            FEED_ID,
            Price {
                price: -1,
                conf: 0,
                exponent: -8,
                publish_time: 42,
            },
        )
        .is_err());
    }

    #[test]
    fn rejects_unrepresentable_pyth_scaling() {
        assert!(scale_pyth_price_to_e8(1, 31, false).is_err());
        assert!(scale_pyth_price_to_e8(1, -47, false).is_err());
    }

    #[test]
    fn derives_market_state_from_updateable_calendar() {
        let mut calendar = MarketCalendar {
            protocol: Pubkey::new_unique(),
            valid_until_ts: 2_000_000,
            regular_open_seconds: 13 * 3_600 + 30 * 60,
            regular_close_seconds: 20 * 3_600,
            extended_open_seconds: 8 * 3_600,
            extended_close_seconds: 21 * 3_600,
            closed_days: vec![10],
            bump: 255,
        };

        assert_eq!(
            derive_market_state_from_calendar(&calendar, 4 * SECONDS_PER_DAY + 14 * 3_600).unwrap(),
            MarketStateAccount::Regular
        );
        assert_eq!(
            derive_market_state_from_calendar(&calendar, 4 * SECONDS_PER_DAY + 8 * 3_600).unwrap(),
            MarketStateAccount::Extended
        );
        assert_eq!(
            derive_market_state_from_calendar(&calendar, 4 * SECONDS_PER_DAY + 22 * 3_600).unwrap(),
            MarketStateAccount::Closed
        );
        assert_eq!(
            derive_market_state_from_calendar(&calendar, 2 * SECONDS_PER_DAY + 14 * 3_600).unwrap(),
            MarketStateAccount::Closed
        );
        assert_eq!(
            derive_market_state_from_calendar(&calendar, 10 * SECONDS_PER_DAY + 14 * 3_600)
                .unwrap(),
            MarketStateAccount::Closed
        );

        calendar.valid_until_ts = 1;
        assert!(derive_market_state_from_calendar(&calendar, 2).is_err());
    }

    #[test]
    fn validates_market_calendar_params() {
        let valid = MarketCalendarParams {
            valid_until_ts: 1_000,
            regular_open_seconds: 13 * 3_600 + 30 * 60,
            regular_close_seconds: 20 * 3_600,
            extended_open_seconds: 8 * 3_600,
            extended_close_seconds: 21 * 3_600,
            closed_days: vec![1, 2, 5],
        };
        validate_market_calendar_params(&valid, 999).unwrap();

        let mut duplicate_day = valid.clone();
        duplicate_day.closed_days = vec![1, 1];
        assert!(validate_market_calendar_params(&duplicate_day, 999).is_err());

        let mut expired = valid.clone();
        expired.valid_until_ts = 998;
        assert!(validate_market_calendar_params(&expired, 999).is_err());

        let mut invalid_windows = valid;
        invalid_windows.regular_close_seconds = invalid_windows.extended_close_seconds + 1;
        assert!(validate_market_calendar_params(&invalid_windows, 999).is_err());
    }

    #[test]
    fn pyth_refresh_uses_xstock_staleness_cap_only() {
        let config = CollateralConfig {
            protocol: Pubkey::new_unique(),
            collateral_mint: Pubkey::new_unique(),
            collateral_vault: Pubkey::new_unique(),
            insurance_collateral_vault: Pubkey::new_unique(),
            token_program: Pubkey::new_unique(),
            symbol: [0; 16],
            collateral_decimals: 6,
            xstock_usd_feed_id: [0; 32],
            underlying_usd_feed_id: [0; 32],
            redemption_rate_feed_id: [0; 32],
            borrow_ltv_bps: 4_500,
            liquidation_threshold_bps: 5_500,
            liquidation_penalty_bps: 800,
            close_factor_bps: 5_000,
            max_confidence_bps: 200,
            max_staleness_seconds: 5,
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
        };

        assert_eq!(pyth_max_age(&config).unwrap(), 5);
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
    fn derives_fixed_pyth_price_feed_account_for_shard_zero() {
        let spyx_feed_id = [
            40, 23, 183, 132, 56, 199, 105, 53, 113, 130, 192, 67, 70, 253, 218, 173, 17, 120,
            200, 47, 64, 72, 130, 143, 224, 153, 124, 60, 100, 98, 78, 20,
        ];

        assert_eq!(
            pyth_fixed_price_feed_account(spyx_feed_id),
            pubkey!("jf8MarLKgBte4f3NWufbNpGRCuBfJLhuZPuFigvSQR2")
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
    fn collateral_feed_config_requires_lazer_xstock_feed_id() {
        let zero = [0_u8; 32];
        let mut lazer_feed = [0_u8; 32];
        lazer_feed[..4].copy_from_slice(&1_843_u32.to_le_bytes());

        assert!(validate_collateral_feed_config(&lazer_feed, &zero, &zero).is_ok());
        assert!(validate_collateral_feed_config(&[7_u8; 32], &zero, &zero).is_err());
        assert!(validate_collateral_feed_config(&zero, &zero, &zero).is_err());
        assert!(validate_collateral_feed_config(&lazer_feed, &lazer_feed, &zero).is_err());
    }

    #[test]
    fn pyth_core_path_rejects_lazer_feed_keys() {
        let mut lazer_feed = [0_u8; 32];
        lazer_feed[..4].copy_from_slice(&1_843_u32.to_le_bytes());
        assert!(require_pyth_core_feed_config(&lazer_feed).is_err());
        assert!(require_pyth_core_feed_config(&[7_u8; 32]).is_ok());
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
