#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use proptest::prop_oneof;
    use proptest::strategy::Just;
    use proptest::test_runner::{Config, TestRunner};
    use rust_decimal::Decimal;

    use domain::{
        Amount, MaskedAccountNumber, Quantity, SensitiveAccountNumber, SensitiveContextError,
        SensitivePersonalName, UntrustedText, WebSearchCandidateQuery, WebSearchDigestError,
        WebSearchInspection, WebSearchQueryDigest, WebSearchRejectionReason,
        WebSearchSensitiveContext, inspect_web_search_query,
    };

    fn empty_ctx() -> WebSearchSensitiveContext {
        WebSearchSensitiveContext::new()
    }

    fn amount(value: i64) -> Amount {
        Amount::parse(&domain::AmountInput::new(&value.to_string())).expect("amount")
    }

    fn quantity(value: i64) -> Quantity {
        Quantity::try_new(Decimal::new(value, 0_u32)).expect("quantity")
    }

    fn assert_rejected_with(
        text: &str,
        ctx: &WebSearchSensitiveContext,
        expected: WebSearchRejectionReason,
    ) {
        let outcome = inspect_web_search_query(&WebSearchCandidateQuery::new(text), ctx);
        let WebSearchInspection::Rejected(rejected) = outcome else {
            panic!("expected rejection for {text:?}");
        };
        assert!(
            rejected.reasons().contains(&expected),
            "query={text:?}, reasons={:?}",
            rejected.reasons()
        );
        let debug = format!("{rejected:?}");
        assert!(debug.contains("RejectedWebSearchQuery"), "{debug}");
        assert!(!debug.contains("secret-leak"), "{debug}");
    }

    #[test]
    fn sha256_digest_matches_nist_vectors_and_validates_format() {
        for (input, expected) in [
            (
                "",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            (
                "abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            (
                "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
        ] {
            let query = WebSearchCandidateQuery::new(input);
            let digest = WebSearchQueryDigest::of_query(&query);
            assert_eq!(digest.as_str(), expected);
            assert_eq!(format!("{digest}"), expected);
            let untrusted = UntrustedText::try_new(input).expect("untrusted");
            assert_eq!(WebSearchQueryDigest::of_untrusted(&untrusted), digest);
            assert_eq!(WebSearchQueryDigest::try_new(expected), Ok(digest));
        }
        for bad in [
            "",
            "short",
            "BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
            "zz7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        ] {
            assert_eq!(
                WebSearchQueryDigest::try_new(bad),
                Err(WebSearchDigestError::InvalidFormat)
            );
        }
    }

    #[test]
    fn sensitive_context_constructors_validate_and_redact_debug() {
        for bad_name in [
            "",
            "   ",
            "--",
            "이",
            "a",
            "홍길동\n",
            &"가".repeat(45_usize),
        ] {
            assert_eq!(
                SensitivePersonalName::try_new(bad_name),
                Err(SensitiveContextError::InvalidFormat),
                "{bad_name}"
            );
        }
        for bad_acct in [
            "",
            "123",
            "12345678\n",
            "1234-5678-abcd",
            "123456789012345678901234567890123",
            "1234567890123456789012345678901234567890123456789",
        ] {
            assert_eq!(
                SensitiveAccountNumber::try_new(bad_acct),
                Err(SensitiveContextError::InvalidFormat),
                "{bad_acct}"
            );
        }
        let name = SensitivePersonalName::try_new(" 홍길동 ").expect("name");
        assert_eq!(name.as_str(), "홍길동");
        let acct = SensitiveAccountNumber::try_new(" 123-45-678901 ").expect("acct");
        assert_eq!(acct.as_str(), "123-45-678901");
        let ctx = WebSearchSensitiveContext::new()
            .with_account_number(acct.clone())
            .with_masked_account(MaskedAccountNumber::try_new("987654321").expect("masked"))
            .with_holding_quantity(quantity(77_i64))
            .with_holding_amount(amount(5_430_000_i64))
            .with_personal_name(name.clone());
        assert_eq!(format!("{name:?}"), "SensitivePersonalName(<redacted>)");
        assert_eq!(format!("{acct:?}"), "SensitiveAccountNumber(<redacted>)");
        let ctx_debug = format!("{ctx:?}");
        assert!(
            ctx_debug.contains("WebSearchSensitiveContext"),
            "{ctx_debug}"
        );
        for secret in ["홍길동", "123-45-678901", "4321", "77", "5430000"] {
            assert!(!ctx_debug.contains(secret), "{ctx_debug}");
        }
        let candidate = WebSearchCandidateQuery::new("홍길동 123-45-678901");
        let cand_debug = format!("{candidate:?}");
        assert_eq!(cand_debug, "WebSearchCandidateQuery(<redacted>)");
        assert!(!cand_debug.contains("홍길동"), "{cand_debug}");
        assert!(!cand_debug.contains("123"), "{cand_debug}");
    }

    #[test]
    fn legitimate_research_queries_and_year_quarter_allowlist_pass_unmodified() {
        for valid in [
            "삼성전자 실적 전망",
            "삼성전자 2026년 3분기 실적",
            "삼성전자 2026년 실적",
            "삼성전자 2026년 상반기 실적",
            "삼성전자 2026년 하반기 전망",
            "삼성전자 2026년3분기 공시",
            "삼성전자 3분기 실적",
            "삼성전자 2025년 4분기 2026년 1분기 실적 비교",
            "AAPL 2026 Q3 earnings",
            "AAPL Q3 2026 earnings",
            "AAPL 2026Q3 earnings",
            "AAPL 2026-Q3 earnings",
            "AAPL Q3-2026 earnings",
            "AAPL FY2026 earnings",
            "AAPL FY 2026 guidance",
            "AAPL FY-2026 revenue",
            "BRK.B earnings outlook",
            "SK하이닉스 HBM3E 수주",
            "Tauri 2 release notes",
            "하나금융 배당주 공시",
            "두산에너빌리티 원전 수주",
            "세아베스틸 주가 전망",
            "한화솔루션 이사회 결의",
            "오일쇼크 원자재 영향",
            "구글 클라우드 주주총회",
            "Capital One shareholder update",
            "삼성전자 (실적)",
            "삼성전자 () 실적",
            "삼성전자 . . 실적",
            "삼성전자 가.나 실적",
            "삼성전자 &#a",
            "삼성전자 a;b",
            "삼성전자 0xa",
            "삼성전자 Abcdefghijklm1a",
            "삼성전자 abcdef=",
            "삼성전자 =a",
            "삼성전자 a=",
            "삼성전자 a..b",
            "삼성전자 매출/영업이익",
        ] {
            let candidate = WebSearchCandidateQuery::new(valid);
            let outcome = inspect_web_search_query(&candidate, &empty_ctx());
            let WebSearchInspection::Approved(approved) = outcome else {
                panic!("expected approval for {valid}: {outcome:?}");
            };
            assert_eq!(approved.text().as_str(), valid);
            assert_eq!(
                approved.digest(),
                &WebSearchQueryDigest::of_query(&candidate)
            );
            let debug = format!("{approved:?}");
            assert!(debug.contains("ApprovedWebSearchQuery"), "{debug}");
            assert!(!debug.contains(valid), "{debug}");
            assert_eq!(
                format!("{:?}", approved.text()),
                "WebSearchQueryText(<redacted>)"
            );
        }
    }

    #[test]
    fn max_length_boundary_queries_are_approved() {
        for max_valid in ["a".repeat(200_usize), "가".repeat(200_usize)] {
            let outcome =
                inspect_web_search_query(&WebSearchCandidateQuery::new(&max_valid), &empty_ctx());
            match outcome {
                WebSearchInspection::Approved(_) => {}
                WebSearchInspection::Rejected(rejected) => {
                    panic!("expected exact max length approval: {rejected:?}");
                }
            }
        }
    }

    #[test]
    fn empty_overlong_control_and_obfuscated_queries_are_rejected() {
        let ctx = empty_ctx();
        assert_rejected_with("", &ctx, WebSearchRejectionReason::EmptyQuery);
        assert_rejected_with("   ", &ctx, WebSearchRejectionReason::EmptyQuery);
        assert_rejected_with(
            &"a".repeat(201_usize),
            &ctx,
            WebSearchRejectionReason::QueryTooLong,
        );
        assert_rejected_with(
            &"가".repeat(201_usize),
            &ctx,
            WebSearchRejectionReason::QueryTooLong,
        );
        assert_rejected_with(
            &"💰".repeat(151_usize),
            &ctx,
            WebSearchRejectionReason::QueryTooLong,
        );
        for ctrl in [
            "삼성전자\n실적",
            "삼성전자\u{200b}실적",
            "삼성전자\u{202e}실적",
            "삼성전자\u{0301}실적",
            "삼성전자\u{e0001}실적",
        ] {
            assert_rejected_with(
                ctrl,
                &ctx,
                WebSearchRejectionReason::ControlOrInvisibleCharacter,
            );
        }
        for obf in [
            "삼성전자\u{3000}주",
            "삼성전자 １２３",
            "삼성전자 ＡＣＣＯＵＮＴ",
            "삼성전자 ①②③④⑤⑥⑦⑧⑨⓪",
            "삼성전자 аеорсхуікмтнв",
            "삼성전자 %31%32%33",
            "삼성전자 %0a실적",
            "삼성전자 %41%43%43%4F%55%4E%54",
            "삼성전자 &#49;&#50;",
            "삼성전자 \\u0031",
            "삼성전자 \\x31",
            "삼성전자 0x1a",
            "삼성전자 0b1010",
            "삼성전자 xn--test",
            "삼성전자 MTIzNDU2Nzg5MDEy",
            "삼성전자 c2Vjcm==",
            "삼성전자 。실적",
            "삼성전자 ㅎㅗㅇㄱㅣㄹㄷㅗㅇ",
            "삼성전자 \u{1112}\u{1169}\u{11BC}",
            "AAPL àccount",
            "삼성전자 💰",
        ] {
            assert_rejected_with(obf, &ctx, WebSearchRejectionReason::EncodedOrObfuscatedText);
        }
    }

    #[test]
    fn empty_overlong_control_and_folding_secondary_reasons_are_checked() {
        let ctx = empty_ctx();
        for (query, reason) in [
            (
                "삼성전자 １２３",
                WebSearchRejectionReason::MultiDigitNumber,
            ),
            (
                "삼성전자 ＡＣＣＯＵＮＴ",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "삼성전자 аccount",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            ("삼성전자 ①②", WebSearchRejectionReason::MultiDigitNumber),
            ("evil。com", WebSearchRejectionReason::UrlOrDomainShape),
            (
                "삼성전자\u{3000}주",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "삼성전자 %31%32",
                WebSearchRejectionReason::MultiDigitNumber,
            ),
            (
                "삼성전자 %41%43%43%4F%55%4E%54",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
        ] {
            assert_rejected_with(query, &ctx, reason);
        }
        let exact_currency =
            inspect_web_search_query(&WebSearchCandidateQuery::new("삼성전자 ₩"), &ctx);
        let WebSearchInspection::Rejected(currency_rej) = exact_currency else {
            panic!("expected currency rejection");
        };
        assert_eq!(
            currency_rej.reasons(),
            &[WebSearchRejectionReason::MoneyQuantityOrAccountTerm]
        );
        let exact_hanja =
            inspect_web_search_query(&WebSearchCandidateQuery::new("삼성전자 三百五十"), &ctx);
        let WebSearchInspection::Rejected(hanja_rej) = exact_hanja else {
            panic!("expected hanja rejection");
        };
        assert_eq!(
            hanja_rej.reasons(),
            &[WebSearchRejectionReason::SpelledOutNumber]
        );
    }

    #[test]
    fn url_domain_and_multi_digit_queries_are_rejected() {
        let ctx = empty_ctx();
        for url_case in [
            "https://evil.com/leak",
            "http://evil.com/a",
            "www.evil.com",
            "evil.com",
            "evil.com/a",
            "sub.evil.kr",
            "evil[.]com",
            "evil(.)net",
            "evil dot com",
            "user@evil.com",
            "127.0.0.1",
            "a.b.c",
            "/api/account",
            "a/b/c",
            "삼성전자?q=leak",
            "삼성전자 key=val",
            "삼성전자 \\evil",
            "삼성전자 ::1",
        ] {
            assert_rejected_with(url_case, &ctx, WebSearchRejectionReason::UrlOrDomainShape);
        }
        for multi_digit_case in [
            "삼성전자 12",
            "삼성전자 005930 실적",
            "삼성전자 2026 실적",
            "삼성전자 1999년 실적",
            "삼성전자 2025년 2026년 2027년 실적",
            "삼성전자 2024년 1분기 2025년 2분기 2026년 3분기 실적",
            "삼성전자 1 2 3 4",
            "삼성전자 1-2-3",
            "삼성전자 3.5",
            "삼성전자 1a2",
        ] {
            assert_rejected_with(
                multi_digit_case,
                &ctx,
                WebSearchRejectionReason::MultiDigitNumber,
            );
        }
    }

    #[test]
    fn spelled_number_and_finance_term_queries_are_rejected() {
        let ctx = empty_ctx();
        for spelled_case in [
            "삼성전자 삼백오십",
            "삼성전자 삼 백 오 십",
            "삼성전자 삼-백-오-십",
            "삼성전자 일이삼사",
            "삼성전자 삼오",
            "삼성전자 스물다섯",
            "삼성전자 열다섯",
            "삼성전자 다섯 주",
            "삼성전자 다섯주",
            "삼성전자 하나 둘",
            "AAPL twenty five",
            "AAPL three hundred",
            "AAPL one two",
            "AAPL t w e n t y",
            "삼성전자 三百五十",
        ] {
            assert_rejected_with(
                spelled_case,
                &ctx,
                WebSearchRejectionReason::SpelledOutNumber,
            );
        }
        for term_case in [
            "삼성전자 ₩",
            "AAPL $",
            "삼성전자 보유 수량",
            "삼성전자 계 좌",
            "삼성전자 계-좌",
            "삼성전자 보_유",
            "삼성전자 평단가",
            "삼성전자 5주",
            "삼성전자 5원",
            "삼성전자 주",
            "AAPL shares",
            "AAPL a c c o u n t",
            "AAPL a-c-c-o-u-n-t",
            "AAPL portfolio",
            "AAPL balance",
            "AAPL krw",
            "삼성전자 주민번호",
        ] {
            assert_rejected_with(
                term_case,
                &ctx,
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            );
        }
    }

    #[test]
    fn sensitive_context_blocks_accounts_holdings_amounts_and_personal_names() {
        let ctx = WebSearchSensitiveContext::new()
            .with_account_number(SensitiveAccountNumber::try_new("123-45-678901").expect("acct"))
            .with_masked_account(MaskedAccountNumber::try_new("999992026").expect("masked"))
            .with_holding_quantity(quantity(7_i64))
            .with_holding_quantity(quantity(0_i64))
            .with_holding_quantity(Quantity::try_new(Decimal::new(5_i64, 2_u32)).expect("frac qty"))
            .with_holding_amount(amount(5_430_000_i64))
            .with_personal_name(SensitivePersonalName::try_new("홍길동").expect("name"))
            .with_personal_name(SensitivePersonalName::try_new("Gil Dong").expect("en name"));
        for leak in [
            "삼성전자 12345678901",
            "삼성전자 8901",
            "삼성전자 678901",
            "삼성전자 123",
            "삼성전자 2026년 실적",
            "삼성전자 7 실적",
            "삼성전자 5430000",
            "삼성전자 5,430,000",
            "삼성전자 0.05",
            "삼성전자 홍길동",
            "삼성전자 홍 길 동",
            "삼성전자 홍-길-동",
            "AAPL Gil Dong",
            "AAPL gildong",
            "삼성전자 １２３４５６７８９０１",
            "삼성전자 %38%39%30%31",
        ] {
            assert_rejected_with(leak, &ctx, WebSearchRejectionReason::SensitiveContextMatch);
        }
        let single_share_ctx = WebSearchSensitiveContext::new()
            .with_holding_quantity(quantity(3_i64))
            .with_holding_quantity(quantity(20_i64))
            .with_holding_amount(amount(0_i64));
        let quarter_ok = inspect_web_search_query(
            &WebSearchCandidateQuery::new("삼성전자 2026년 3분기 실적"),
            &single_share_ctx,
        );
        match quarter_ok {
            WebSearchInspection::Approved(_) => {}
            WebSearchInspection::Rejected(rejected) => {
                panic!("expected quarter allowlist pass: {rejected:?}");
            }
        }
        assert_rejected_with(
            "삼성전자 3 실적",
            &single_share_ctx,
            WebSearchRejectionReason::SensitiveContextMatch,
        );
        assert_rejected_with(
            "삼성전자 20 실적",
            &single_share_ctx,
            WebSearchRejectionReason::SensitiveContextMatch,
        );
        assert_rejected_with(
            "삼성전자 2 0 실적",
            &single_share_ctx,
            WebSearchRejectionReason::SensitiveContextMatch,
        );
    }

    struct MixedCase<'a> {
        qty_raw: i64,
        amt_raw: i64,
        acct_str: &'a str,
        stock: &'a str,
        topic: &'a str,
        shape: u8,
    }

    fn build_mixed_sensitive_query(case: &MixedCase<'_>) -> String {
        let last_four: String = case
            .acct_str
            .chars()
            .skip(case.acct_str.len().saturating_sub(4_usize))
            .collect();
        let spaced_qty: String = case
            .qty_raw
            .to_string()
            .chars()
            .map(|ch| ch.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let fullwidth_amt: String = case
            .amt_raw
            .to_string()
            .chars()
            .filter_map(|ch| {
                u32::from(ch)
                    .checked_add(0xFEE0_u32)
                    .and_then(char::from_u32)
            })
            .collect();
        let stock = case.stock;
        let topic = case.topic;
        let qty_raw = case.qty_raw;
        let amt_raw = case.amt_raw;
        let acct_str = case.acct_str;
        match case.shape {
            0_u8 => format!("{stock} {qty_raw} {topic}"),
            1_u8 => format!("{stock} {amt_raw} {topic}"),
            2_u8 => format!("{stock} {acct_str} {topic}"),
            3_u8 => format!("{stock} {last_four} {topic}"),
            4_u8 => format!("{stock} {spaced_qty} {topic}"),
            5_u8 => format!("{stock} {fullwidth_amt} {topic}"),
            6_u8 => format!("{stock} {qty_raw}주 {topic}"),
            7_u8 => format!("{stock} {amt_raw}원 {topic}"),
            _ => format!("{stock} 2026년 3분기 {last_four} {topic}"),
        }
    }

    #[test]
    fn property_test_mixed_holdings_amounts_and_accounts_pass_zero_queries() {
        let strategy = (
            1_i64..=999_999_i64,
            1_i64..=999_999_999_i64,
            10_000_000_u64..=999_999_999_999_u64,
            prop_oneof![
                Just("삼성전자"),
                Just("SK하이닉스"),
                Just("AAPL"),
                Just("NVDA"),
            ],
            prop_oneof![
                Just("실적"),
                Just("공시"),
                Just("earnings"),
                Just("outlook")
            ],
            0_u8..=8_u8,
        );
        let passed = AtomicUsize::new(0_usize);
        TestRunner::new(Config::with_cases(testkit::property_cases()))
            .run(
                &strategy,
                |(qty_raw, amt_raw, acct_raw, stock, topic, shape)| {
                    let acct_str = acct_raw.to_string();
                    let ctx = WebSearchSensitiveContext::new()
                        .with_account_number(
                            SensitiveAccountNumber::try_new(&acct_str).expect("acct"),
                        )
                        .with_masked_account(
                            MaskedAccountNumber::try_new(&acct_str).expect("masked"),
                        )
                        .with_holding_quantity(quantity(qty_raw))
                        .with_holding_amount(amount(amt_raw));
                    let raw_query = build_mixed_sensitive_query(&MixedCase {
                        qty_raw,
                        amt_raw,
                        acct_str: &acct_str,
                        stock,
                        topic,
                        shape,
                    });
                    let outcome =
                        inspect_web_search_query(&WebSearchCandidateQuery::new(&raw_query), &ctx);
                    match outcome {
                        WebSearchInspection::Approved(_) => {
                            passed.fetch_add(1_usize, Ordering::SeqCst);
                            proptest::prop_assert!(false, "leaked query approved: {raw_query}");
                        }
                        WebSearchInspection::Rejected(_) => {}
                    }
                    Ok(())
                },
            )
            .expect("all sensitive mixed queries must be rejected");
        assert_eq!(passed.load(Ordering::SeqCst), 0_usize);
    }

    #[test]
    fn punctuation_wrapped_obfuscation_and_domain_variants_are_rejected() {
        let ctx = empty_ctx();
        for (query, reason) in [
            (
                "삼성전자 (0x1a)",
                WebSearchRejectionReason::EncodedOrObfuscatedText,
            ),
            (
                "삼성전자 [0b1010]",
                WebSearchRejectionReason::EncodedOrObfuscatedText,
            ),
            (
                "삼성전자 (MTIzNDU2Nzg5MDEy)",
                WebSearchRejectionReason::EncodedOrObfuscatedText,
            ),
            (
                "삼성전자 {MTIzNDU2Nzg5MDEy}",
                WebSearchRejectionReason::EncodedOrObfuscatedText,
            ),
            ("evil.com.", WebSearchRejectionReason::UrlOrDomainShape),
            ("evil.com:8", WebSearchRejectionReason::UrlOrDomainShape),
            ("evil.shop/leak", WebSearchRejectionReason::UrlOrDomainShape),
            ("<evil.com>", WebSearchRejectionReason::UrlOrDomainShape),
            ("{evil.com}", WebSearchRejectionReason::UrlOrDomainShape),
            ("evil.com!", WebSearchRejectionReason::UrlOrDomainShape),
            (
                "삼성전자 계.좌",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "삼성전자 계/좌",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "AAPL a.c.c.o.u.n.t",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "삼성전자 (원)",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "삼성전자 삼.백.오.십",
                WebSearchRejectionReason::SpelledOutNumber,
            ),
        ] {
            assert_rejected_with(query, &ctx, reason);
        }
    }

    #[test]
    fn fractional_sensitive_values_block_truncated_integer_part() {
        let frac_ctx = WebSearchSensitiveContext::new()
            .with_holding_quantity(
                Quantity::try_new(Decimal::new(202_650_i64, 2_u32)).expect("qty"),
            )
            .with_holding_amount(Amount::parse(&domain::AmountInput::new("2027.50")).expect("amt"));
        assert_rejected_with(
            "삼성전자 2026.5",
            &frac_ctx,
            WebSearchRejectionReason::SensitiveContextMatch,
        );
        assert_rejected_with(
            "삼성전자 2026년 실적",
            &frac_ctx,
            WebSearchRejectionReason::SensitiveContextMatch,
        );
        assert_rejected_with(
            "삼성전자 2027년 실적",
            &frac_ctx,
            WebSearchRejectionReason::SensitiveContextMatch,
        );
    }

    #[test]
    fn delimiter_glued_domains_localhost_ports_and_multi_word_punct_obfuscation_are_rejected() {
        let ctx = empty_ctx();
        for (query, reason) in [
            ("evil.shop", WebSearchRejectionReason::UrlOrDomainShape),
            ("leak.blog", WebSearchRejectionReason::UrlOrDomainShape),
            (
                "evil.com#secret",
                WebSearchRejectionReason::UrlOrDomainShape,
            ),
            (
                "삼성전자,evil.com",
                WebSearchRejectionReason::UrlOrDomainShape,
            ),
            ("a(evil.com)", WebSearchRejectionReason::UrlOrDomainShape),
            ("localhost", WebSearchRejectionReason::UrlOrDomainShape),
            ("localhost:8", WebSearchRejectionReason::UrlOrDomainShape),
            ("localhost/leak", WebSearchRejectionReason::UrlOrDomainShape),
            ("host:8", WebSearchRejectionReason::UrlOrDomainShape),
            (
                "삼성전자,원",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "AAPL a.c.c.o.u.n.t b.a.l.a.n.c.e",
                WebSearchRejectionReason::MoneyQuantityOrAccountTerm,
            ),
            (
                "AAPL t.w.e.n.t.y f.i.v.e",
                WebSearchRejectionReason::SpelledOutNumber,
            ),
            (
                "AAPL o.n.e t.w.o",
                WebSearchRejectionReason::SpelledOutNumber,
            ),
            (
                "삼성전자 하.나 다.섯",
                WebSearchRejectionReason::SpelledOutNumber,
            ),
        ] {
            assert_rejected_with(query, &ctx, reason);
        }
        for safe in [
            "U.S. treasury yield",
            "BRK.B,AAPL earnings",
            "AAPL P:E ratio",
        ] {
            match inspect_web_search_query(&WebSearchCandidateQuery::new(safe), &ctx) {
                WebSearchInspection::Approved(ok) => assert_eq!(ok.text().as_str(), safe),
                WebSearchInspection::Rejected(rejected) => {
                    panic!("expected safe query '{safe}' approved, got {rejected:?}");
                }
            }
        }
    }
}
