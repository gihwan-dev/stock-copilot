use super::{
    CollectedEntry, Database, DatabasePath, Fixture, InformationKey, InformationLinks,
    InformationStore, InstrumentId, NewsItem, NewsRepository, NewsSource, clock, key, news,
    runtime,
};
use application::{
    CandidateNewsRepository, CollectedItemReads, CollectedKind, CollectedTimes, PublicationTime,
};
use domain::Timestamp;

struct Reports {
    original: NewsItem,
    update: NewsItem,
    first: InstrumentId,
    confirmed: InstrumentId,
    old: Timestamp,
    later: Timestamp,
}

impl Reports {
    fn new() -> Self {
        let original = news(
            NewsSource::YahooFinance,
            "https://example.com/news",
            "보유 원문",
            Some("기존 본문"),
        );
        let update = news(
            NewsSource::YahooFinance,
            "https://example.com/news",
            "새 제목",
            None,
        );
        let first = InstrumentId::try_new("OTHER").expect("기존 종목");
        let confirmed = InstrumentId::try_new("US-master-117").expect("공식 종목");
        let old = crate::tests::timestamp();
        let later = old
            .checked_add(
                domain::TimeSpan::try_new(
                    domain::SpanSeconds::new(120),
                    domain::Nanosecond::new(0),
                )
                .expect("간격"),
            )
            .expect("이후");
        Self {
            original,
            update,
            first,
            confirmed,
            old,
            later,
        }
    }
    fn seed_before_restart(&self, path: &DatabasePath) {
        let database = Database::open(path, &key(), &clock()).expect("저장소");
        runtime().block_on(self.repair(&InformationStore::new(&database)));
    }
    async fn repair(&self, store: &InformationStore<'_>) {
        store.store(&self.original).await.expect("기존 원문만 저장");
        store
            .set_instruments(
                &InformationKey::News(
                    NewsSource::YahooFinance,
                    self.original.collected().url().clone(),
                ),
                std::slice::from_ref(&self.first),
            )
            .await
            .expect("기존 종목 연결");
        assert_eq!(
            store
                .collected_since(CollectedKind::News, self.old)
                .await
                .expect("관측 없는 원문")
                .len(),
            0
        );
        let repaired = store
            .retain_candidate_news(&self.update, &self.confirmed, self.old, self.later)
            .await
            .expect("시각과 연결 복구");
        assert_eq!(repaired.published_at, PublicationTime::Known(self.old));
        assert_eq!(repaired.collected_at, self.later);
        assert_eq!(repaired.entry, CollectedEntry::News(self.original.clone()));
        assert_eq!(
            repaired.instruments,
            vec![self.first.clone(), self.confirmed.clone()]
        );
        store
            .record_times(&repaired.entry, PublicationTime::NotProvided, self.old)
            .await
            .expect("발행일을 받지 못한 기존 데이터");
        let filled = store
            .retain_candidate_news(&self.update, &self.confirmed, self.old, self.later)
            .await
            .expect("누락된 발행일 복구");
        assert_eq!(filled, repaired);
    }
}
#[test]
fn candidate_news_repairs_a_partial_record_without_replacing_content_or_other_links_after_reopen() {
    let fixture = Fixture::new();
    let path = DatabasePath::new(fixture.path());
    let rt = runtime();
    let reports = Reports::new();
    reports.seed_before_restart(&path);
    let reopened = Database::open(&path, &key(), &clock()).expect("다시 열기");
    let store = InformationStore::new(&reopened);
    rt.block_on(async {
        let offline = store
            .collected_since(CollectedKind::News, reports.old)
            .await
            .expect("오프라인 원문");
        let cached = offline.first().expect("기사");
        assert_eq!(
            cached.instruments,
            vec![reports.first, reports.confirmed.clone()]
        );
        let repeated = store
            .retain_candidate_news(
                &reports.update,
                &reports.confirmed,
                reports.later,
                reports.later,
            )
            .await
            .expect("중복 요청");
        assert_eq!(&repeated, cached);
        assert_eq!(
            store
                .for_instrument(&reports.confirmed)
                .await
                .expect("종목 뉴스"),
            vec![reports.original]
        );
    });
}

#[test]
fn candidate_news_rolls_back_article_times_and_links_together_before_retry() {
    use application::{CollectedPresence, CollectedPresenceCheck};
    let fixture = Fixture::new();
    let rt = runtime();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("저장소");
    let store = InformationStore::new(&db);
    let headline = news(
        NewsSource::Yonhap,
        "https://example.com/issuer",
        "새 공장",
        None,
    );
    let instrument = InstrumentId::try_new("issuer-id").expect("공식 종목");
    let at = crate::tests::timestamp();
    rt.block_on(async {
        db.write(|tx| tx.execute_batch(crate::sql::information_tests::REJECT_CANDIDATE_LINK))
            .await
            .expect("실패 주입");
        let _failure = store
            .retain_candidate_news(&headline, &instrument, at, at)
            .await
            .expect_err("관측 저장 뒤 연결 실패");
        assert_eq!(
            store
                .presence(headline.collected().url())
                .await
                .expect("원문 없음"),
            CollectedPresence::New
        );
        assert_eq!(
            store
                .collected_since(CollectedKind::News, at)
                .await
                .expect("관측 없음")
                .len(),
            0
        );
        db.write(|tx| tx.execute_batch(crate::sql::information_tests::ACCEPT_CANDIDATE_LINK))
            .await
            .expect("실패 해소");
        let retry = store
            .retain_candidate_news(&headline, &instrument, at, at)
            .await
            .expect("재시도");
        assert_eq!(retry.instruments, vec![instrument]);
    });
}

#[test]
fn simultaneous_candidate_confirmation_preserves_both_issuers() {
    let fixture = Fixture::new();
    let db = Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("저장소");
    let store = InformationStore::new(&db);
    let reports = Reports::new();
    runtime().block_on(async {
        let retain = |issuer| {
            store.retain_candidate_news(&reports.original, issuer, reports.old, reports.later)
        };
        let (first, second) = tokio::join!(retain(&reports.first), retain(&reports.confirmed));
        first.expect("첫 연결");
        second.expect("두 번째 연결");
        let found = store
            .collected_since(CollectedKind::News, reports.old)
            .await
            .expect("연결");
        assert_eq!(found.len(), 1);
        assert_eq!(
            found.first().expect("기사").instruments,
            vec![reports.first, reports.confirmed]
        );
    });
}
