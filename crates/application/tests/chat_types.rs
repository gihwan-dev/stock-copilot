#[cfg(test)]
mod tests {
    use application::{
        AnalysisToolAllowance, ChatCardId, ChatFailureReason, ChatScreenContext, ChatSequence,
        ChatSessionId, ChatTurnId, ClassifiedError, CredentialSource, ErrorClass, IdentifierError,
        IsolationFailure, PromptError, ResponseMismatch, SystemFailure, UserAction,
        build_chat_prompt, check_analysis_tool,
    };
    use domain::{HorizonSelection, KnownMarket, Symbol, UntrustedText};

    #[test]
    fn chat_identifiers_validate_bounds_and_character_sets() {
        for bad in ["", "한글", "a b", "a/b", &"x".repeat(129)] {
            assert_eq!(
                ChatSessionId::try_new(bad),
                Err(IdentifierError::InvalidFormat),
                "잘못된 세션 ID: {bad}"
            );
            assert_eq!(
                ChatTurnId::try_new(bad),
                Err(IdentifierError::InvalidFormat),
                "잘못된 턴 ID: {bad}"
            );
            assert_eq!(
                ChatCardId::try_new(bad),
                Err(IdentifierError::InvalidFormat),
                "잘못된 카드 ID: {bad}"
            );
        }
        assert_eq!(
            ChatSessionId::try_new("chat_1.a-b").expect("세션").as_str(),
            "chat_1.a-b",
            "허용 문자"
        );
        assert_eq!(
            ChatCardId::try_new("signal:005930").expect("카드").as_str(),
            "signal:005930",
            "카드 ID 콜론 허용"
        );
        assert_eq!(ChatSequence::new(u64::MAX).next(), None, "순번 넘침은 None");
    }

    #[test]
    fn analysis_tool_allowlist_blocks_order_settings_and_web_tools() {
        for allowed in [
            "get_portfolio",
            "get_allocation",
            "get_quote",
            "get_profile",
            "submit_estimate",
            "request_web_research",
        ] {
            assert_eq!(
                check_analysis_tool(&UntrustedText::try_new(allowed).expect("텍스트")),
                AnalysisToolAllowance::Allowed,
                "허용 분석 도구: {allowed}"
            );
        }
        for forbidden in [
            "submit_order",
            "propose_order",
            "approve_order",
            "change_risk_settings",
            "update_settings",
            "WebSearch",
            "WebFetch",
            "Bash",
            "",
        ] {
            assert_eq!(
                check_analysis_tool(&UntrustedText::try_new(forbidden).expect("텍스트")),
                AnalysisToolAllowance::Forbidden,
                "금지 도구 차단: {forbidden}"
            );
        }
    }

    #[test]
    fn chat_prompt_embeds_screen_context_and_rejects_empty_text() {
        let context = ChatScreenContext::new(
            Some(Symbol::try_new("AAPL").expect("종목")),
            Some(KnownMarket::Us),
            Some(HorizonSelection::HalfYearOrMore),
        );
        let prompt = build_chat_prompt(
            &UntrustedText::try_new("실적 요약해 줘").expect("텍스트"),
            Some(&context),
        )
        .expect("프롬프트");
        assert!(
            prompt
                .as_str()
                .contains("[screen_context:symbol=AAPL,market=US,horizon=half_year_or_more]\n"),
            "화면 맥락 포함"
        );
        assert!(prompt.as_str().ends_with("실적 요약해 줘"), "원문 포함");
        assert_eq!(
            build_chat_prompt(&UntrustedText::try_new("   ").expect("공백"), None),
            Err(PromptError::Empty),
            "공백 메시지 거부"
        );
    }

    #[test]
    fn chat_failure_reasons_classify_into_closed_error_classes() {
        assert_eq!(
            ChatFailureReason::IsolationRejected(IsolationFailure::LoginRequired).class(),
            ErrorClass::UserAction(UserAction::CheckCredentials),
            "로그인 필요 분류"
        );
        assert_eq!(
            ChatFailureReason::InvalidCredential(CredentialSource::Dart).class(),
            ErrorClass::UserAction(UserAction::CheckCredentials),
            "자격 오류 분류"
        );
        assert_eq!(
            ChatFailureReason::StorageError(SystemFailure::PermissionDenied).class(),
            ErrorClass::UserAction(UserAction::CheckPermissions),
            "권한 오류 분류"
        );
        assert_eq!(
            ChatFailureReason::InvalidResponse(ResponseMismatch::Schema).class(),
            ErrorClass::Bug(application::BugDiagnostic::Response(
                ResponseMismatch::Schema
            )),
            "응답 스키마 오류는 Bug"
        );
    }
}
