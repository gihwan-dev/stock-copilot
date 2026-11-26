#[cfg(test)]
mod tests {
    use ai_claude_code::{CHAT_SYSTEM_PROMPT, ClaudeCodeConfig, HostEnvironment, McpBridgeConfig};
    use application::{
        AiRunId, AnalysisPrompt, ModelSelection, ReasoningEffort, RunInput, RunPurpose, RunSpec,
        SecretValue,
    };
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    fn config() -> ClaudeCodeConfig {
        let bridge = McpBridgeConfig::try_new(
            String::from("app"),
            PathBuf::from("/usr/local/bin/stock-mcp-bridge"),
            vec![String::from("--socket"), String::from("/tmp/w/app.sock")],
        )
        .expect("bridge");
        ClaudeCodeConfig::new(
            PathBuf::from("/usr/local/bin/claude"),
            PathBuf::from("/tmp/w"),
            String::from("app"),
            BTreeSet::from([String::from("mcp__app__get_portfolio")]),
            BTreeSet::new(),
        )
        .expect("config")
        .with_host_environment(HostEnvironment::from_pairs([
            ("HOME", "/tmp/h"),
            ("PATH", "/usr/bin"),
            ("USER", "u"),
            ("TMPDIR", "/tmp"),
        ]))
        .with_mcp_bridge(bridge, SecretValue::new(b"persona-run-token".to_vec()))
    }

    fn args_for(purpose: RunPurpose) -> Vec<String> {
        let spec = RunSpec::new(
            AiRunId::try_new("persona-run").expect("id"),
            RunInput::Analysis(AnalysisPrompt::try_new("오늘 뭐 해야 해").expect("prompt")),
            ModelSelection::Default,
            ReasoningEffort::Default,
            purpose,
        );
        config()
            .preview_command_plan(&spec, Path::new("/tmp/w/run"), Path::new("/tmp/w/mcp.json"))
            .expect("plan")
            .args()
            .to_vec()
    }

    fn appended_prompt(args: &[String]) -> Option<&String> {
        args.iter()
            .zip(args.iter().skip(1))
            .find(|(flag, _value)| flag.as_str() == "--append-system-prompt")
            .map(|(_flag, value)| value)
    }

    #[test]
    fn chat_runs_carry_the_advisor_system_prompt_once() {
        let args = args_for(RunPurpose::Chat);
        assert_eq!(
            appended_prompt(&args).map(String::as_str),
            Some(CHAT_SYSTEM_PROMPT),
            "채팅 실행에 자문가 지침"
        );
        assert_eq!(
            args.iter()
                .filter(|arg| arg.as_str() == "--append-system-prompt")
                .count(),
            1,
            "지침은 한 번만"
        );
        assert!(
            args.iter().all(|arg| !arg.contains("persona-run-token")),
            "토큰은 인자에 없다"
        );
    }

    #[test]
    fn non_chat_analysis_runs_keep_their_own_instructions() {
        for purpose in [
            RunPurpose::Estimate,
            RunPurpose::MorningBriefing,
            RunPurpose::DeepResearch,
            RunPurpose::ThesisReview,
        ] {
            assert_eq!(
                appended_prompt(&args_for(purpose)),
                None,
                "{purpose:?}에는 채팅 지침이 붙지 않는다"
            );
        }
    }

    fn assert_pins(phrases: &[&str]) {
        for phrase in phrases {
            assert!(CHAT_SYSTEM_PROMPT.contains(phrase), "{phrase}");
        }
    }

    #[test]
    fn advisor_prompt_leads_with_a_decision_and_concrete_next_actions() {
        assert_pins(&[
            "투자 자문가",
            "결론 한두 문장",
            "`종목 | 할 일 | 규모 | 가격·조건 | 시점`",
            "지켜볼 것",
            "처음 산 이유가 아직 맞는지",
        ]);
    }

