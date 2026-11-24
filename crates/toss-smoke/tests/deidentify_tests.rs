#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use serde_json::{Value, json};
    use toss_smoke::deidentify::{
        DeidentifyError, account_like_strings, deidentify_recording, has_allowed_origin,
    };

    fn account_number() -> String {
        ["110", "48", "203917"].join("-")
    }

    fn recording() -> Value {
        let account = account_number();
        json!({
            "formatVersion": 1_u64,
            "openapiVersion": "1.2.28",
            "steps": [
                {
                    "step": "accounts",
                    "path": "/api/v1/accounts",
                    "status": 200_u64,
                    "outcome": "ok",
                    "schemaStatus": "match",
                    "rateLimit": null,
                    "body": {"result": [{"accountNo": account, "accountSeq": 7_u64, "accountType": "BROKERAGE"}]}
                },
                {
                    "step": "holdings",
                    "path": "/api/v1/holdings",
                    "status": 200_u64,
                    "outcome": "ok",
                    "schemaStatus": "match",
                    "rateLimit": null,
                    "body": {"result": [{
                        "symbol": "005930",
                        "quantity": "37",
                        "averagePurchasePrice": "71234.56",
                        "purchaseAmount": "-2635678.72",
                        "lastPrice": "0",
                        "settlementDate": "2026-03-25",
                        "accountNo": account
                    }]}
                },
                {
                    "step": "token",
                    "path": "/oauth2/token",
                    "status": null,
                    "outcome": "skipped",
                    "schemaStatus": "none",
                    "rateLimit": null,
                    "body": null
                }
            ]
        })
    }

    #[test]
    fn recording_becomes_deidentified_fixtures_without_accounts_or_amounts() {
        let fixtures = deidentify_recording(&recording()).expect("비식별화");
        assert_eq!(fixtures.len(), 2);
        let text =
            serde_json::to_string(&fixtures.iter().map(|item| &item.value).collect::<Vec<_>>())
                .expect("직렬화");
        for original in [
            account_number().as_str(),
            "71234.56",
            "2635678.72",
            "\"37\"",
        ] {
            assert!(!text.contains(original), "{original}");
        }
        for fixture in &fixtures {
            assert!(has_allowed_origin(&fixture.value));
            assert_eq!(account_like_strings(&fixture.value), Vec::<String>::new());
            assert_eq!(fixture.value["method"], "get");
            assert_eq!(fixture.value["status"], "200");
        }
        let accounts = &fixtures[0].value["body"]["result"][0];
        let holdings = &fixtures[1].value["body"]["result"][0];
        assert_eq!(accounts["accountNo"], holdings["accountNo"]);
        assert_eq!(accounts["accountNo"], "synthetic-account-101");
        assert_eq!(accounts["accountType"], "BROKERAGE");
        assert_eq!(accounts["accountSeq"], 1_u64);
        assert_eq!(holdings["symbol"], "005930");
        assert_eq!(holdings["lastPrice"], "0");
        assert_eq!(holdings["settlementDate"], "2026-03-25");
        let price = holdings["averagePurchasePrice"].as_str().expect("가격");
        assert_eq!(price.split_once('.').map(|(_, scale)| scale.len()), Some(2));
        assert!(
            holdings["purchaseAmount"]
                .as_str()
                .expect("금액")
                .starts_with('-')
        );
        assert_eq!(
            fixtures[1].value["schema"],
            "#/paths/~1api~1v1~1holdings/get/responses/200/content/application~1json/schema"
        );
        assert_eq!(fixtures[0].name, "deidentified-accounts-200.json");
    }

    #[test]
    fn non_recordings_and_other_versions_are_rejected() {
        assert_eq!(
            deidentify_recording(&json!([])),
            Err(DeidentifyError::NotRecording)
        );
        assert_eq!(
            deidentify_recording(&json!({"formatVersion": 2_u64, "steps": []})),
            Err(DeidentifyError::UnsupportedFormat)
        );
        assert_eq!(
            deidentify_recording(&json!({"formatVersion": 1_u64})),
            Err(DeidentifyError::NotRecording)
        );
    }

    #[test]
    fn free_text_account_numbers_stop_the_tool() {
        let mut value = recording();
        value["steps"][0]["body"]["message"] = Value::from(format!("계좌 {}", account_number()));
        assert_eq!(
            deidentify_recording(&value),
            Err(DeidentifyError::LeakRemains)
        );
    }

    #[test]
    fn account_shape_detection_spares_dates_times_and_short_numbers() {
        for clean in [
            json!({"date": "2026-03-25", "at": "2026-03-25T00:00:00+09:00", "amount": "123456789.12"}),
            json!({"accountNo": "synthetic-account-731"}),
            json!({"memo": "00000000731", "value": "1041436650000"}),
        ] {
            assert_eq!(account_like_strings(&clean), Vec::<String>::new());
        }
        for leaked in [
            json!({"memo": account_number()}),
            json!({"accountNo": "12345678901"}),
            json!({"accountNumber": "anything"}),
        ] {
            assert_eq!(account_like_strings(&leaked).len(), 1);
        }
    }

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Scope {
        Fixture,
        Other,
    }

    fn scope_of(name: &str, inside: Scope) -> Scope {
        if name == "tests" || name == "fixtures" {
            Scope::Fixture
        } else {
            inside
        }
    }

    fn collect_fixtures(directory: &Path, inside: Scope, found: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let raw_name = entry.file_name();
            let name = raw_name.to_string_lossy();
            if name == "target" || name == "node_modules" || name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                let nested = scope_of(&name, inside);
                collect_fixtures(&path, nested, found);
            } else if inside == Scope::Fixture && name.ends_with(".json") {
                found.push(path);
            }
        }
    }

    #[test]
    fn repository_toss_fixtures_have_no_account_numbers_and_only_synthetic_origins() {
        let root = Path::new("../..");
        let mut files = Vec::new();
        collect_fixtures(&root.join("crates"), Scope::Other, &mut files);
        collect_fixtures(&root.join("fixtures"), Scope::Fixture, &mut files);
        let mut toss = 0_usize;
        for file in &files {
            let text = fs::read_to_string(file).expect("픽스처");
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            assert_eq!(
                account_like_strings(&value),
                Vec::<String>::new(),
                "{}",
                file.display()
            );
            let path = value.get("path").and_then(Value::as_str);
            if path.is_some_and(|item| item.starts_with("/api/")) {
                toss = toss.saturating_add(1);
                assert!(has_allowed_origin(&value), "{}", file.display());
            }
        }
        assert!(toss >= 60, "토스 픽스처 {toss}개");
    }
}
