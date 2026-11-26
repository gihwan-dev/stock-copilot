#[cfg(test)]
mod tests {
    use application::{
        HeadlineAdmission, HeadlineKey, RecentHeadlines, StorySimilarity, group_same_story,
    };
    use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds, UntrustedText};

    const WINDOW: i64 = 48 * 3_600;

    fn text(value: &str) -> UntrustedText {
        UntrustedText::try_new(value).expect("제목")
    }

    fn key(value: &str) -> HeadlineKey {
        HeadlineKey::of(&text(value))
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
    fn google_publisher_suffix_and_punctuation_do_not_change_the_key() {
        assert_eq!(
            key("삼성전자, 3분기 영업익 10조 돌파 - 매일경제"),
            key("삼성전자 3분기 영업익 10조 돌파"),
        );
        assert_ne!(
            key("삼성전자 3분기 영업익 10조 돌파"),
            key("SK하이닉스 3분기 영업익 10조 돌파"),
        );
        assert_eq!(key("Apple Beats Estimates").as_str(), "applebeatsestimates");
    }

    #[test]
    fn a_long_trailing_segment_is_kept_as_part_of_the_title() {
        let long = key("반도체 - 메모리 가격 반등과 함께 수출 회복세가 이어지는 중이다");
        assert!(long.as_str().contains("메모리"), "{}", long.as_str());
        assert!(long.as_str().starts_with("반도체"), "{}", long.as_str());
    }

    #[test]
    fn recent_headlines_rejects_the_same_story_within_two_days_only() {
        let mut recent = RecentHeadlines::new();
        let title = text("코스피 2,600 회복 - 연합뉴스");
        assert_eq!(recent.admit(&title, at(0)), HeadlineAdmission::New);
        assert_eq!(
            recent.admit(&text("코스피 2600 회복"), at(3_600)),
            HeadlineAdmission::Duplicate
        );
        assert_eq!(
            recent.admit(&title, at(WINDOW + 7_200)),
            HeadlineAdmission::New
        );
        assert_eq!(
            recent.admit(&text("---"), at(WINDOW + 7_300)),
            HeadlineAdmission::New
        );
        assert_eq!(
            recent.admit(&text("---"), at(WINDOW + 7_400)),
            HeadlineAdmission::New,
            "글자가 없는 제목은 비교하지 않는다"
        );
    }

    #[test]
    fn similar_titles_group_under_the_newest_representative() {
        let items = vec![
            ("삼성전자 3분기 영업이익 10조 돌파 전망", 100),
            ("삼성전자 3분기 영업이익 10조 돌파 전망 나와", 90),
            ("현대차 미국 판매 신기록", 80),
            ("삼성전자 3분기 영업이익 10조 돌파", 70),
        ];
        let groups = group_same_story(
            items
                .into_iter()
                .map(|(title, seconds)| (text(title), at(seconds)))
                .collect(),
            |item| &item.0,
            |item| item.1,
        );
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].related.len(), 2);
        assert_eq!(groups[1].related.len(), 0);
        assert_eq!(
            groups[0].representative.0.as_untrusted_str(),
            "삼성전자 3분기 영업이익 10조 돌파 전망"
        );
    }

    #[test]
    fn similar_titles_two_days_apart_stay_separate() {
        let groups = group_same_story(
            vec![
                (text("금리 동결"), at(WINDOW + 1)),
                (text("금리 동결"), at(0)),
            ],
            |item| &item.0,
            |item| item.1,
        );
        assert_eq!(groups.len(), 2);
    }

    #[test]
    fn different_companies_with_the_same_template_are_not_merged() {
        assert_eq!(
            key("LG에너지솔루션 목표가 상향").similarity(&key("포스코퓨처엠 목표가 상향")),
            StorySimilarity::Different
        );
        assert_eq!(key("").similarity(&key("")), StorySimilarity::Different);
    }
}
