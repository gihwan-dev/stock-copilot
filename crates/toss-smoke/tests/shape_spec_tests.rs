#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;

    use serde_json::{Value, json};
    use toss_smoke::shape::{
        EndpointKind, FieldSpec, Schema, SpecMatch, result_schema, validate_response,
    };

    fn read_json(relative: &str) -> Value {
        let text = fs::read_to_string(relative).expect("read fixture");
        serde_json::from_str(&text).expect("parse JSON")
    }

    fn required_field_names(schema: &Schema) -> BTreeSet<&'static str> {
        match schema {
            Schema::Object { required, .. } => {
                required.iter().map(|spec: &FieldSpec| spec.name).collect()
            }
            Schema::Array { item } => required_field_names(item),
            Schema::String
            | Schema::NonEmptyString
            | Schema::DecimalString
            | Schema::Integer
            | Schema::MinInteger(_)
            | Schema::Boolean
            | Schema::Enum { .. }
            | Schema::Nullable(_) => BTreeSet::new(),
        }
    }

    fn openapi_required(spec: &Value, schema_name: &str) -> BTreeSet<String> {
        spec.pointer(&format!("/components/schemas/{schema_name}/required"))
            .and_then(Value::as_array)
            .expect("schema required array")
            .iter()
            .map(|item| item.as_str().expect("field name").to_owned())
            .collect()
    }

    #[test]
    fn compiled_schemas_match_openapi_v1_2_28_required_fields() {
        let spec = read_json("../../docs/toss-openapi-1.2.28.json");
        let mappings = [
            (EndpointKind::Token, "OAuth2TokenResponse"),
            (EndpointKind::Accounts, "Account"),
            (EndpointKind::Holdings, "HoldingsOverview"),
            (EndpointKind::BuyingPower, "BuyingPowerResponse"),
            (EndpointKind::Transactions, "PaginatedTransactionResponse"),
            (EndpointKind::ExchangeRate, "ExchangeRateResponse"),
            (EndpointKind::CalendarKr, "KrMarketCalendarResponse"),
            (EndpointKind::CalendarUs, "UsMarketCalendarResponse"),
            (
                EndpointKind::IndicatorQuotes,
                "MarketIndicatorPriceResponse",
            ),
            (EndpointKind::Sectors, "SectorsResponse"),
            (EndpointKind::Quotes, "PriceResponse"),
            (EndpointKind::Candles, "CandlePageResponse"),
        ];
        for (kind, openapi_name) in mappings {
            let actual: BTreeSet<String> = required_field_names(result_schema(kind))
                .into_iter()
                .map(str::to_owned)
                .collect();
            let expected = openapi_required(&spec, openapi_name);
            assert_eq!(actual, expected, "required mismatch for {openapi_name}");
        }
    }

    const SYNTHETIC_FIXTURES: [(EndpointKind, &str); 18] = [
        (
            EndpointKind::Accounts,
            "../adapters/broker-toss/tests/recordings/t05-accounts.json",
        ),
        (
            EndpointKind::Accounts,
            "../adapters/broker-toss/tests/account_recordings/t05-accounts-empty.json",
        ),
        (
            EndpointKind::Holdings,
            "../adapters/broker-toss/tests/recordings/t05-holdings.json",
        ),
        (
            EndpointKind::Holdings,
            "../adapters/broker-toss/tests/recordings/t05-holdings-empty.json",
        ),
        (
            EndpointKind::Holdings,
            "../adapters/broker-toss/tests/recordings/t05-holdings-kr.json",
        ),
        (
            EndpointKind::Holdings,
            "../adapters/broker-toss/tests/recordings/t05-holdings-kr-usd-omitted.json",
        ),
        (
            EndpointKind::Transactions,
            "../adapters/broker-toss/tests/recordings/t06-trades.json",
        ),
        (
            EndpointKind::Transactions,
            "../adapters/broker-toss/tests/recordings/t06-cash.json",
        ),
        (
            EndpointKind::Transactions,
            "../adapters/broker-toss/tests/recordings/t06-empty.json",
        ),
        (
            EndpointKind::ExchangeRate,
            "../adapters/broker-toss/tests/recordings/exchange-rate.json",
        ),
        (
            EndpointKind::CalendarKr,
            "../adapters/broker-toss/tests/market_recordings/t08-kr.json",
        ),
        (
            EndpointKind::CalendarKr,
            "../adapters/broker-toss/tests/market_recordings/t08-kr-holiday.json",
        ),
        (
            EndpointKind::CalendarUs,
            "../adapters/broker-toss/tests/market_recordings/t08-us.json",
        ),
        (
            EndpointKind::CalendarUs,
            "../adapters/broker-toss/tests/market_recordings/t08-us-holiday.json",
        ),
        (
            EndpointKind::IndicatorQuotes,
            "../adapters/broker-toss/tests/market_recordings/t08-indices.json",
        ),
        (
            EndpointKind::Sectors,
            "../adapters/broker-toss/tests/market_recordings/t09-sectors.json",
        ),
        (
            EndpointKind::Quotes,
            "../adapters/broker-toss/tests/market_recordings/t07-quote.json",
        ),
        (
            EndpointKind::Candles,
            "../adapters/broker-toss/tests/market_recordings/t07-daily.json",
        ),
    ];

    #[test]
    fn synthetic_broker_toss_recordings_match_smoke_shape_validator() {
        for (kind, path) in SYNTHETIC_FIXTURES {
            let fixture = read_json(path);
            let body = fixture.get("body").expect("recording body");
            let summary = validate_response(kind, body);
            assert_eq!(
                summary.spec_match,
                SpecMatch::Matched,
                "fixture failed: {path}"
            );
        }
    }

    #[test]
    fn validator_detects_mismatches_and_never_echoes_raw_values() {
        let secret_sentinel = "SENSITIVE_VALUE_991827";
        let bad_token = json!({
            "access_token": secret_sentinel,
            "token_type": secret_sentinel,
            "expires_in": 1_i32
        });
        let token_summary = validate_response(EndpointKind::Token, &bad_token);
        let SpecMatch::Mismatched(token_reasons) = token_summary.spec_match else {
            panic!("expected token mismatch");
        };
        let joined_token = token_reasons.join(" | ");
        assert!(joined_token.contains("token_type"), "{joined_token}");
        assert!(joined_token.contains("expires_in"), "{joined_token}");
        assert!(!joined_token.contains(secret_sentinel), "{joined_token}");

        let bad_accounts = json!({
            "result": [{ "accountNo": secret_sentinel, "accountSeq": "not-int", "accountType": secret_sentinel }],
            "error": { "code": secret_sentinel }
        });
        let acc_summary = validate_response(EndpointKind::Accounts, &bad_accounts);
        assert_eq!(acc_summary.unknown_enum_count, 1);
        let SpecMatch::Mismatched(acc_reasons) = acc_summary.spec_match else {
            panic!("expected accounts mismatch");
        };
        let joined_acc = acc_reasons.join(" | ");
        assert!(joined_acc.contains("$.error"), "{joined_acc}");
        assert!(joined_acc.contains("accountSeq"), "{joined_acc}");
        assert!(!joined_acc.contains(secret_sentinel), "{joined_acc}");

        let open_enum_buying_power = json!({
            "result": { "currency": "EUR_SECRET_77", "cashBuyingPower": "5000000", "extraField": "x" }
        });
        let bp_summary = validate_response(EndpointKind::BuyingPower, &open_enum_buying_power);
        assert_eq!(bp_summary.spec_match, SpecMatch::Matched);
        assert_eq!(bp_summary.unknown_enum_count, 1);
        assert_eq!(bp_summary.extra_field_count, 1);
        assert_eq!(bp_summary.known_enums.len(), 0);

        let bad_decimal = json!({
            "result": { "currency": "KRW", "cashBuyingPower": "1e9" }
        });
        let dec_summary = validate_response(EndpointKind::BuyingPower, &bad_decimal);
        assert_ne!(dec_summary.spec_match, SpecMatch::Matched);
    }
}
