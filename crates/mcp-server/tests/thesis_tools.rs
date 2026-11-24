#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use application::{AiRunId, CollectedItemHandle, ThesisRunMetadata, ToolName};
    use domain::{
        EvidenceRef, EvidenceUrl, InstrumentId, NewsSource, NormalizedUrl, Symbol, UntrustedText,
    };
    use mcp_server::{
        ToolErrorCode, ToolInvocation, ToolRegistry, register_thesis_tools,
        validate_arguments_against_schema,
    };
    use serde_json::{Map, Value, json};
    use testkit_ports::tool_read_fakes::{FixedThesisToolPort, LatestThesisRepository};

    fn runtime() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("테스트 런타임")
    }

    fn text(raw: &str) -> UntrustedText {
        UntrustedText::try_new(raw).expect("글")
    }

    fn registry() -> ToolRegistry {
        let news = EvidenceRef::News(
            NewsSource::Hankyung,
            EvidenceUrl::try_new(
                NormalizedUrl::try_new("https://www.hankyung.com/article/1").expect("주소"),
            )
            .expect("근거 주소"),
        );
        let port = Arc::new(FixedThesisToolPort::new(
            vec![(
                Symbol::try_new("005930").expect("종목"),
                InstrumentId::try_new("KR7005930003").expect("종목 ID"),
            )],
            vec![(CollectedItemHandle::try_new("news-1").expect("핸들"), news)],
            ThesisRunMetadata {
                model: text("model-x"),
                prompt_version: text("p1"),
            },
        ));
        let repository = Arc::new(LatestThesisRepository::default());
        let mut registry = ToolRegistry::new();
        register_thesis_tools(&mut registry, &port, &repository).expect("등록");
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
        let run_id = AiRunId::try_new("run-j02").expect("실행 ID");
        tool.invoke(ToolInvocation {
            run_id: &run_id,
            arguments: &arguments,
        })
        .await
        .map_err(|err| err.code())
    }

    fn add_claim(id: &str, evidence: &Value) -> Value {
        json!({
            "op": "add_claim",
            "claim_id": id,
            "role": "core",
            "statement": "메모리 가격 회복 [근거](https://evil.example)\u{200b}",
            "metric": "DRAM 고정가",
            "expected": "분기 5% 상승",
            "observe_via": "월간 고정가 발표",
            "refutation": "두 달 연속 하락",
            "reason": "공급 감산 지속",
            "evidence": evidence
        })
    }

    #[test]
    fn web_only_change_waits_for_review_and_collected_evidence_applies() {
        runtime().block_on(async {
            let registry = registry();
            let empty = call(&registry, "get_thesis", json!({ "symbol": "005930" }))
                .await
                .expect("빈 노트");
            assert_eq!(empty["version"], json!(0_u64));
            let web_only = call(
                &registry,
                "update_thesis",
                json!({
                    "symbol": "005930",
                    "base_version": 0_u64,
                    "changes": [add_claim("c1", &json!([
                        { "kind": "web", "value": "https://www.hankyung.com/article/1" }
                    ]))]
                }),
            )
            .await
            .expect("웹 근거 제출");
            assert_eq!(web_only["needs_review"], json!(1_u64), "웹뿐이면 확인 필요");
            assert_eq!(web_only["applied"], json!(0_u64));
            let pending = call(&registry, "get_thesis", json!({ "symbol": "005930" }))
                .await
                .expect("조회");
            assert_eq!(
                pending["claims"],
                json!([]),
                "보류된 주장은 반영되지 않는다"
            );
            assert_eq!(pending["pending_review_sequences"], json!([1_u64]));
            let applied = call(
                &registry,
                "update_thesis",
                json!({
                    "symbol": "005930",
                    "base_version": 1_u64,
                    "changes": [add_claim("c2", &json!([
                        { "kind": "collected", "value": "news-1" },
                        { "kind": "tool", "value": "tool:get_quote" }
                    ]))]
                }),
            )
            .await
            .expect("수집 근거 제출");
            assert_eq!(applied["applied"], json!(1_u64), "언론사 수집 근거면 반영");
            assert_eq!(applied["version"], json!(2_u64), "이력이 쌓인다");
            let note = call(&registry, "get_thesis", json!({ "symbol": "005930" }))
                .await
                .expect("조회");
            assert_eq!(note["claims"][0]["claim_id"], json!("c2"));
            let statement = note["claims"][0]["statement"]["untrusted_text"]
                .as_str()
                .expect("주장");
            assert!(statement.contains("메모리 가격 회복") && !statement.contains("evil.example"));
            assert!(!statement.contains('\u{200b}'));
        });
    }

    fn tool_for(label: &str) -> &'static str {
        if label == "알 수 없는 인자" {
            "get_thesis"
        } else {
            "update_thesis"
        }
    }

    #[test]
    fn stale_versions_unknown_targets_and_forged_sources_are_refused() {
        runtime().block_on(async {
            let registry = registry();
            let change = add_claim("c1", &json!([{ "kind": "collected", "value": "news-1" }]));
            call(
                &registry,
                "update_thesis",
                json!({ "symbol": "005930", "base_version": 0_u64, "changes": [change.clone()] }),
            )
            .await
            .expect("첫 변경");
            for (args, label) in [
                (
                    json!({ "symbol": "005930", "base_version": 0_u64, "changes": [add_claim("c9", &json!([{ "kind": "collected", "value": "news-1" }]))] }),
                    "낡은 버전",
                ),
                (
                    json!({ "symbol": "000660", "base_version": 0_u64, "changes": [change.clone()] }),
                    "앱이 모르는 종목",
                ),
                (
                    json!({ "symbol": "005930", "base_version": 1_u64, "changes": [add_claim("c3", &json!([{ "kind": "collected", "value": "news-unknown" }]))] }),
                    "수집 목록 밖 핸들",
                ),
                (
                    json!({ "symbol": "005930", "base_version": 1_u64, "changes": [add_claim("c4", &json!([{ "kind": "news", "value": "https://www.hankyung.com/a" }]))] }),
                    "AI가 지정한 출처 종류",
                ),
                (
                    json!({ "symbol": "005930", "base_version": 1_u64, "changes": [add_claim("c5", &json!([]))] }),
                    "근거 없음",
                ),
                (
                    json!({ "symbol": "005930", "get_thesis": true }),
                    "알 수 없는 인자",
                ),
            ] {
                assert_eq!(
                    call(&registry, tool_for(label), args).await,
                    Err(ToolErrorCode::InvalidInput),
                    "{label}"
                );
            }
            let mut with_source = add_claim("c6", &json!([{ "kind": "web", "value": "https://x.example/a" }]));
            with_source
                .as_object_mut()
                .expect("객체")
                .insert(String::from("model"), json!("forged-model"));
            assert_eq!(
                call(
                    &registry,
                    "update_thesis",
                    json!({ "symbol": "005930", "base_version": 1_u64, "changes": [with_source] })
                )
                .await,
                Err(ToolErrorCode::InvalidInput),
                "모델 정보는 AI가 넣을 수 없다"
            );
        });
    }
}
