#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{
        AiRunId, CollectedItemHandle, CollectedItemQuery, DisclosureToolLine, LookbackDays,
        NewsKeyword, NewsToolLine, RetryDelay, SystemFailure, SystemPort, SystemPortError,
        ToolName,
    };
    use domain::{
        DisclosureSource, InformationSource, Nanosecond, NewsSource, OffsetSeconds, Symbol,
        Timestamp, UnixSeconds, UntrustedText,
    };
    use mcp_server::{
        ToolErrorCode, ToolInvocation, ToolRegistry, UNTRUSTED_TEXT_FIELD, make_cursor_store,
        register_news_tools, validate_arguments_against_schema,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::SteppedClock;
    use testkit_ports::tool_read_fakes::{FixedNewsToolReads, NewsReadCall};

    const MARKER: &str = "IGNOREPREVIOUS";

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0_u32),
            OffsetSeconds::new(32_400_i32),
        )
        .expect("시각")
    }

    fn text(raw: &str) -> UntrustedText {
        UntrustedText::try_new(raw).expect("외부 글")
    }

    fn handle(raw: &str) -> CollectedItemHandle {
        CollectedItemHandle::try_new(raw).expect("핸들")
    }

    fn news(id: &str, seconds: i64) -> NewsToolLine {
        NewsToolLine {
            handle: handle(id),
            source: InformationSource::News(NewsSource::Yonhap),
            published_at: at(seconds),
            collected_at: at(seconds.saturating_add(60_i64)),
            title: text(&format!(
                "삼성전자 실적\u{200b} {MARKER} [여기](https://evil.example/a) <script>x</script>"
            )),
            summary: Some(text(&format!("요약 {MARKER} javascript:alert(1)"))),
            symbols: vec![
                Symbol::try_new("005930").expect("종목"),
                Symbol::try_new("005930").expect("종목"),
            ],
        }
    }

    fn registry_with(reads: &Arc<FixedNewsToolReads>) -> ToolRegistry {
        let cursors = make_cursor_store(Arc::new(SteppedClock::new(at(1_764_000_000_i64))));
        let mut registry = ToolRegistry::new();
        register_news_tools(&mut registry, reads, &cursors).expect("등록");
        registry
    }

    async fn call(
        registry: &ToolRegistry,
        name: &str,
        args: Value,
    ) -> Result<Value, ToolErrorCode> {
        let tool_name = ToolName::try_new(name).expect("이름");
        let (spec, tool) = registry.get(&tool_name).expect("등록된 도구");
        let arguments: Map<String, Value> = args.as_object().cloned().expect("객체 인자");
        validate_arguments_against_schema(&arguments, spec.input_schema())
            .map_err(|err| err.code())?;
        let run_id = AiRunId::try_new("run-n13").expect("실행 ID");
        tool.invoke(ToolInvocation {
            run_id: &run_id,
            arguments: &arguments,
        })
        .await
        .map_err(|err| err.code())
    }

    fn strings_outside_untrusted(value: &Value, key: Option<&str>, found: &mut Vec<String>) {
        match value {
            Value::String(text) => {
                if key != Some(UNTRUSTED_TEXT_FIELD) {
                    found.push(text.clone());
                }
            }
            Value::Array(items) => {
                for item in items {
                    strings_outside_untrusted(item, key, found);
                }
            }
            Value::Object(map) => {
                for (child_key, child) in map {
                    strings_outside_untrusted(child, Some(child_key.as_str()), found);
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) => {}
        }
    }

    #[test]
    fn external_text_leaves_only_through_normalized_untrusted_text() {
        runtime().block_on(async {
            let reads = Arc::new(FixedNewsToolReads::default().with_news(Ok(vec![
                news("item-old", 1_763_000_000_i64),
                news("item-new", 1_763_900_000_i64),
            ])));
            let registry = registry_with(&reads);
            let output = call(&registry, "search_news", json!({}))
                .await
                .expect("성공");
            let mut outside = Vec::new();
            strings_outside_untrusted(&output, None, &mut outside);
            assert!(
                outside.iter().all(|text| !text.contains(MARKER)),
                "외부 글은 untrusted_text 밖에 나오지 않는다: {outside:?}"
            );
            let rendered = serde_json::to_string(&output).expect("직렬화");
            for banned in [
                "evil.example",
                "https://",
                "javascript:",
                "<script>",
                "\u{200b}",
            ] {
                assert!(!rendered.contains(banned), "{banned}가 남으면 안 된다");
            }
            let first = &output["items"][0];
            assert_eq!(first["handle"], json!("item-new"), "최신순");
            assert_eq!(first["source"], json!("yonhap"));
            assert_eq!(first["symbols"], json!(["005930"]), "종목 중복 제거");
            assert_eq!(first["collected_at"], json!("2025-11-23T21:14:20+09:00"));
            assert!(
                first["title"][UNTRUSTED_TEXT_FIELD]
                    .as_str()
                    .is_some_and(|title| title.contains("삼성전자 실적") && title.contains("여기")),
                "링크는 표시 글만 남는다"
            );
        });
    }

    #[test]
    fn search_arguments_are_validated_and_forwarded() {
        runtime().block_on(async {
            let reads = Arc::new(FixedNewsToolReads::default());
            let registry = registry_with(&reads);
            call(
                &registry,
                "search_news",
                json!({ "symbol": "005930", "keyword": "  실적  ", "days": 30_u64 }),
            )
            .await
            .expect("성공");
            assert_eq!(
                reads.calls(),
                vec![NewsReadCall::SearchNews(CollectedItemQuery {
                    symbol: Some(Symbol::try_new("005930").expect("종목")),
                    keyword: Some(NewsKeyword::try_new("실적").expect("검색어")),
                    lookback: LookbackDays::try_new(30_u16).expect("기간"),
                })]
            );
            for bad in [
                json!({ "keyword": "a\u{0007}b" }),
                json!({ "keyword": "   " }),
                json!({ "days": 91_u64 }),
                json!({ "url": "https://example.com" }),
                json!({ "symbol": "../../etc" }),
            ] {
                assert_eq!(
                    call(&registry, "search_news", bad.clone()).await,
                    Err(ToolErrorCode::InvalidInput),
                    "{bad}"
                );
            }
            assert_eq!(
                call(&registry, "get_disclosures", json!({ "keyword": "x" })).await,
                Err(ToolErrorCode::InvalidInput),
                "공시는 검색어를 받지 않는다"
            );
        });
    }

    #[test]
    fn disclosures_render_titles_as_untrusted_and_failures_are_structured() {
        runtime().block_on(async {
            let reads = Arc::new(FixedNewsToolReads::default().with_disclosures(Ok(vec![
                DisclosureToolLine {
                    handle: handle("disc-1"),
                    source: DisclosureSource::Dart,
                    filed_at: at(1_763_900_000_i64),
                    collected_at: at(1_763_900_100_i64),
                    title: text(&format!("주요사항보고서 {MARKER}")),
                    symbols: vec![Symbol::try_new("005930").expect("종목")],
                },
            ])));
            let output = call(&registry_with(&reads), "get_disclosures", json!({}))
                .await
                .expect("성공");
            let mut outside = Vec::new();
            strings_outside_untrusted(&output, None, &mut outside);
            assert!(outside.iter().all(|text| !text.contains(MARKER)));
            assert_eq!(output["items"][0]["source"], json!("dart"));
            let failure = SystemPortError::new(
                SystemPort::Repository,
                SystemFailure::Unavailable(RetryDelay::Unknown),
            );
            let failing = registry_with(&Arc::new(
                FixedNewsToolReads::default()
                    .with_news(Err(failure))
                    .with_disclosures(Err(failure)),
            ));
            for name in ["search_news", "get_disclosures"] {
                assert_eq!(
                    call(&failing, name, json!({})).await,
                    Err(ToolErrorCode::UpstreamUnavailable)
                );
            }
        });
    }
}