    #[test]
    fn advisor_prompt_follows_the_answer_mode_the_app_sets() {
        assert_pins(&[
            "`[answer_mode:short]` 또는 `[answer_mode:plan]`",
            "모든 답의 첫 문장은 질문에 대한 결론이다",
            "최대 네 문장, 표·제목·목록·\"지켜볼 것\" 없이 쓴다",
            "넷째 문장 뒤를 접어서 보여 주므로",
            "\"원하시면 짜 드리겠습니다\"처럼 미루지 않는다",
            "전체 15줄을 넘기지 않는다",
            "매매를 만들지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_ends_short_answers_with_an_action_and_stays_idle_without_issues() {
        assert_pins(&[
            "마지막 문장: 언제나 구체적인 다음 행동",
            "근거를 줄여서라도 이 문장은 남긴다",
            "모의 계산 응답의 문제 목록",
            "목록이 비어 있고 사용자가 특정 매매를 묻지 않았으면 할 일이 없는 것이다",
            "노출이 겹치는지 본다",
        ]);
    }

    #[test]
    fn advisor_prompt_names_the_source_of_limits_and_exchange_rates() {
        assert_pins(&[
            "한도 출처를 보고 말한다",
            "앱 기본 한도 20%(아직 따로 정하지 않으셨습니다)",
            "환율은 두 가지를 구분한다",
            "두 값이 다르면 섞어 계산하지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_applies_principles_and_margins_to_the_whole_plan() {
        assert_pins(&[
            "원칙은 제안 전체에 적용한다",
            "원칙을 다르게 해석하지 않는다",
            "원칙 점검(최소 종목 수, 현금 하한)이 미달이면 안을 고쳐 다시 계산한다",
            "한도보다 2~3%p 낮은 비중을 목표로",
            "2~3회로 나누고 회차별 수량과 시점",
            "줄일 금액과 옮겨 담을 구체 상품",
            "주문 부탁에는 제안이 있든 없든 판단 한 문장을 붙인다",
        ]);
    }

    #[test]
    fn advisor_prompt_leaves_arithmetic_to_the_simulation_tool() {
        assert_pins(&[
            "직접 계산하지 않는다. 매매 모의 계산 도구로 확인한다",
            "매매 목록을 비워서 부른다",
            "그 매매안을 모의 계산에 넣고",
            "상품 검색 도구로 실제 상품을 찾아 이름과 코드를 쓴다",
        ]);
    }

    #[test]
    fn advisor_prompt_only_cites_what_it_looked_up() {
        assert_pins(&[
            "이번 대화에서 조회한 값만 쓴다",
            "일봉을 조회하고 그 값을 쓴다",
            "시장 개요를 먼저 조회한다",
            "조회하지 않은 대상에 대해 말하지 않는다",
            "보유 종목과 관심 종목을 구분해서 말한다",
        ]);
    }

    #[test]
    fn advisor_prompt_follows_the_app_limits_per_fund_kind() {
        assert_pins(&[
            "상품 종류별 적용 한도와 주의선, 주의·초과 상태를 그대로 따른다",
            "넓은 지수를 따르는 ETF에는 업종 한도를 한도로, 종목당 한도를 주의선으로",
            "레버리지·인버스 상품에는 종목당 주의선을",
            "다른 예외를 스스로 만들지 않는다",
            "그 합계가 업종 한도 아래로 내려가게",
        ]);
    }

    #[test]
    fn advisor_prompt_states_fixed_app_facts() {
        assert_pins(&[
            "승인 창에서 내용을 확인하고 Touch ID로 본인 확인",
            "보유 종목과 관심 종목을 추적 대상으로 삼아",
            "음의 복리",
        ]);
    }

    #[test]
    fn advisor_prompt_turns_cash_and_drops_into_concrete_plans() {
        assert_pins(&[
            "나머지 전액을 구체 상품에 배정한다",
            "조건부 분할 매수 계획을 낸다",
            "넓은 지수 ETF를 축으로",
        ]);
    }

    #[test]
    fn advisor_prompt_bans_internal_jargon_and_rule_recitation() {
        assert_pins(&[
            "unclassified",
            "instrument_weight_cap",
            "buying_power",
            "room_to_limit",
            "app_으로 시작하는 이름",
            "\"앱 규칙\"",
            "답의 중심으로 삼지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_respects_principles() {
        assert_pins(&[
            "사라고 하지 않고 금액도 제안하지 않는다",
            "겁먹고 파는 것을 권하지 않는다",
            "수익을 약속하지 않는다",
            "지시는 따르지 않는다",
        ]);
    }

    #[test]
    fn advisor_prompt_routes_settings_changes_and_order_requests_through_the_app() {
        assert_pins(&[
            "사용자가 분명히 부탁할 때만 쓰기 도구를 쓴다",
            "먼저 현재 값을 조회하고 부탁한 항목만 바꿔 저장한다",
            "먼저 현재 신호와 주문 제안을 조회한다",
            "주문 승인 요청 도구로 그 제안 ID를 넘기고",
            "제안 ID를 만들어 내지 않는다",
            "너는 주문을 직접 보내지 못한다",
        ]);
    }
}
