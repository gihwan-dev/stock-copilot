use crate::{
    Database, DatabasePath, InformationStore,
    tests::{Fixture, clock, key, runtime},
};
use application::{
    CollectedEntry, DisclosureRepository, InformationKey, InformationLinks, InformationSearch,
    NewsRepository, SearchLimit,
};
use domain::{
    DartReceiptNo, Disclosure, InstrumentId, KindReceiptNo, NewsItem, NewsSource, NormalizedUrl,
    ReceiptNo, UntrustedText,
};

fn text(value: &str) -> UntrustedText {
    UntrustedText::try_new(value).expect("text")
}
fn news(source: NewsSource, url: &str, title: &str, body: Option<&str>) -> NewsItem {
    NewsItem::new(
        source,
        NormalizedUrl::try_new(url).expect("url"),
        text(title),
        body.map(text),
    )
}
fn disclosure(receipt: ReceiptNo, title: &str) -> Disclosure {
    Disclosure::new(
        receipt,
        NormalizedUrl::try_new("https://example.com/disclosure").expect("url"),
        text(title),
        Some(text("본문 원문")),
    )
}
#[test]
fn bigram_search_preserves_raw_text_and_rebuilds_identically() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let item = news(
        NewsSource::Yonhap,
        "https://example.com/news1",
        "삼성전자는 ABC-123!! 상승",
        Some("원문 \u{1112}\u{1161}\u{11ab}\u{1100}\u{1173}\u{11af}"),
    );
    rt.block_on(store.store(&item)).expect("save");
    let expected = vec![CollectedEntry::News(item)];
    let limit = SearchLimit::try_new(2).expect("limit");
    for query in [
        "삼성",
        "전자",
        "전",
        "ABC-1",
        "!!",
        "\u{1112}\u{1161}\u{11ab}",
    ] {
        assert_eq!(
            rt.block_on(store.search(&text(query), limit)),
            Ok(expected.clone()),
            "{query}"
        );
    }
    assert_eq!(rt.block_on(store.search(&text("한글"), limit)), Ok(vec![]));
    assert_eq!(rt.block_on(store.search(&text(""), limit)), Ok(vec![]));
    rt.block_on(database.write(|tx| {
        tx.execute(crate::sql::information_tests::STRAY_INDEX, [])?;
        Ok(())
    }))
    .expect("stray index fixture");
    assert_eq!(
        rt.block_on(store.search(&text("ghost"), limit))
            .expect_err("stray index is not an item")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    rt.block_on(store.rebuild_index()).expect("rebuild");
    assert_eq!(rt.block_on(store.search(&text("ghost"), limit)), Ok(vec![]));
    for query in [
        "삼성",
        "전자",
        "전",
        "ABC-1",
        "!!",
        "\u{1112}\u{1161}\u{11ab}",
    ] {
        assert_eq!(
            rt.block_on(store.search(&text(query), limit)),
            Ok(expected.clone()),
            "{query}"
        );
    }
}

#[test]
fn ascii_case_insensitive_search_preserves_original_and_survives_rebuild() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let item = news(
        NewsSource::YahooFinance,
        "https://example.com/nvidia",
        "NVIDIA 삼성전자가 ABC-123!! Ä",
        Some("GPU ghZ"),
    );
    rt.block_on(store.store(&item)).expect("save");
    let expected = vec![CollectedEntry::News(item)];
    let limit = SearchLimit::try_new(10).expect("limit");
    for rebuild in [false, true] {
        if rebuild {
            rt.block_on(store.rebuild_index()).expect("rebuild");
        }
        for query in [
            "N", "n", "NV", "nv", "NVIDIA", "nViDiA", "가 ab", "gpu", "GHZ", "Ä",
        ] {
            assert_eq!(
                rt.block_on(store.search(&text(query), limit)),
                Ok(expected.clone()),
                "{query}"
            );
        }
        assert_eq!(rt.block_on(store.search(&text("ä"), limit)), Ok(vec![]));
    }
}

