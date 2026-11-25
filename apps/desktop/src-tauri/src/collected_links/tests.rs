use super::{LinkingTimes, TitleLinker};
use application::{
    AliasName, CollectedEntry, CollectedTimes, InformationKey, InformationLinks, NewsDataSettings,
    NewsDataSettingsChange, NewsDataSettingsRepository, PublicationTime, StockAlias,
    SystemPortError, WriteCompleted,
};
use domain::{InstrumentId, NewsItem, NewsSource, NormalizedUrl, Timestamp, UntrustedText};
use std::future::{Future, ready};
use std::sync::RwLock;

type Links = Vec<(InformationKey, Vec<InstrumentId>)>;

#[derive(Default)]
struct Store {
    settings: Option<NewsDataSettings>,
    times: RwLock<usize>,
    links: RwLock<Links>,
}

impl CollectedTimes for Store {
    fn record_times(
        &self,
        _entry: &CollectedEntry,
        _published_at: PublicationTime,
        _collected_at: Timestamp,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        if let Ok(mut times) = self.times.write() {
            *times = times.saturating_add(1);
        }
        ready(Ok(WriteCompleted))
    }
}

impl InformationLinks for Store {
    fn set_instruments(
        &self,
        item: &InformationKey,
        instruments: &[InstrumentId],
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        if let Ok(mut links) = self.links.write() {
            links.push((item.clone(), instruments.to_vec()));
        }
        ready(Ok(WriteCompleted))
    }
}

impl NewsDataSettingsRepository for Store {
    fn load(
        &self,
    ) -> impl Future<Output = Result<Option<NewsDataSettings>, SystemPortError>> + Send {
        ready(Ok(self.settings.clone()))
    }

    fn store(
        &self,
        _settings: &NewsDataSettings,
    ) -> impl Future<Output = Result<WriteCompleted, SystemPortError>> + Send {
        ready(Ok(WriteCompleted))
    }
}

struct ContainsAlias;

impl TitleLinker for ContainsAlias {
    fn instruments_in(&self, title: &UntrustedText, aliases: &[StockAlias]) -> Vec<InstrumentId> {
        aliases
            .iter()
            .filter(|alias| title.as_untrusted_str().contains(alias.alias.as_str()))
            .map(|alias| alias.instrument.clone())
            .collect()
    }
}

fn samsung() -> InstrumentId {
    InstrumentId::try_new("005930").expect("종목")
}

fn settings_with_alias() -> NewsDataSettings {
    let mut settings = NewsDataSettings::initial();
    settings
        .apply(NewsDataSettingsChange::AddAlias(StockAlias {
            instrument: samsung(),
            alias: AliasName::try_new("삼성전자").expect("약칭"),
        }))
        .expect("약칭 추가");
    settings
}

fn news(title: &str) -> CollectedEntry {
    CollectedEntry::News(NewsItem::new(
        NewsSource::Yonhap,
        NormalizedUrl::try_new("https://www.yna.co.kr/view/AKR1").expect("주소"),
        UntrustedText::try_new(title).expect("제목"),
        None,
    ))
}

fn at() -> Timestamp {
    Timestamp::from_parts(
        domain::UnixSeconds::new(1_790_000_000_i64),
        domain::Nanosecond::new(0_u32),
        domain::OffsetSeconds::new(0_i32),
    )
    .expect("시각")
}

fn record(store: &Store, linker: Option<&dyn TitleLinker>, title: &str) {
    let times = LinkingTimes::new(store, linker);
    let WriteCompleted = tauri::async_runtime::block_on(times.record_times(
        &news(title),
        PublicationTime::NotProvided,
        at(),
    ))
    .expect("기록");
}

#[test]
fn collected_titles_are_linked_to_instruments_through_the_alias_table() {
    let store = Store {
        settings: Some(settings_with_alias()),
        ..Store::default()
    };
    record(&store, Some(&ContainsAlias), "삼성전자 반도체 수출 증가");
    record(&store, Some(&ContainsAlias), "환율 하락");
    record(&store, None, "삼성전자 신제품");
    let links = store.links.read().expect("연결").clone();
    assert_eq!(
        links,
        vec![(
            InformationKey::News(
                NewsSource::Yonhap,
                NormalizedUrl::try_new("https://www.yna.co.kr/view/AKR1").expect("주소"),
            ),
            vec![samsung()],
        )],
        "매칭된 제목만, 연결기가 있을 때만"
    );
    assert_eq!(
        *store.times.read().expect("시각"),
        3_usize,
        "시각은 항상 남긴다"
    );
}

#[test]
fn nothing_is_linked_before_alias_settings_exist() {
    let store = Store::default();
    record(&store, Some(&ContainsAlias), "삼성전자 반도체");
    assert_eq!(store.links.read().expect("연결").len(), 0_usize);
}
