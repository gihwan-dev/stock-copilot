use super::{MatchName, MatchNameClarity, MatchNameRole, NewsTracking, match_names};
use crate::holdings_link::InstrumentDirectory;
use application::TrackedInstrument;
use domain::{DisplayInstrumentName, InstrumentId, KnownMarket, Symbol};
use sources::TrackedTargets;
use std::future::{Future, ready};
use std::pin::Pin;
use std::sync::Arc;

fn tracked(symbol: &str, market: KnownMarket, name: &str) -> TrackedInstrument {
    TrackedInstrument {
        symbol: Symbol::try_new(symbol).expect("종목"),
        market,
        name: DisplayInstrumentName::try_new(name).expect("이름"),
    }
}

fn id(symbol: &str) -> InstrumentId {
    InstrumentId::try_new(symbol).expect("종목")
}

fn name(symbol: &str, text: &str, clarity: MatchNameClarity) -> MatchName {
    MatchName {
        instrument: id(symbol),
        name: text.to_owned(),
        role: MatchNameRole::Official,
        clarity,
    }
}

#[test]
fn tracked_names_and_codes_become_match_names_keyed_by_ticker() {
    let names = match_names(
        &[
            tracked("005930", KnownMarket::Kr, "삼성전자"),
            tracked("AAPL", KnownMarket::Us, "애플"),
            tracked("ON", KnownMarket::Us, "온세미"),
        ],
        &[],
        &InstrumentDirectory::default(),
    );
    assert_eq!(
        names,
        vec![
            name("005930", "삼성전자", MatchNameClarity::Distinct),
            name("005930", "005930", MatchNameClarity::Distinct),
            name("AAPL", "애플", MatchNameClarity::NeedsContext),
            name("AAPL", "AAPL", MatchNameClarity::Distinct),
            name("ON", "온세미", MatchNameClarity::Distinct),
        ],
        "두 글자 이름은 문맥이 필요하고 짧은 미국 티커는 쓰지 않는다"
    );
}

struct Fixed(Vec<TrackedInstrument>);

impl TrackedTargets for Fixed {
    fn tracked(&self) -> Pin<Box<dyn Future<Output = Vec<TrackedInstrument>> + Send + '_>> {
        Box::pin(ready(self.0.clone()))
    }
}

#[test]
fn the_tracking_slot_is_empty_until_the_app_binds_its_instruments() {
    let slot = NewsTracking::default();
    assert_eq!(
        tauri::async_runtime::block_on(slot.tracked()),
        Vec::new(),
        "묶기 전"
    );
    slot.bind(Arc::new(Fixed(vec![tracked(
        "005930",
        KnownMarket::Kr,
        "삼성전자",
    )])));
    slot.bind(Arc::new(Fixed(Vec::new())));
    assert_eq!(
        tauri::async_runtime::block_on(slot.tracked()).len(),
        1,
        "처음 묶은 것만 쓴다"
    );
}