#[test]
fn updating_an_item_removes_every_old_bigram_from_search() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let old = news(NewsSource::Yonhap, "https://example.com/item", "ABCD", None);
    let updated = news(NewsSource::Yonhap, "https://example.com/item", "WXYZ", None);
    let limit = SearchLimit::try_new(10).expect("limit");
    rt.block_on(store.store(&old)).expect("old");
    rt.block_on(store.store(&updated)).expect("updated");
    for rebuild in [false, true] {
        if rebuild {
            rt.block_on(store.rebuild_index()).expect("rebuild");
        }
        for query in ["AB", "ABC", "ABCD"] {
            assert_eq!(rt.block_on(store.search(&text(query), limit)), Ok(vec![]));
        }
        assert_eq!(
            rt.block_on(store.search(&text("WX"), limit)),
            Ok(vec![CollectedEntry::News(updated.clone())])
        );
    }
}

#[test]
fn news_identity_dedup_and_links_are_scoped_by_source() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let first = news(
        NewsSource::Yonhap,
        "https://example.com/news1",
        "삼성 첫 뉴스",
        None,
    );
    let second = news(
        NewsSource::Hankyung,
        "https://example.com/news1",
        "삼성 둘째 뉴스",
        None,
    );
    rt.block_on(store.store(&first)).expect("news");
    rt.block_on(store.store(&first)).expect("dedup");
    rt.block_on(store.store(&second)).expect("other source");
    let first_id = InstrumentId::try_new("instrument-a").expect("id");
    let second_id = InstrumentId::try_new("instrument-b").expect("id");
    let key = InformationKey::News(
        NewsSource::Yonhap,
        NormalizedUrl::try_new("https://example.com/news1").expect("url"),
    );
    rt.block_on(store.set_instruments(&key, &[first_id.clone(), first_id.clone()]))
        .expect("link");
    assert_eq!(
        rt.block_on(store.for_instrument(&first_id)),
        Ok(vec![first])
    );
    let updated = news(
        NewsSource::Yonhap,
        "https://example.com/news1",
        "삼성 갱신 뉴스",
        Some("새로운 본문"),
    );
    rt.block_on(store.store(&updated)).expect("update");
    assert_eq!(
        rt.block_on(store.search(&text("갱신"), SearchLimit::try_new(10).expect("limit"))),
        Ok(vec![CollectedEntry::News(updated.clone())])
    );
    assert_eq!(
        rt.block_on(store.for_instrument(&first_id)),
        Ok(vec![updated])
    );
    assert_eq!(
        rt.block_on(store.search(&text("첫 뉴스"), SearchLimit::try_new(10).expect("limit"))),
        Ok(vec![])
    );
    assert_eq!(rt.block_on(store.for_instrument(&second_id)), Ok(vec![]));
    assert_eq!(
        rt.block_on(store.search(&text("삼성"), SearchLimit::try_new(10).expect("limit")))
            .expect("search")
            .len(),
        2
    );
}

#[test]
fn disclosure_identity_links_and_search_limit_hold_across_categories() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    for (source, url) in [
        (NewsSource::Yonhap, "https://example.com/news1"),
        (NewsSource::Hankyung, "https://example.com/news1"),
    ] {
        rt.block_on(store.store(&news(source, url, "삼성 뉴스", None)))
            .expect("news");
    }
    let dart = disclosure(
        ReceiptNo::Dart(DartReceiptNo::try_new("20261118000001").expect("id")),
        "삼성 공시",
    );
    let kind = disclosure(
        ReceiptNo::Kind(KindReceiptNo::try_new("20261118000001").expect("id")),
        "삼성 공시",
    );
    rt.block_on(store.store_disclosure(&dart)).expect("dart");
    rt.block_on(store.store_disclosure(&kind)).expect("kind");
    let second_id = InstrumentId::try_new("instrument-b").expect("id");
    rt.block_on(store.set_instruments(
        &InformationKey::Disclosure(dart.key().clone()),
        std::slice::from_ref(&second_id),
    ))
    .expect("link");
    assert_eq!(rt.block_on(store.for_instrument(&second_id)), Ok(vec![]));
    assert_eq!(
        rt.block_on(store.disclosures_for_instrument(&second_id)),
        Ok(vec![dart.clone()])
    );
    assert_eq!(
        rt.block_on(store.disclosure(dart.key())),
        Ok(Some(dart.clone()))
    );
    assert_eq!(rt.block_on(store.disclosure(kind.key())), Ok(Some(kind)));
    assert_eq!(
        rt.block_on(store.search(&text("삼성"), SearchLimit::try_new(2).expect("limit")))
            .expect("search")
            .len(),
        2
    );
    assert_eq!(
        rt.block_on(store.search(&text("삼성"), SearchLimit::try_new(10).expect("limit")))
            .expect("search")
            .len(),
        4
    );
    assert_eq!(
        rt.block_on(store.search(&text("삼"), SearchLimit::try_new(2).expect("limit")))
            .expect("single")
            .len(),
        2
    );
}

