#[cfg(test)]
mod tests {
    use broker_toss_schema::types::{AssetClass, OrderStatus, TransactionType};
    use serde::{Serialize, de::DeserializeOwned};
    use serde_json::Value;
    use std::collections::BTreeSet;
    use std::fs;

    fn round_trip<T: DeserializeOwned + Serialize>(example: &Value) {
        let typed: T = serde_json::from_value(example.clone()).expect("spec example");
        assert_eq!(serde_json::to_value(typed).expect("serialize"), *example);
    }

    #[test]
    fn every_schema_level_example_round_trips_through_generated_types() {
        let input =
            fs::read_to_string("../../../docs/toss-openapi-1.2.28.json").expect("local spec");
        let spec: Value = serde_json::from_str(&input).expect("spec JSON");
        let schemas = spec
            .pointer("/components/schemas")
            .and_then(Value::as_object)
            .expect("schemas");
        let mut seen = BTreeSet::new();
        for (name, schema) in schemas {
            if let Some(example) = schema.get("example") {
                match name.as_str() {
                    "AssetClass" => round_trip::<AssetClass>(example),
                    "OrderStatus" => round_trip::<OrderStatus>(example),
                    "TransactionType" => round_trip::<TransactionType>(example),
                    other => panic!("new example needs typed round trip: {other}"),
                }
                let _inserted = seen.insert(name.as_str());
            }
        }
        assert_eq!(
            seen,
            BTreeSet::from(["AssetClass", "OrderStatus", "TransactionType"])
        );
    }

    #[test]
    fn unknown_enum_can_be_observed_in_raw_payload_before_typed_decoding() {
        let raw: Value = serde_json::from_str("\"FUTURE_STATUS\"").expect("raw JSON");
        let error =
            serde_json::from_value::<OrderStatus>(raw.clone()).expect_err("closed generated enum");
        assert!(error.to_string().contains("unknown variant"));
        assert_eq!(raw.as_str(), Some("FUTURE_STATUS"));
        assert_eq!(
            serde_json::to_string(&raw).expect("serialize raw"),
            "\"FUTURE_STATUS\""
        );
    }
    #[test]
    fn generated_pattern_checks_preserve_exact_decimal_text_and_reject_invalid_values() {
        use broker_toss_schema::types::{
            ConditionalOrderCreateRequestClientOrderId, ConditionalOrderCreateRequestQuantity,
        };
        for value in ["0", "1", "123.450000000000000001"] {
            let raw = Value::String(String::from(value));
            round_trip::<ConditionalOrderCreateRequestQuantity>(&raw);
        }
        for value in ["-1", "1e3", "NaN", "1.", ""] {
            let _error = serde_json::from_value::<ConditionalOrderCreateRequestQuantity>(
                Value::String(String::from(value)),
            )
            .expect_err("decimal pattern");
        }
        round_trip::<ConditionalOrderCreateRequestClientOrderId>(&Value::String(String::from(
            "order_123-test",
        )));
        let _error = serde_json::from_value::<ConditionalOrderCreateRequestClientOrderId>(
            Value::String(String::from("contains space")),
        )
        .expect_err("identifier pattern");
    }
    #[test]
    fn rust_field_renaming_preserves_wire_json() {
        use broker_toss_schema::types::{Account, AfterMarketSession};
        let account: Value = serde_json::from_str(
            r#"{"accountNo":"12345678","accountSeq":1,"accountType":"BROKERAGE"}"#,
        )
        .expect("account JSON");
        round_trip::<Account>(&account);
        let session: Value = serde_json::from_str(r#"{"startTime":"2026-03-25T15:30:00+09:00","endTime":"2026-03-25T20:00:00+09:00","singlePriceAuctionEndTime":"2026-03-25T16:00:00+09:00"}"#).expect("session JSON");
        round_trip::<AfterMarketSession>(&session);
    }
}
