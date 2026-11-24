#[cfg(test)]
mod tests {
    use domain::{InstrumentId, UntrustedText};
    use sources::{
        MatchDictionaryEntry, MatchNameAmbiguity, MatchNameKind, StockMatcher, TitleMatch,
    };

    fn id(raw: &str) -> InstrumentId {
        InstrumentId::try_new(raw).expect("종목")
    }

    fn entry(
        instrument: &str,
        name: &str,
        kind: MatchNameKind,
        ambiguity: MatchNameAmbiguity,
    ) -> MatchDictionaryEntry {
        MatchDictionaryEntry::new(id(instrument), name, kind, ambiguity).expect("사전 항목")
    }

    fn matcher() -> StockMatcher {
        use MatchNameAmbiguity::{CommonNoun, Distinct};
        use MatchNameKind::{Alias, English, Official};
        StockMatcher::new(vec![
            entry("KR7005930003", "삼성전자", Official, Distinct),
            entry("KR7000660001", "SK하이닉스", Official, Distinct),
            entry("KR7000660001", "하이닉스", Alias, Distinct),
            entry("KR7034730002", "SK", Official, Distinct),
            entry("KR7011070000", "LG이노텍", Official, Distinct),
            entry("KR7003550001", "LG", Official, Distinct),
            entry("KR7035420009", "NAVER", English, Distinct),
            entry("KR7SG00000001", "SG", Official, Distinct),
            entry("KR7012030008", "DB", Official, Distinct),
            entry("KR7001680005", "대상", Official, CommonNoun),
            entry("KR7CHAEBI0001", "채비", Official, CommonNoun),
            entry("KR7005190007", "동서", Official, CommonNoun),
            entry("KR7SAMYOUNG01", "삼영", Official, CommonNoun),
            entry("KR7RAY0000001", "레이", Official, CommonNoun),
            entry("KR7005380001", "현대자동차", Official, Distinct),
            entry("KR7005380001", "현대차", Alias, Distinct),
        ])
    }

    struct Case {
        title: &'static str,
        matched: &'static [&'static str],
        ambiguous: &'static [&'static str],
        groups: &'static [&'static str],
    }

    const CASES: &[Case] = &[
        Case {
            title: "ESG 경영 강화 나선다",
            matched: &[],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "ADB 아시아 성장률 전망 하향",
            matched: &[],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "NAVER, 웹툰 자회사 상장 추진",
            matched: &["KR7035420009"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "NAVER는 AI 투자 확대",
            matched: &["KR7035420009"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "NAVER웹툰 해외 진출",
            matched: &[],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "SG 주가 급등",
            matched: &["KR7SG00000001"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "삼성전자가 HBM 공급 확대",
            matched: &["KR7005930003"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "강삼영 교수 칼럼",
            matched: &[],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "동서발전 신규 발전소 착공",
            matched: &[],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "디스플레이 업황 회복",
            matched: &[],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "삼성전자&SK하이닉스 동반 강세",
            matched: &["KR7000660001", "KR7005930003"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "'하이닉스' 목표가 상향",
            matched: &["KR7000660001"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "현대차의 美 공장 가동",
            matched: &["KR7005380001"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "대상, 3분기 실적 개선",
            matched: &["KR7001680005"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "올해 지원 대상 확대",
            matched: &[],
            ambiguous: &["KR7001680005"],
            groups: &[],
        },
        Case {
            title: "채비 특징주 상한가",
            matched: &["KR7CHAEBI0001"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "전기차 충전 채비 서두르는 업계",
            matched: &[],
            ambiguous: &["KR7CHAEBI0001"],
            groups: &[],
        },
        Case {
            title: "SK에코플랜트 IPO 재추진",
            matched: &[],
            ambiguous: &[],
            groups: &["SK"],
        },
        Case {
            title: "LG이노텍 아이폰 카메라 모듈 공급",
            matched: &["KR7011070000"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "SK 지주사 배당 확대",
            matched: &["KR7034730002"],
            ambiguous: &[],
            groups: &[],
        },
        Case {
            title: "DB손해보험 실적 발표",
            matched: &[],
            ambiguous: &[],
            groups: &["DB"],
        },
    ];

    fn ids(raw: &[&str]) -> Vec<InstrumentId> {
        let mut out: Vec<InstrumentId> = raw.iter().map(|value| id(value)).collect();
        out.sort();
        out
    }

    fn run(title: &str) -> TitleMatch {
        matcher().match_title(&UntrustedText::try_new(title).expect("제목"))
    }

    #[test]
    fn every_fixture_title_is_judged_as_expected() {
        for case in CASES {
            let result = run(case.title);
            assert_eq!(
                result.matched(),
                ids(case.matched).as_slice(),
                "{}",
                case.title
            );
            assert_eq!(
                result.ambiguous(),
                ids(case.ambiguous).as_slice(),
                "{}",
                case.title
            );
            let groups: Vec<String> = case
                .groups
                .iter()
                .map(|group| (*group).to_owned())
                .collect();
            assert_eq!(result.group_mentions(), groups.as_slice(), "{}", case.title);
        }
    }

    #[test]
    fn only_titles_the_rules_cannot_settle_go_to_the_fast_model() {
        let handed_over: Vec<&str> = CASES
            .iter()
            .filter(|case| run(case.title).needs_fast_model())
            .map(|case| case.title)
            .collect();
        assert_eq!(
            handed_over,
            vec!["올해 지원 대상 확대", "전기차 충전 채비 서두르는 업계"]
        );
        assert_eq!(handed_over.len(), 2_usize);
        assert_eq!(CASES.len(), 21_usize);
    }

    #[test]
    fn dictionary_entries_reject_blank_and_oversized_names() {
        let instrument = id("KR7005930003");
        for bad in ["", "   ", &"가".repeat(65)] {
            assert_eq!(
                MatchDictionaryEntry::new(
                    instrument.clone(),
                    bad,
                    MatchNameKind::Alias,
                    MatchNameAmbiguity::Distinct
                ),
                None
            );
        }
        let trimmed = MatchDictionaryEntry::new(
            instrument,
            " 삼전 ",
            MatchNameKind::Alias,
            MatchNameAmbiguity::Distinct,
        )
        .expect("약칭");
        assert_eq!(trimmed.name(), "삼전");
        assert_eq!(trimmed.kind(), MatchNameKind::Alias);
    }
}