#[test]
fn search_limit_rejects_values_outside_its_closed_range() {
    assert_eq!(*SearchLimit::try_new(1).expect("minimum").as_usize(), 1);
    assert_eq!(*SearchLimit::try_new(100).expect("maximum").as_usize(), 100);
    assert_eq!(
        SearchLimit::try_new(0),
        Err(application::SearchLimitError::OutOfRange)
    );
    assert_eq!(
        SearchLimit::try_new(101),
        Err(application::SearchLimitError::OutOfRange)
    );
}

#[test]
fn replacing_and_clearing_links_changes_only_the_selected_item() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let item = news(NewsSource::Yonhap, "https://example.com/link", "기사", None);
    let other = news(
        NewsSource::Yonhap,
        "https://example.com/other",
        "기타",
        None,
    );
    let first_id = InstrumentId::try_new("instrument-a").expect("id");
    let second_id = InstrumentId::try_new("instrument-b").expect("id");
    let first_key = InformationKey::News(NewsSource::Yonhap, item.collected().url().clone());
    let second_key = InformationKey::News(NewsSource::Yonhap, other.collected().url().clone());
    for article in [&item, &other] {
        rt.block_on(store.store(article)).expect("save");
    }
    for identity in [&first_key, &second_key] {
        rt.block_on(store.set_instruments(identity, std::slice::from_ref(&first_id)))
            .expect("link");
    }
    rt.block_on(store.set_instruments(&first_key, std::slice::from_ref(&second_id)))
        .expect("replace");
    assert_eq!(
        rt.block_on(store.for_instrument(&first_id)),
        Ok(vec![other])
    );
    assert_eq!(
        rt.block_on(store.for_instrument(&second_id)),
        Ok(vec![item])
    );
    rt.block_on(store.set_instruments(&first_key, &[]))
        .expect("clear");
    assert_eq!(rt.block_on(store.for_instrument(&second_id)), Ok(vec![]));
}

#[test]
fn search_index_failure_rolls_back_the_original_item() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let original = news(
        NewsSource::Yonhap,
        "https://example.com/news1",
        "기존 원문",
        None,
    );
    let changed = news(
        NewsSource::Yonhap,
        "https://example.com/news1",
        "바뀐 원문",
        None,
    );
    let id = InstrumentId::try_new("instrument-a").expect("id");
    let key = InformationKey::News(
        NewsSource::Yonhap,
        NormalizedUrl::try_new("https://example.com/news1").expect("url"),
    );
    rt.block_on(store.set_instruments(&key, std::slice::from_ref(&id)))
        .expect("link");
    rt.block_on(store.store(&original)).expect("save");
    rt.block_on(database.write(|tx| tx.execute_batch(crate::sql::information_tests::DROP_INDEX)))
        .expect("fault fixture");
    assert_eq!(
        rt.block_on(store.store(&changed))
            .expect_err("index failure")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(rt.block_on(store.for_instrument(&id)), Ok(vec![original]));
}

