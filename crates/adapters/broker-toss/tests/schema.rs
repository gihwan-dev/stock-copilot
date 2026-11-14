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
    fn unknown_enum_survives_lists_and_transaction_rows() {
        use broker_toss_schema::types::{Currency, Transaction};
        let currencies: Vec<Currency> =
            serde_json::from_str("[\"KRW\",\"EUR\",\"USD\"]").expect("open currencies");
        assert_eq!(
            currencies.get(1),
            Some(&Currency::Unknown(String::from("EUR")))
        );
        assert_eq!(
            serde_json::to_string(&currencies).expect("serialize"),
            "[\"KRW\",\"EUR\",\"USD\"]"
        );
        let source = r#"{"transactionType":"BUY","marketCountry":"KR","currency":"KRW","symbol":null,"assetClass":null,"isinCode":null,"quantity":null,"filledPrice":null,"amount":null,"commission":"0","tax":"0","netAmount":null,"exchangeRate":null,"settlementDate":"2026-03-25","settled":true,"transactionTime":null,"isCancellation":false}"#;
        let normal: Value = serde_json::from_str(source).expect("transaction");
        let mut unknown = normal.clone();
        *unknown.get_mut("currency").expect("currency") = Value::String(String::from("EUR"));
        *unknown.get_mut("transactionType").expect("kind") =
            Value::String(String::from("FUTURE_KIND"));
        let rows = Value::Array(vec![normal.clone(), unknown, normal]);
        round_trip::<Vec<Transaction>>(&rows);
        let decoded: Vec<Transaction> = serde_json::from_value(rows).expect("all rows retained");
        assert_eq!(decoded.len(), 3);
        assert_eq!(
            decoded.get(1).expect("unknown row").currency,
            Currency::Unknown(String::from("EUR"))
        );
    }

    #[test]
    fn order_create_request_rejects_ambiguous_amount_and_quantity() {
        use broker_toss_schema::types::OrderCreateRequest;
        let quantity = r#"{"symbol":"005930","side":"BUY","orderType":"MARKET","quantity":"1","confirmHighValueOrder":false,"timeInForce":"DAY"}"#;
        let mut raw: Value = serde_json::from_str(quantity).expect("wire JSON");
        round_trip::<OrderCreateRequest>(&raw);
        raw.as_object_mut().expect("object").insert(
            String::from("orderAmount"),
            Value::String(String::from("10")),
        );
        let _error = serde_json::from_value::<OrderCreateRequest>(raw)
            .expect_err("mutually exclusive fields");
        let amount: Value = serde_json::from_str(r#"{"symbol":"AAPL","side":"BUY","orderType":"MARKET","orderAmount":"10","confirmHighValueOrder":false}"#).expect("amount JSON");
        round_trip::<OrderCreateRequest>(&amount);
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
