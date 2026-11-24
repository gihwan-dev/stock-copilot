#[cfg(test)]
mod tests {
    use domain::{
        Currency, DisclosureAction, DisclosureGrade, DisclosureRevision, HoldingStatus, Nanosecond,
        NumericInput, OffsetSeconds, Price, Timestamp, TriggerJudgement, UnixSeconds,
        UntrustedText, daily_move_trigger, disclosure_action, grade_disclosure, news_burst_trigger,
    };

    fn grade(title: &str, revision: DisclosureRevision) -> DisclosureGrade {
        grade_disclosure(&UntrustedText::try_new(title).expect("제목"), revision)
    }

    fn price(text: &str) -> Price {
        Price::parse(&NumericInput::try_new(text).expect("입력"), Currency::Krw).expect("가격")
    }

    fn at(seconds: i64) -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(seconds),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }

    #[test]
    fn titles_are_graded_by_the_r07_keyword_table() {
        use DisclosureGrade::{High, Low, Normal, Urgent};
        use DisclosureRevision::Original;
        for (title, expected) in [
            ("[유]매매거래정지(상장적격성 실질심사 사유 발생)", Urgent),
            ("[코]관리종목지정(감사의견 거절)", Urgent),
            ("[유]횡령ㆍ배임혐의발생", Urgent),
            ("[유]유상증자결정", High),
            ("[코]최대주주변경", High),
            ("[유]연결재무제표기준영업(잠정)실적(공정공시)", High),
            ("[유]영업실적등에대한전망(공정공시)", High),
            ("[유]단일판매ㆍ공급계약체결", High),
            ("[유]현금ㆍ현물배당결정", High),
            ("[유]분기보고서 (2026.09)", Normal),
            ("[유]기업설명회(IR)개최", Normal),
            ("[파]주식선물 가격제한폭 확대요건 도달", Low),
            ("[유]ETN 투자유의 안내", Low),
        ] {
            assert_eq!(grade(title, Original), expected, "{title}");
        }
        assert_eq!(grade("[유]잠정실적 공시", Original), High);
    }

    #[test]
    fn corrections_inherit_the_original_grade_one_step_lower() {
        use DisclosureRevision::{Correction, Original};
        assert_eq!(
            grade("[기재정정]매매거래정지", Correction),
            DisclosureGrade::High
        );
        assert_eq!(
            grade("[정정]유상증자결정", Correction),
            DisclosureGrade::Normal
        );
        assert_eq!(grade("[정정]분기보고서", Correction), DisclosureGrade::Low);
        assert_eq!(grade("[정정]주식선물", Correction), DisclosureGrade::Low);
        assert_eq!(grade("유상증자결정", Original), DisclosureGrade::High);
    }

    #[test]
    fn only_held_urgent_and_high_disclosures_wake_and_only_urgent_notifies() {
        use DisclosureGrade::{High, Low, Normal, Urgent};
        use HoldingStatus::{Held, NotHeld};
        assert_eq!(
            disclosure_action(Urgent, Held),
            DisclosureAction::WakeAndNotify
        );
        assert_eq!(disclosure_action(High, Held), DisclosureAction::Wake);
        assert_eq!(disclosure_action(Normal, Held), DisclosureAction::Briefing);
        assert_eq!(disclosure_action(Low, Held), DisclosureAction::CollectOnly);
        for grade in [Urgent, High, Normal, Low] {
            assert_eq!(
                disclosure_action(grade, NotHeld),
                DisclosureAction::CollectOnly
            );
        }
    }

    #[test]
    fn daily_move_triggers_at_five_percent_in_either_direction() {
        assert_eq!(
            daily_move_trigger(&price("10000"), &price("10500")),
            TriggerJudgement::Triggered
        );
        assert_eq!(
            daily_move_trigger(&price("10000"), &price("9500")),
            TriggerJudgement::Triggered
        );
        assert_eq!(
            daily_move_trigger(&price("10000"), &price("10499")),
            TriggerJudgement::Quiet
        );
        assert_eq!(
            daily_move_trigger(&price("10000"), &price("9501")),
            TriggerJudgement::Quiet
        );
        let usd =
            Price::parse(&NumericInput::try_new("10").expect("입력"), Currency::Usd).expect("가격");
        assert_eq!(
            daily_move_trigger(&price("10000"), &usd),
            TriggerJudgement::Undetermined
        );
    }

    #[test]
    fn five_news_items_within_one_hour_trigger() {
        let now = at(10_000);
        let inside = [at(6_401), at(7_000), at(8_000), at(9_000), at(10_000)];
        assert_eq!(
            news_burst_trigger(&inside, now),
            TriggerJudgement::Triggered
        );
        let edge = [at(6_400), at(7_000), at(8_000), at(9_000), at(10_000)];
        assert_eq!(news_burst_trigger(&edge, now), TriggerJudgement::Quiet);
        assert_eq!(
            news_burst_trigger(&inside[..4], now),
            TriggerJudgement::Quiet
        );
        let future = [at(6_500), at(7_000), at(8_000), at(9_000), at(10_001)];
        assert_eq!(news_burst_trigger(&future, now), TriggerJudgement::Quiet);
    }
}