#[test]
fn corrupt_news_is_rejected_without_exposing_an_unrelated_instrument() {
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let bad = news(
        NewsSource::Yonhap,
        "https://example.com/bad",
        "첫 기사",
        None,
    );
    let good = news(
        NewsSource::Hankyung,
        "https://example.com/good",
        "둘째 기사",
        None,
    );
    let first = InstrumentId::try_new("instrument-a").expect("id");
    let second = InstrumentId::try_new("instrument-b").expect("id");
    let first_key = InformationKey::News(NewsSource::Yonhap, bad.collected().url().clone());
    let second_key = InformationKey::News(NewsSource::Hankyung, good.collected().url().clone());
    rt.block_on(store.store(&bad)).expect("bad save");
    rt.block_on(store.store(&good)).expect("good save");
    rt.block_on(store.set_instruments(&first_key, std::slice::from_ref(&first)))
        .expect("link");
    rt.block_on(store.set_instruments(&second_key, std::slice::from_ref(&second)))
        .expect("link");
    rt.block_on(database.write(|tx| {
        tx.execute(crate::sql::information_tests::CORRUPT_URL, [])?;
        Ok(())
    }))
    .expect("fixture");
    assert_eq!(
        rt.block_on(store.for_instrument(&first))
            .expect_err("invalid url")
            .failure(),
        application::SystemFailure::RepositoryIntegrity
    );
    assert_eq!(rt.block_on(store.for_instrument(&second)), Ok(vec![good]));
}

#[test]
fn every_information_sql_constant_prepares_on_migrated_database() {
    let mut connection = rusqlite::Connection::open_in_memory().expect("memory");
    crate::migration::run(&mut connection, crate::tests::timestamp()).expect("migration");
    for statement in [
        crate::sql::information::BY_KEY,
        crate::sql::information::BY_ID,
        crate::sql::information::ALL_ITEMS,
        crate::sql::information::INSERT_ITEM,
        crate::sql::information::UPDATE_ITEM,
        crate::sql::information::DELETE_LINKS,
        crate::sql::information::INSERT_LINK,
        crate::sql::information::BY_INSTRUMENT,
        crate::sql::information::DELETE_INDEX,
        crate::sql::information::DELETE_ALL_INDEX,
        crate::sql::information::INSERT_INDEX,
        crate::sql::information::SEARCH_INDEX,
    ] {
        connection.prepare(statement).expect("prepare");
    }
}

#[test]
fn populated_index_survives_database_reopen_and_integrity_check() {
    let fixture = Fixture::new();
    let rt = runtime();
    let path = DatabasePath::new(fixture.path());
    let item = news(
        NewsSource::Yonhap,
        "https://example.com/reopen",
        "삼성전자는 재개",
        Some("원본"),
    );
    {
        let database = Database::open(&path, &key(), &clock()).expect("open");
        let store = InformationStore::new(&database);
        rt.block_on(store.store(&item)).expect("save");
    };
    let database = Database::open(&path, &key(), &clock()).expect("reopen");
    let store = InformationStore::new(&database);
    assert_eq!(
        rt.block_on(store.search(&text("전자"), SearchLimit::try_new(10).expect("limit"))),
        Ok(vec![CollectedEntry::News(item)])
    );
}

#[test]
fn collected_url_catalog_knows_only_urls_the_app_collected() {
    use application::CollectedUrlCatalog;
    let fixture = Fixture::new();
    let rt = runtime();
    let database =
        Database::open(&DatabasePath::new(fixture.path()), &key(), &clock()).expect("open");
    let store = InformationStore::new(&database);
    let item = news(
        NewsSource::Yonhap,
        "https://www.yna.co.kr/view/AKR1",
        "기사",
        None,
    );
    rt.block_on(store.store(&item)).expect("save");
    let parse = |raw: &str| {
        domain::ParsedExternalUrl::parse(&domain::ExternalUrlInput::new(raw)).expect("주소")
    };
    let known = rt
        .block_on(store.find_collected(&parse("https://www.yna.co.kr/view/AKR1")))
        .expect("조회");
    assert_eq!(
        known,
        Some(NormalizedUrl::try_new("https://www.yna.co.kr/view/AKR1").expect("url"))
    );
    let unknown = rt
        .block_on(store.find_collected(&parse("https://evil.example/view/AKR1")))
        .expect("조회");
    assert_eq!(unknown, None);
}

mod estimate_citations;

mod candidate_news;
