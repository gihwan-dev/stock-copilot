use super::{serde_detail, strip_values};

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct Probe {
    amount: u64,
    name: String,
    kind: ProbeKind,
}

#[derive(Debug, serde::Deserialize)]
enum ProbeKind {
    Bearer,
}

fn detail(json: &str) -> String {
    serde_detail(&serde_json::from_str::<Probe>(json).expect_err("probe must fail"))
}

const SECRETS: [&str; 5] = [
    "98765432",
    "fake-access-91",
    "00000070219",
    "secret-token-xyz",
    "tsck_live",
];

fn assert_clean(text: &str) {
    for secret in SECRETS {
        assert!(!text.contains(secret), "{secret} leaked in {text}");
    }
}

#[test]
fn serde_messages_keep_shape_but_drop_values() {
    let cases = [
        (
            r#"{"amount":"98765432","name":"x","kind":"Bearer"}"#,
            "data: invalid type: string <value>, expected u64 at line 1 column 20",
        ),
        (
            r#"{"amount":1,"name":98765432,"kind":"Bearer"}"#,
            "data: invalid type: integer <value>, expected a string at line 1 column 27",
        ),
        (
            r#"{"amount":1,"name":"x","kind":"secret-token-xyz"}"#,
            "data: unknown variant <value>, expected `Bearer` at line 1 column 48",
        ),
        (
            r#"{"amount":1,"name":"x"}"#,
            "data: missing field `kind` at line 1 column 23",
        ),
        (
            r#"{"amount":1,"name":"x","kind":"Bearer","fake-access-91":1}"#,
            "data: unknown field `fake-access-91`, expected one of `amount`, `name`, `kind` at line 1 column 55",
        ),
        (
            r#"{"amount":1,"name":"00000070219"#,
            "eof: EOF while parsing a string at line 1 column 31",
        ),
    ];
    for (json, expected) in cases {
        assert_eq!(detail(json), expected, "{json}");
    }
    for json in [
        r#"{"amount":"98765432","name":"x","kind":"Bearer"}"#,
        r#"{"amount":1,"name":98765432,"kind":"Bearer"}"#,
        r#"{"amount":1,"name":"x","kind":"secret-token-xyz"}"#,
        r#"{"amount":1,"name":"00000070219"#,
    ] {
        assert_clean(&detail(json));
    }
}

#[test]
fn value_deserializer_messages_and_custom_text_are_masked() {
    let value = serde_json::json!({"amount": -98_765_432_i64, "name": "x", "kind": "Bearer"});
    let message = serde_detail(&serde_json::from_value::<Probe>(value).expect_err("negative"));
    assert_eq!(
        message,
        "data: invalid value: integer <value>, expected u64"
    );
    assert_eq!(
        strip_values("custom failure for \"secret-token-xyz\" with 98765432 and `00000070219`"),
        "custom failure for <value> with # and <value>"
    );
    assert_eq!(
        strip_values("invalid length 3, expected a tuple of size 2"),
        "invalid length 3, expected a tuple of size 2"
    );
    assert_eq!(
        strip_values("invalid type: null, expected a string"),
        "invalid type: null, expected a string"
    );
    assert_eq!(
        strip_values("unknown variant `x`"),
        "unknown variant <value>"
    );
}

#[test]
fn probe_accepts_the_well_formed_shape() {
    let probe: Probe =
        serde_json::from_str(r#"{"amount":1,"name":"x","kind":"Bearer"}"#).expect("probe");
    assert_eq!((probe.amount, probe.name.as_str()), (1, "x"));
    let ProbeKind::Bearer = probe.kind;
}
