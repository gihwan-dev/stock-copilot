#[cfg(test)]
mod tests {
    use domain::{
        AuthorizedExternalUrl, ExternalUrlAllowance, ExternalUrlHost, ExternalUrlInput,
        ExternalUrlPath, ExternalUrlPort, ExternalUrlQuery, ExternalUrlRejection,
        ExternalUrlScheme, FixedExternalUrl, NormalizedUrl, ParsedExternalUrl,
    };
    use proptest::prelude::{Strategy, prop_assert, prop_assert_eq};
    use proptest::test_runner::{Config, TestRunner};

    fn parse_raw(raw: &str) -> Result<ParsedExternalUrl, ExternalUrlRejection> {
        ParsedExternalUrl::parse(&ExternalUrlInput::new(raw))
    }

    fn fixed_anthropic_url() -> &'static str {
        "https://claude.ai/settings/data-privacy-controls"
    }

    #[test]
    fn fixed_url_matches_exact_parsed_structure_and_exposes_canonical_text() {
        let fixed = FixedExternalUrl::AnthropicDataPrivacyControls;
        let expected = fixed.parsed_url();
        assert_eq!(expected.scheme(), ExternalUrlScheme::Https);
        assert_eq!(expected.host().as_str(), "claude.ai");
        assert_eq!(expected.port(), ExternalUrlPort::ImplicitHttps);
        assert_eq!(expected.path().as_str(), "/settings/data-privacy-controls");
        assert_eq!(expected.query(), None);
        assert_eq!(expected.canonical_url().as_str(), fixed_anthropic_url());

        let parsed = parse_raw(fixed_anthropic_url()).expect("고정 주소 파싱");
        assert_eq!(parsed, expected);
        assert_eq!(FixedExternalUrl::match_parsed(&parsed), Some(fixed));

        let authorized =
            AuthorizedExternalUrl::authorize_fixed(parsed.clone()).expect("고정 주소 인가");
        assert_eq!(authorized.parsed(), &parsed);
        assert_eq!(authorized.allowance(), ExternalUrlAllowance::Fixed(fixed));
        assert_eq!(authorized.canonical_url().as_str(), fixed_anthropic_url());
    }

    #[test]
    fn external_url_input_debug_redacts_raw_text() {
        let secret = "https://example.com/article?token=secret-value-123";
        let input = ExternalUrlInput::new(secret);
        assert_eq!(input.as_str(), secret);
        let rendered = format!("{input:?}");
        assert!(!rendered.contains("secret-value-123"), "{rendered}");
        assert!(!rendered.contains("example.com"), "{rendered}");
        assert!(
            rendered.contains(&format!("byte_len: {}", secret.len())),
            "{rendered}"
        );
    }

    #[test]
    fn rejects_non_https_schemes_including_case_and_custom_schemes() {
        for raw in [
            "http://claude.ai/settings/data-privacy-controls",
            "http://www.mk.co.kr/news/stock/12205485",
            "HTTP://claude.ai/settings/data-privacy-controls",
            "HTTPS://claude.ai/settings/data-privacy-controls",
            "HtTpS://claude.ai/settings/data-privacy-controls",
            "https ://claude.ai/settings/data-privacy-controls",
            "file:///etc/passwd",
            "file://localhost/etc/passwd",
            "FILE:///etc/passwd",
            "javascript:alert(1)",
            "JavaScript:alert(1)",
            "data:text/html,<script>alert(1)</script>",
            "DATA:text/plain,hello",
            "vbscript:msgbox(1)",
            "ftp://example.com/file",
            "mailto:user@example.com",
            "tel:+821012345678",
            "about:blank",
            "tauri://localhost/index.html",
            "stock-copilot://open/1",
            "x-apple.systempreferences:com.apple.preference.security",
            "//claude.ai/settings/data-privacy-controls",
            "/settings/data-privacy-controls",
            "claude.ai/settings/data-privacy-controls",
        ] {
            let result = parse_raw(raw);
            assert!(result.is_err(), "비-https 허용됨: {raw} -> {result:?}");
        }
        for exact in [
            "http://claude.ai/settings/data-privacy-controls",
            "HTTPS://claude.ai/settings/data-privacy-controls",
            "javascript:alert(1)",
            "file:///etc/passwd",
            "data:text/html,ok",
            "custom-scheme://example.com/a",
        ] {
            assert_eq!(
                parse_raw(exact),
                Err(ExternalUrlRejection::NonHttpsScheme),
                "{exact}"
            );
        }
    }

    #[test]
    fn rejects_userinfo_ports_trailing_dot_and_punycode_hosts() {
        for (raw, expected) in [
            (
                "https://user:pass@claude.ai/settings/data-privacy-controls",
                ExternalUrlRejection::UserInfoNotAllowed,
            ),
            (
                "https://user@claude.ai/settings/data-privacy-controls",
                ExternalUrlRejection::UserInfoNotAllowed,
            ),
            (
                "https://@claude.ai/settings/data-privacy-controls",
                ExternalUrlRejection::UserInfoNotAllowed,
            ),
            (
                "https://claude.ai@evil.com/settings/data-privacy-controls",
                ExternalUrlRejection::UserInfoNotAllowed,
            ),
            (
                "https://claude.ai:443/settings/data-privacy-controls",
                ExternalUrlRejection::ExplicitPortNotAllowed,
            ),
            (
                "https://claude.ai:8443/settings/data-privacy-controls",
                ExternalUrlRejection::ExplicitPortNotAllowed,
            ),
            (
                "https://claude.ai:/settings/data-privacy-controls",
                ExternalUrlRejection::ExplicitPortNotAllowed,
            ),
            (
                "https://[::1]/settings/data-privacy-controls",
                ExternalUrlRejection::ExplicitPortNotAllowed,
            ),
            (
                "https://claude.ai./settings/data-privacy-controls",
                ExternalUrlRejection::TrailingDotNotAllowed,
            ),
            (
                "https://www.mk.co.kr./news/stock/12205485",
                ExternalUrlRejection::TrailingDotNotAllowed,
            ),
            (
                "https://xn--clade-xsa.ai/settings/data-privacy-controls",
                ExternalUrlRejection::PunycodeNotAllowed,
            ),
            (
                "https://XN--clade-xsa.ai/settings/data-privacy-controls",
                ExternalUrlRejection::PunycodeNotAllowed,
            ),
            (
                "https://sub.Xn--80ak6aa92e.com/a",
                ExternalUrlRejection::PunycodeNotAllowed,
            ),
        ] {
            assert_eq!(parse_raw(raw), Err(expected), "{raw}");
        }
    }

    #[test]
    fn rejects_case_host_tricks_ip_literals_and_malformed_hosts() {
        for raw in [
            "https://CLAUDE.AI/settings/data-privacy-controls",
            "https://Claude.Ai/settings/data-privacy-controls",
            "https://www.MK.co.kr/news/stock/12205485",
            "https:///settings/data-privacy-controls",
            "https://localhost/a",
            "https://sub.localhost/a",
            "https://printer.local/a",
            "https://.claude.ai/a",
            "https://claude..ai/a",
            "https://-claude.ai/a",
            "https://claude-.ai/a",
            "https://claude_ai.com/a",
            "https://claude%2eai/a",
            "https://127.0.0.1/a",
            "https://127.1/a",
            "https://0x7f000001/a",
            "https://1.2.3.0x4/a",
            "https://example.123/a",
            "https://example.0x1f/a",
            "https://example.a/a",
            "https://example.a1/a",
        ] {
            assert_eq!(
                parse_raw(raw),
                Err(ExternalUrlRejection::InvalidHost),
                "{raw}"
            );
        }
    }

    #[test]
    fn enforces_dns_host_and_label_length_boundaries() {
        let label_63 = "a".repeat(63);
        let label_63_url = format!("https://{label_63}.com/a");
        assert_eq!(
            parse_raw(&label_63_url)
                .expect("63자리 레이블")
                .canonical_url()
                .as_str(),
            label_63_url
        );
        assert_eq!(
            parse_raw(&format!("https://{}.com/a", "a".repeat(64))),
            Err(ExternalUrlRejection::InvalidHost)
        );
        let host_253 = [
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61),
        ]
        .join(".");
        assert_eq!(host_253.len(), 253);
        let host_253_url = format!("https://{host_253}/a");
        assert_eq!(
            parse_raw(&host_253_url)
                .expect("253자리 호스트")
                .canonical_url()
                .as_str(),
            host_253_url
        );
        assert_eq!(
            parse_raw(&format!("https://{host_253}e/a")),
            Err(ExternalUrlRejection::InvalidHost)
        );
    }

    #[test]
    fn rejects_backslashes_whitespace_control_chars_unicode_and_length_bounds() {
        assert_eq!(parse_raw(""), Err(ExternalUrlRejection::Empty));
        let exact_max = format!("https://example.com/{}", "a".repeat(8172));
        assert_eq!(exact_max.len(), 8192);
        assert_eq!(
            parse_raw(&exact_max)
                .expect("최대 길이")
                .canonical_url()
                .as_str(),
            exact_max
        );
        assert_eq!(
            parse_raw(&format!("{exact_max}a")),
            Err(ExternalUrlRejection::TooLong)
        );
        for raw in [
            r"https://claude.ai\@evil.com/settings/data-privacy-controls",
            r"https://claude.ai\settings/data-privacy-controls",
            r"https:\\claude.ai/settings/data-privacy-controls",
            r"https://claude.ai/settings\data-privacy-controls",
            r"https://example.com/a?x=1\2",
            " https://claude.ai/settings/data-privacy-controls",
            "https://claude.ai/settings/data-privacy-controls ",
            "https://claude.ai/settings/data-privacy-controls\t",
            "https://claude.ai/settings/data-privacy-controls\r",
            "https://claude.ai/settings/data-privacy-controls\n",
            "https://claude.ai/settings/\x00data-privacy-controls",
            "https://claude.ai/settings/\x1fdata-privacy-controls",
            "https://claude.ai/settings/\x7fdata-privacy-controls",
            "https://\u{0441}laude.ai/settings/data-privacy-controls",
            "https://claude.ai/settings/data-privacy-controls\u{200b}",
            "https://claude.ai/settings/data-privacy-controls\u{202e}",
            "https://claude.ai\u{ff0f}settings/data-privacy-controls",
            "https://example.com/<script>",
            "https://example.com/\"quoted\"",
            "https://example.com/{brace}",
            "https://example.com/|pipe",
            "https://example.com/^caret",
            "https://example.com/`tick",
            "https://example.com/[bracket]",
        ] {
            assert_eq!(
                parse_raw(raw),
                Err(ExternalUrlRejection::ForbiddenCharacter),
                "{raw:?}"
            );
        }
    }

    #[test]
    fn rejects_fragments_and_percent_encoding_tricks() {
        assert_eq!(
            parse_raw("https://claude.ai/settings/data-privacy-controls#section"),
            Err(ExternalUrlRejection::FragmentNotAllowed)
        );
        assert_eq!(
            parse_raw("https://claude.ai/settings/data-privacy-controls#"),
            Err(ExternalUrlRejection::FragmentNotAllowed)
        );
        for raw in [
            "https://claude.ai/settings/%",
            "https://claude.ai/settings/%2",
            "https://claude.ai/settings/%GG",
            "https://claude.ai/settings/%2G",
            "https://claude.ai/settings/%2f",
            "https://claude.ai/settings/%2F",
            "https://claude.ai/settings/%5c",
            "https://claude.ai/settings/%5C",
            "https://claude.ai/settings/%00",
            "https://claude.ai/settings/%0A",
            "https://claude.ai/settings/%0D",
            "https://claude.ai/settings/%1F",
            "https://claude.ai/settings/%7F",
            "https://claude.ai/settings/%252F",
            "https://claude.ai/settings/%255C",
            "https://claude.ai/settings/%252E",
            "https://claude.ai/settings/%2500",
            "https://claude.ai/settings/%257F",
            "https://claude.ai/settings/%ed%95%9c",
            "https://claude.ai/settings/%Ed",
            "https://example.com/a?x=%00",
            "https://example.com/a?x=%0A",
            "https://example.com/a?x=%5C",
            "https://example.com/a?x=%7F",
            "https://example.com/a?x=%ed",
            "https://example.com/a?x=%Ed",
            "https://example.com/a?x=%2",
            "https://example.com/a?x=%ZZ",
        ] {
            assert_eq!(
                parse_raw(raw),
                Err(ExternalUrlRejection::InvalidPercentEncoding),
                "{raw}"
            );
        }
        for allowed in [
            "https://example.com/a%2530",
            "https://example.com/a%25",
            "https://example.com/a%25ZZ",
        ] {
            assert_eq!(
                parse_raw(allowed)
                    .expect("허용 이중 인코딩 외")
                    .canonical_url()
                    .as_str(),
                allowed
            );
        }
    }

    #[test]
    fn rejects_invalid_paths_dot_segments_and_scheme_in_path() {
        for raw in [
            "https://claude.ai",
            "https://claude.ai?a=1",
            "https://claude.ai//settings/data-privacy-controls",
            "https://example.com//evil.com",
            "https://claude.ai/settings/./data-privacy-controls",
            "https://claude.ai/settings/../settings/data-privacy-controls",
            "https://claude.ai/.",
            "https://claude.ai/..",
            "https://claude.ai/%2E",
            "https://claude.ai/%2E%2E/a",
            "https://claude.ai/.%2E/a",
            "https://claude.ai/https://evil.com",
            "https://claude.ai/http:/evil.com",
            "https://claude.ai/javascript:alert(1)",
            "https://claude.ai/data:text/html,x",
            "https://claude.ai/file:/etc/passwd",
            "https://claude.ai/vbscript:x",
            "https://claude.ai/blob:x",
            "https://claude.ai/about:blank",
            "https://claude.ai/%20javascript:alert(1)",
            "https://claude.ai?next=https://evil.com",
        ] {
            assert_eq!(
                parse_raw(raw),
                Err(ExternalUrlRejection::InvalidPath),
                "{raw}"
            );
        }
    }

    #[test]
    fn rejects_empty_query_forms_and_open_redirect_queries() {
        for raw in [
            "https://example.com/a?",
            "https://example.com/a?&",
            "https://example.com/a?a=1&",
            "https://example.com/a?a=1&&b=2",
            "https://example.com/a?&b=2",
        ] {
            assert_eq!(
                parse_raw(raw),
                Err(ExternalUrlRejection::InvalidQuery),
                "{raw}"
            );
        }
        for raw in [
            "https://claude.ai/redirect?url=https://evil.com/phish",
            "https://claude.ai/redirect?url=http://evil.com/phish",
            "https://claude.ai/out?next=https%3A%2F%2Fevil.com",
            "https://claude.ai/out?next=https%253A%252F%252Fevil.com",
            "https://claude.ai/out?to=//evil.com/phish",
            "https://claude.ai/out?to=%2F%2Fevil.com/phish",
            "https://claude.ai/out?to=%252F%252Fevil.com/phish",
            "https://claude.ai/out?to=javascript:alert(1)",
            "https://claude.ai/out?to=javascript%3Aalert(1)",
            "https://claude.ai/out?to=data:text/html,ok",
            "https://claude.ai/out?to=file:/etc/passwd",
            "https://claude.ai/out?to=http:evil.com",
            "https://claude.ai/out?to=https:evil.com",
            "https://claude.ai/out?to=vbscript:run",
            "https://claude.ai/out?to=blob:x",
            "https://claude.ai/out?to=about:blank",
            "https://claude.ai/out?https://evil.com",
            "https://claude.ai/out?to=a%255Cb",
            "https://claude.ai/out?to=%20//evil.com",
            "https://claude.ai/out?to=%20javascript:alert(1)",
        ] {
            assert_eq!(
                parse_raw(raw),
                Err(ExternalUrlRejection::OpenRedirectNotAllowed),
                "{raw}"
            );
        }
        let benign_query = "https://example.com/a?flag&x=a%25ZZ&y=a%25";
        assert_eq!(
            parse_raw(benign_query)
                .expect("일반 쿼리")
                .canonical_url()
                .as_str(),
            benign_query
        );
    }

    #[test]
    fn fixed_url_rejects_prefix_spoofing_case_path_and_extra_query() {
        for candidate in [
            "https://claude.ai/settings/data-privacy-controls/",
            "https://claude.ai/settings/data-privacy-controls/extra",
            "https://claude.ai/settings/data-privacy-controls-extra",
            "https://claude.ai/settings/data-privacy-controls?next=1",
            "https://claude.ai/settings/data-privacy-controls?utm_source=app",
            "https://claude.ai/Settings/data-privacy-controls",
            "https://claude.ai/%73ettings/data-privacy-controls",
            "https://claude.ai/redirect",
            "https://claude.ai/settings",
            "https://claude.ai/",
            "https://claude.ai.evil.com/settings/data-privacy-controls",
            "https://sub.claude.ai/settings/data-privacy-controls",
        ] {
            let parsed = parse_raw(candidate).expect("구조 파싱 가능하지만 비허용");
            assert_eq!(FixedExternalUrl::match_parsed(&parsed), None, "{candidate}");
            assert_eq!(
                AuthorizedExternalUrl::authorize_fixed(parsed),
                None,
                "{candidate}"
            );
        }
    }

    #[test]
    fn collected_url_authorization_requires_exact_https_structural_match() {
        let real_samples = [
            "https://kind.krx.co.kr/disclosure/rsstodaydistribute.do?method=searchRssTodayDistribute&currentPageSize=100",
            "https://dart.fss.or.kr/dsaf001/main.do?rcpNo=20261111800749",
            "https://www.yna.co.kr/view/AKR20261111157200030?input=1195m",
            "https://www.hankyung.com/article/202611110253i",
            "https://www.mk.co.kr/news/stock/12205485",
            "https://news.google.com/rss/articles/CBMi?hl=ko&gl=KR",
            "https://feeds.finance.yahoo.com/rss/2.0/headline?s=AAPL&region=US&lang=en-US",
            "https://www.bok.or.kr/portal/bbs/B0000552/news.rss?menuNo=200690",
            "https://www.federalreserve.gov/feeds/press_all.xml",
            "https://example.com/%ED%95%9C%EA%B8%80?time=09:30",
        ];
        for sample in real_samples {
            let collected = NormalizedUrl::try_new(sample).expect("수집 주소");
            let requested = parse_raw(sample).expect("요청 주소");
            let authorized =
                AuthorizedExternalUrl::authorize_collected(requested.clone(), &collected)
                    .expect("수집 일치 인가");
            assert_eq!(authorized.allowance(), ExternalUrlAllowance::Collected);
            assert_eq!(authorized.parsed(), &requested);
            assert_eq!(authorized.canonical_url().as_str(), sample);
        }

        let collected_http =
            NormalizedUrl::try_new("http://www.edaily.co.kr/news/newspath.asp?newsid=1")
                .expect("http 수집 주소");
        assert_eq!(
            ParsedExternalUrl::from_collected(&collected_http),
            Err(ExternalUrlRejection::NonHttpsScheme)
        );
        let requested_https =
            parse_raw("https://www.edaily.co.kr/news/newspath.asp?newsid=1").expect("https 요청");
        assert_eq!(
            AuthorizedExternalUrl::authorize_collected(requested_https, &collected_http),
            Err(ExternalUrlRejection::NonHttpsScheme)
        );

        let collected_article =
            NormalizedUrl::try_new("https://www.mk.co.kr/news/stock/12205485").expect("수집 기사");
        for tampered in [
            "https://www.mk.co.kr/news/stock/122054859",
            "https://www.mk.co.kr/news/stock/12205485/extra",
            "https://www.mk.co.kr/news/stock/12205485?holdings=SAMSUNG_100",
            "https://www.mk.co.kr/news/stock/12205485?utm_source=rss",
            "https://www.mk.co.kr/News/stock/12205485",
            "https://m.mk.co.kr/news/stock/12205485",
            "https://www.mk.co.kr/redirect",
        ] {
            let requested = parse_raw(tampered).expect("변조 요청 파싱");
            assert_eq!(
                AuthorizedExternalUrl::authorize_collected(requested, &collected_article),
                Err(ExternalUrlRejection::NotAllowed),
                "{tampered}"
            );
        }
    }

    #[test]
    fn component_newtypes_enforce_direct_validation_boundaries() {
        let host = ExternalUrlHost::try_new("example.com").expect("호스트");
        assert_eq!(host.as_str(), "example.com");
        assert_eq!(
            ExternalUrlHost::try_new("example.com."),
            Err(ExternalUrlRejection::TrailingDotNotAllowed)
        );
        let path = ExternalUrlPath::try_new("/news/1").expect("경로");
        assert_eq!(path.as_str(), "/news/1");
        assert_eq!(
            ExternalUrlPath::try_new("news/1"),
            Err(ExternalUrlRejection::InvalidPath)
        );
        assert_eq!(
            ExternalUrlPath::try_new("/news/[1]"),
            Err(ExternalUrlRejection::ForbiddenCharacter)
        );
        let query = ExternalUrlQuery::try_new("id=1").expect("쿼리");
        assert_eq!(query.as_str(), "id=1");
        assert_eq!(
            ExternalUrlQuery::try_new(""),
            Err(ExternalUrlRejection::InvalidQuery)
        );
        assert_eq!(
            ExternalUrlQuery::try_new("id=[1]"),
            Err(ExternalUrlRejection::ForbiddenCharacter)
        );
    }

    #[test]
    fn property_host_scheme_and_character_bypasses_are_always_rejected() {
        let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
        let strategy = (
            prop_scheme(),
            "[a-z][a-z0-9]{0,8}",
            "[a-z][a-z0-9]{0,8}",
            "[a-zA-Z0-9][a-zA-Z0-9._~-]{0,11}",
            1_u16..=65535_u16,
        );
        runner
            .run(&strategy, |(scheme, host_a, host_b, segment, port)| {
                let host = format!("{host_a}.{host_b}.com");
                let clean_path = format!("/{segment}");
                let valid = format!("https://{host}{clean_path}");
                let parsed = parse_raw(&valid).expect("유효 https");
                let canonical = parsed.canonical_url();
                prop_assert_eq!(canonical.as_str(), valid.as_str());
                prop_assert_eq!(FixedExternalUrl::match_parsed(&parsed), None);
                let non_https = format!("{scheme}://{host}{clean_path}");
                prop_assert!(parse_raw(&non_https).is_err());
                prop_assert_eq!(
                    parse_raw(&format!("https://{host_a}:{host_b}@{host}{clean_path}")),
                    Err(ExternalUrlRejection::UserInfoNotAllowed)
                );
                prop_assert_eq!(
                    parse_raw(&format!("https://{host}:{port}{clean_path}")),
                    Err(ExternalUrlRejection::ExplicitPortNotAllowed)
                );
                prop_assert_eq!(
                    parse_raw(&format!("https://{host}.{clean_path}")),
                    Err(ExternalUrlRejection::TrailingDotNotAllowed)
                );
                prop_assert_eq!(
                    parse_raw(&format!("https://xn--{host_a}.{host_b}.com{clean_path}")),
                    Err(ExternalUrlRejection::PunycodeNotAllowed)
                );
                prop_assert_eq!(
                    parse_raw(&format!(
                        "https://{}{clean_path}",
                        host.to_ascii_uppercase()
                    )),
                    Err(ExternalUrlRejection::InvalidHost)
                );
                prop_assert_eq!(
                    parse_raw(&format!("https://{host}\\{segment}")),
                    Err(ExternalUrlRejection::ForbiddenCharacter)
                );
                prop_assert_eq!(
                    parse_raw(&format!("https://{host}{clean_path} ")),
                    Err(ExternalUrlRejection::ForbiddenCharacter)
                );
                prop_assert_eq!(
                    parse_raw(&format!("https://{host}{clean_path}\n")),
                    Err(ExternalUrlRejection::ForbiddenCharacter)
                );
                Ok(())
            })
            .expect("속성 테스트");
    }

    #[test]
    fn property_prefix_extensions_percent_tricks_and_open_redirects_never_match_allowed_urls() {
        let mut runner = TestRunner::new(Config::with_cases(testkit::property_cases()));
        let strategy = ("[a-zA-Z0-9_-]{1,12}", "[a-z]{1,8}", "[a-z]{2,8}");
        runner
            .run(&strategy, |(suffix, key, evil)| {
                let fixed_base = fixed_anthropic_url();
                for candidate in [
                    format!("{fixed_base}/{suffix}"),
                    format!("{fixed_base}{suffix}"),
                    format!("{fixed_base}?{key}={suffix}"),
                    format!("https://claude.ai.{evil}.com/settings/data-privacy-controls"),
                ] {
                    let parsed = parse_raw(&candidate).expect("변형 후보 파싱");
                    prop_assert_eq!(FixedExternalUrl::match_parsed(&parsed), None);
                    prop_assert_eq!(AuthorizedExternalUrl::authorize_fixed(parsed), None);
                }
                for redirect in [
                    format!("https://claude.ai/redirect?{key}=https://{evil}.com/p"),
                    format!("https://claude.ai/redirect?{key}=https%3A%2F%2F{evil}.com"),
                    format!("https://claude.ai/redirect?{key}=//{evil}.com"),
                ] {
                    prop_assert_eq!(
                        parse_raw(&redirect),
                        Err(ExternalUrlRejection::OpenRedirectNotAllowed)
                    );
                }
                prop_assert_eq!(
                    parse_raw(&format!("https://claude.ai/settings%2F{suffix}")),
                    Err(ExternalUrlRejection::InvalidPercentEncoding)
                );
                let collected_raw = format!("https://www.mk.co.kr/news/{suffix}");
                let collected = NormalizedUrl::try_new(&collected_raw).expect("수집 주소");
                let tampered =
                    parse_raw(&format!("{collected_raw}?{key}=leak")).expect("쿼리 추가");
                prop_assert_eq!(
                    AuthorizedExternalUrl::authorize_collected(tampered, &collected),
                    Err(ExternalUrlRejection::NotAllowed)
                );
                Ok(())
            })
            .expect("접두·리다이렉트 속성 테스트");
    }

    fn prop_scheme() -> impl Strategy<Value = String> {
        proptest::prop_oneof![
            proptest::strategy::Just(String::from("http")),
            proptest::strategy::Just(String::from("HTTP")),
            proptest::strategy::Just(String::from("HTTPS")),
            proptest::strategy::Just(String::from("HtTpS")),
            proptest::strategy::Just(String::from("file")),
            proptest::strategy::Just(String::from("ftp")),
            proptest::strategy::Just(String::from("javascript")),
            proptest::strategy::Just(String::from("data")),
            proptest::strategy::Just(String::from("tauri")),
            proptest::strategy::Just(String::from("custom")),
        ]
    }
}
