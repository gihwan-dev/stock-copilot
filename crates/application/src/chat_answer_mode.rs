use domain::UntrustedText;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatAnswerMode {
    Short,
    Plan,
}

impl ChatAnswerMode {
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::Short => "[answer_mode:short]",
            Self::Plan => "[answer_mode:plan]",
        }
    }
}

#[must_use]
pub fn chat_answer_mode(message: &UntrustedText) -> ChatAnswerMode {
    let compact: String = message
        .as_untrusted_str()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    if [
        "점검",
        "리밸런싱",
        "리밸런스",
        "어떻게넣",
        "투입",
        "배분",
        "처음인데",
        "시작해",
        "시작하",
        "오늘뭐",
        "뭐하면좋",
        "뭐해야",
        "포트폴리오어때",
        "계획",
        "전략",
        "짜줘",
        "짜주",
    ]
    .iter()
    .any(|marker| compact.contains(marker))
    {
        ChatAnswerMode::Plan
    } else {
        ChatAnswerMode::Short
    }
}

#[cfg(test)]
mod tests {
    use super::{ChatAnswerMode, chat_answer_mode};
    use domain::UntrustedText;

    fn mode(text: &str) -> ChatAnswerMode {
        chat_answer_mode(&UntrustedText::try_new(text).expect("글"))
    }

    #[test]
    fn planning_requests_get_the_plan_shape() {
        for text in [
            "포트폴리오 점검 좀 해줘",
            "리밸런싱 해줘. 국내 반 해외 반 정도로 맞추고 싶어",
            "현금이 너무 많은 것 같은데 어떻게 넣으면 좋을까?",
            "오늘 뭐 해야 해?",
            "처음인데 어떻게 시작해야 돼?",
            "내 포트폴리오 어때? 뭐 하면 좋을지 알려줘",
        ] {
            assert_eq!(mode(text), ChatAnswerMode::Plan, "{text}");
        }
    }

    #[test]
    fn single_questions_get_the_short_shape() {
        for text in [
            "뭐 살까?",
            "뭐 팔까?",
            "지금 내 계좌 괜찮아?",
            "이거 손절해야 돼?",
            "오늘 할 거 있어?",
            "ㅇㅇ 그럼 삼전 몇 주 팔면 됨?",
            "종목당 한도 15%로 낮춰줘",
            "카카오 30주 사줘",
        ] {
            assert_eq!(mode(text), ChatAnswerMode::Short, "{text}");
        }
    }

    #[test]
    fn tags_are_fixed_strings_the_prompt_refers_to() {
        assert_eq!(ChatAnswerMode::Short.as_tag(), "[answer_mode:short]");
        assert_eq!(ChatAnswerMode::Plan.as_tag(), "[answer_mode:plan]");
    }
}
