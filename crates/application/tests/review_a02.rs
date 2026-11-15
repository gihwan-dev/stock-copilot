#[cfg(test)]
mod tests {
    use application::{FxRateQuery, OrderConfirmation, RecordOutcome, RiskChangeConfirmation};
    use domain::{
        HoursPermissionState, NumericInput, OrderPermission, ProfileWeight, RiskAppetite,
        RiskSettings, SavedMarketOrderSetting, WeightCaps,
    };

    fn caps() -> WeightCaps {
        let weight =
            ProfileWeight::parse(&NumericInput::try_new("0.25").expect("비중 입력")).expect("비중");
        WeightCaps::try_new(weight, weight, weight).expect("상한")
    }

    fn check_caps(
        orders: OrderPermission,
        market: SavedMarketOrderSetting,
        hours: HoursPermissionState,
        appetite: RiskAppetite,
    ) {
        for caps in [caps(), WeightCaps::default()] {
            let settings = RiskSettings::new(orders, market, hours, caps, appetite);
            assert_eq!(settings.orders(), orders);
            assert_eq!(settings.market_orders(), market);
            assert_eq!(settings.hours(), hours);
            assert_eq!(settings.weight_caps(), caps);
            assert_eq!(settings.risk_appetite(), appetite);
        }
    }

    fn check_risk_settings(orders: OrderPermission, market: SavedMarketOrderSetting) {
        for hours in [
            HoursPermissionState::Disabled,
            HoursPermissionState::Enabled,
        ] {
            for appetite in [
                RiskAppetite::Conservative,
                RiskAppetite::Neutral,
                RiskAppetite::Aggressive,
            ] {
                check_caps(orders, market, hours, appetite);
            }
        }
    }

    #[test]
    fn risk_settings_preserve_all_five_policy_values() {
        for orders in [OrderPermission::Disabled, OrderPermission::Enabled] {
            for market in [
                SavedMarketOrderSetting::Disabled,
                SavedMarketOrderSetting::Enabled,
            ] {
                check_risk_settings(orders, market);
            }
        }
    }

    #[test]
    fn presence_results_are_closed_and_not_interchangeable() {
        fn risk(value: RiskChangeConfirmation) -> &'static str {
            match value {
                RiskChangeConfirmation::Confirmed => "confirmed",
                RiskChangeConfirmation::Denied => "denied",
                RiskChangeConfirmation::Unavailable => "unavailable",
            }
        }
        assert_eq!(risk(RiskChangeConfirmation::Confirmed), "confirmed");
        assert_eq!(risk(RiskChangeConfirmation::Denied), "denied");
        assert_eq!(risk(RiskChangeConfirmation::Unavailable), "unavailable");
        assert_ne!(
            std::any::TypeId::of::<OrderConfirmation>(),
            std::any::TypeId::of::<RiskChangeConfirmation>()
        );
    }

    #[test]
    fn duplicate_append_is_a_normal_result_and_fx_query_is_closed() {
        fn record(value: RecordOutcome) -> &'static str {
            match value {
                RecordOutcome::Recorded => "recorded",
                RecordOutcome::AlreadyRecorded => "already-recorded",
            }
        }
        assert_eq!(record(RecordOutcome::Recorded), "recorded");
        assert_eq!(record(RecordOutcome::AlreadyRecorded), "already-recorded");
        let at = domain::Timestamp::from_parts(
            domain::UnixSeconds::new(42),
            domain::Nanosecond::new(0),
            domain::OffsetSeconds::new(0),
        )
        .expect("시각");
        for query in [FxRateQuery::Latest, FxRateQuery::At(at)] {
            let expected = match query {
                FxRateQuery::Latest => FxRateQuery::Latest,
                FxRateQuery::At(value) => {
                    assert_eq!(value, at);
                    FxRateQuery::At(value)
                }
            };
            assert_eq!(query, expected);
        }
    }
}
