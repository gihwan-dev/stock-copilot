use crate::support_tests::validation::validate_recording;
use crate::support_tests::{recordings, route, serve_recordings, spec};
use serde_json::Value;

#[test]
fn all_recordings_match_current_spec_and_generated_types() {
    let spec = spec();
    for recording in recordings() {
        validate_recording(&spec, &recording).expect("recording matches current spec");
    }
}

#[test]
fn recordings_replay_over_local_client() {
    use application::SecretValue;
    use broker_toss::{ReadRequest, TossEndpoint, TossHttpClient};
    for recording in recordings() {
        validate_recording(&spec(), &recording).expect("valid recording");
        let server = serve_recordings(&recording);
        let client = TossHttpClient::new(&TossEndpoint::local(server.port()).expect("endpoint"))
            .expect("client");
        let route = route(&recording);
        let response = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(client.get(
                &ReadRequest::new(route),
                &SecretValue::new(b"synthetic-token".to_vec()),
            ))
            .expect("response");
        assert_eq!(
            u64::from(response.status()),
            recording["status"]
                .as_str()
                .expect("status")
                .parse::<u64>()
                .expect("number")
        );
        assert_eq!(
            serde_json::from_slice::<Value>(response.body().as_exposed_secret()).expect("body"),
            recording["body"]
        );
        let request = String::from_utf8(server.finish().expect("request")).expect("utf8");
        assert!(request.starts_with(&format!(
                "GET {} HTTP/1.1\r\n",
                recording["path"]
                    .as_str()
                    .expect("path")
                    .replace("{symbol}", "synthetic-731")
                    .replace("{orderId}", "synthetic-731")
                    .replace("{sectorId}", "synthetic-731")
            )));
    }
}

#[test]
fn invalid_recordings_are_rejected() {
    let spec = spec();
    let original = recordings()
        .into_iter()
        .find(|recording| recording["path"] == "/api/v1/accounts")
        .expect("accounts");
    for (pointer, value) in [
        ("/origin", Value::from("live")),
        ("/method", Value::from("post")),
        ("/path", Value::from("/missing")),
        ("/status", Value::from("201")),
        ("/schema", Value::from("#/components/schemas/ApiResponse")),
        ("/body/result", Value::Null),
        ("/body/result/0/accountSeq", Value::from("413")),
        ("/body/result/0/accountType", Value::from("FUTURE")),
        ("/body/result/0/accountNo", Value::from(731_i32)),
    ] {
        let mut changed = original.clone();
        *changed.pointer_mut(pointer).expect("pointer") = value;
        assert!(validate_recording(&spec, &changed).is_err(), "{pointer}");
    }
    for pointer in ["/body/result/0", "/body"] {
        let mut changed = original.clone();
        changed
            .pointer_mut(pointer)
            .expect("object")
            .as_object_mut()
            .expect("object")
            .clear();
        assert!(validate_recording(&spec, &changed).is_err(), "{pointer}");
    }
    let mut changed_spec = spec.clone();
    changed_spec["components"]["schemas"]["Account"]["required"]
        .as_array_mut()
        .expect("required")
        .push(Value::from("newRequired"));
    assert!(validate_recording(&changed_spec, &original).is_err());
    changed_spec = spec;
    changed_spec["components"]["schemas"]["Account"]["properties"]["accountSeq"]["type"] =
        Value::from("string");
    assert!(validate_recording(&changed_spec, &original).is_err());
}

#[test]
fn schema_constraints_and_unsupported_keywords_fail_closed() {
    use crate::support_tests::validation::validate;
    use serde_json::json;
    let empty = json!({});
    for (schema, valid, invalid) in constraint_cases() {
        validate(&empty, &schema, &valid, 0).expect("valid");
        assert!(validate(&empty, &schema, &invalid, 0).is_err(), "{schema}");
    }
    for schema in [
        json!({"type":"future"}),
        json!({"type":true}),
        json!({"pattern":"x"}),
        json!({"$ref":"https://invalid/schema"}),
        json!({"$ref":"#/absent"}),
    ] {
        assert!(
            validate(&empty, &schema, &json!("x"), 0).is_err(),
            "{schema}"
        );
    }
    validate(&empty, &empty, &Value::Null, 64).expect("depth boundary");
    assert!(validate(&empty, &empty, &Value::Null, 65).is_err());
    let recursive = json!({"node":{"$ref":"#/node"}});
    assert!(validate(&recursive, &json!({"$ref":"#/node"}), &Value::Null, 0).is_err());
}

fn constraint_cases() -> Vec<(Value, Value, Value)> {
    use serde_json::json;
    vec![
        (json!({"type":"object"}), json!({}), json!([])),
        (json!({"type":"array"}), json!([]), json!({})),
        (json!({"type":"string"}), json!("sample"), json!(81_i32)),
        (json!({"type":"integer"}), json!(73_i32), json!(1.25_f64)),
        (json!({"type":"number"}), json!(1.75_f64), json!("1.75")),
        (json!({"type":"boolean"}), json!(true), json!("true")),
        (json!({"type":"null"}), Value::Null, json!(false)),
        (json!({"type":["object","null"]}), Value::Null, json!("x")),
        (
            json!({"type":"string","maxLength":3_i32}),
            json!("가나다"),
            json!("가나다라"),
        ),
        (
            json!({"enum":["FIRST","SECOND"]}),
            json!("SECOND"),
            json!("THIRD"),
        ),
        (
            json!({"type":"array","items":{"type":"integer"}}),
            json!([17_i32, 29_i32]),
            json!([17_i32, "29"]),
        ),
        (
            json!({"required":["field"]}),
            json!({"field":null}),
            json!({}),
        ),
        (
            json!({"properties":{"field":{"type":"string"}}}),
            json!({"field":"text"}),
            json!({"field":17_i32}),
        ),
        (
            json!({"type":"object","additionalProperties":false}),
            json!({}),
            json!({"extra":1_i32}),
        ),
        (
            json!({"additionalProperties":{"type":"string"}}),
            json!({"a":"value"}),
            json!({"a":1_i32}),
        ),
        (
            json!({"allOf":[{"type":"string"},{"maxLength":2_i32}]}),
            json!("ab"),
            json!("abc"),
        ),
    ]
}

#[test]
fn one_of_requires_exactly_one_matching_branch() {
    use crate::support_tests::validation::validate;
    use serde_json::json;
    let spec = json!({});
    let nullable = json!({"type":"object","nullable":true});
    validate(&spec, &nullable, &Value::Null, 0).expect("nullable object");
    assert!(validate(&spec, &nullable, &json!(17_i32), 0).is_err());
    let schema = json!({"oneOf":[{"type":"integer"},{"type":"string"}]});
    validate(&spec, &schema, &json!(73_i32), 0).expect("one branch");
    assert!(validate(&spec, &schema, &Value::Null, 0).is_err());
    assert!(validate(&spec, &json!({"oneOf":[{},{}]}), &Value::Null, 0).is_err());
}
