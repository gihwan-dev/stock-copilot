use crate::{Currency, Money, RankingKind, SectorId, Symbol};
use rust_decimal::Decimal;
use std::collections::BTreeSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscoverySource {
    Ranking(RankingKind),
    Sector(SectorId),
    Supply,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum CapBand {
    Large,
    Mid,
    Small,
}

impl CapBand {
    #[must_use]
    pub fn of(market_cap: Option<Money>) -> Self {
        let Some(value) = market_cap.filter(|money| money.currency() == Currency::Krw) else {
            return Self::Small;
        };
        let amount = value.amount().as_decimal();
        if amount >= Decimal::from(10_000_000_000_000_i64) {
            Self::Large
        } else if amount >= Decimal::from(1_000_000_000_000_i64) {
            Self::Mid
        } else {
            Self::Small
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryInput {
    pub symbol: Symbol,
    pub source: DiscoverySource,
    pub market_cap: Option<Money>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveryCandidate {
    symbol: Symbol,
    sources: Vec<DiscoverySource>,
    band: CapBand,
}

impl DiscoveryCandidate {
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub fn sources(&self) -> &[DiscoverySource] {
        &self.sources
    }
    #[must_use]
    pub const fn band(&self) -> CapBand {
        self.band
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoverySlots(u8);

impl DiscoverySlots {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }
    #[must_use]
    pub const fn as_u8(&self) -> &u8 {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoveryQuota {
    pub large: DiscoverySlots,
    pub mid: DiscoverySlots,
    pub small: DiscoverySlots,
}

impl DiscoveryQuota {
    pub const DEFAULT: Self = Self {
        large: DiscoverySlots(2),
        mid: DiscoverySlots(3),
        small: DiscoverySlots(3),
    };
    const fn slots(self, band: CapBand) -> DiscoverySlots {
        match band {
            CapBand::Large => self.large,
            CapBand::Mid => self.mid,
            CapBand::Small => self.small,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiscoverySeed(u64);

impl DiscoverySeed {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }
}

fn merge(inputs: &[DiscoveryInput], skip: &BTreeSet<Symbol>) -> Vec<DiscoveryCandidate> {
    let mut merged: Vec<(DiscoveryCandidate, Option<Money>)> = Vec::new();
    for input in inputs {
        if skip.contains(&input.symbol) {
            continue;
        }
        match merged
            .iter_mut()
            .find(|(candidate, _cap)| candidate.symbol == input.symbol)
        {
            Some((candidate, cap)) => {
                if !candidate.sources.contains(&input.source) {
                    candidate.sources.push(input.source.clone());
                }
                if cap.is_none() {
                    *cap = input.market_cap;
                }
            }
            None => merged.push((
                DiscoveryCandidate {
                    symbol: input.symbol.clone(),
                    sources: vec![input.source.clone()],
                    band: CapBand::Small,
                },
                input.market_cap,
            )),
        }
    }
    merged
        .into_iter()
        .map(|(mut candidate, cap)| {
            candidate.band = CapBand::of(cap);
            candidate
        })
        .collect()
}

fn shuffle(items: &mut [DiscoveryCandidate], seed: DiscoverySeed) {
    items.sort_by_cached_key(|candidate| {
        let mut state = seed.0 ^ 0xCBF2_9CE4_8422_2325;
        for byte in candidate.symbol.as_str().bytes() {
            state ^= u64::from(byte);
            state = state.wrapping_mul(0x0100_0000_01B3);
        }
        state ^= state >> 33_u32;
        state = state.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
        state ^= state >> 33_u32;
        (state, candidate.symbol.clone())
    });
}

#[must_use]
pub fn select_discovery_candidates(
    inputs: &[DiscoveryInput],
    skip: &BTreeSet<Symbol>,
    quota: DiscoveryQuota,
    seed: DiscoverySeed,
) -> Vec<DiscoveryCandidate> {
    let mut merged = merge(inputs, skip);
    merged.sort_by_key(|candidate| std::cmp::Reverse(candidate.sources.len()));
    let mut chosen = Vec::new();
    for band in [CapBand::Large, CapBand::Mid, CapBand::Small] {
        let limit = usize::from(*quota.slots(band).as_u8());
        chosen.extend(
            merged
                .iter()
                .filter(|candidate| candidate.band == band)
                .take(limit)
                .cloned(),
        );
    }
    shuffle(&mut chosen, seed);
    chosen
}

#[cfg(test)]
mod tests {
    use super::{
        CapBand, DiscoveryInput, DiscoveryQuota, DiscoverySeed, DiscoverySlots, DiscoverySource,
        select_discovery_candidates,
    };
    use crate::{Currency, Money, NumericInput, RankingKind, SectorId, Symbol};
    use proptest::prelude::{ProptestConfig, any};
    use proptest::{prop_assert_eq, proptest};
    use std::collections::BTreeSet;

    fn krw(value: &str) -> Money {
        Money::parse(&NumericInput::try_new(value).expect("입력"), Currency::Krw).expect("금액")
    }
    fn symbol(value: &str) -> Symbol {
        Symbol::try_new(value).expect("심볼")
    }
    fn input(name: &str, source: DiscoverySource, cap: Option<Money>) -> DiscoveryInput {
        DiscoveryInput {
            symbol: symbol(name),
            source,
            market_cap: cap,
        }
    }
    fn ranking() -> DiscoverySource {
        DiscoverySource::Ranking(RankingKind::from_code(
            &crate::MasterCode::try_new("TRADING_VALUE").expect("코드"),
        ))
    }
    fn sector() -> DiscoverySource {
        DiscoverySource::Sector(SectorId::try_new("semis").expect("섹터"))
    }
    fn quota(large: u8, mid: u8, small: u8) -> DiscoveryQuota {
        DiscoveryQuota {
            large: DiscoverySlots::new(large),
            mid: DiscoverySlots::new(mid),
            small: DiscoverySlots::new(small),
        }
    }

    #[test]
    fn cap_bands_use_krw_thresholds_and_unknown_is_small() {
        assert_eq!(CapBand::of(Some(krw("10000000000000"))), CapBand::Large);
        assert_eq!(CapBand::of(Some(krw("9999999999999"))), CapBand::Mid);
        assert_eq!(CapBand::of(Some(krw("1000000000000"))), CapBand::Mid);
        assert_eq!(CapBand::of(Some(krw("999999999999"))), CapBand::Small);
        assert_eq!(CapBand::of(None), CapBand::Small);
        let usd = Money::parse(
            &NumericInput::try_new("99999999999999").expect("입력"),
            Currency::Usd,
        )
        .ok();
        assert_eq!(CapBand::of(usd), CapBand::Small);
    }

    #[test]
    fn duplicates_merge_sources_known_symbols_are_skipped_and_bands_are_capped() {
        let inputs = vec![
            input("BIG1", ranking(), Some(krw("20000000000000"))),
            input("BIG2", ranking(), Some(krw("30000000000000"))),
            input("BIG1", sector(), None),
            input("BIG3", DiscoverySource::Supply, Some(krw("40000000000000"))),
            input("MID1", sector(), Some(krw("2000000000000"))),
            input("HELD", ranking(), Some(krw("2000000000000"))),
            input("TINY", DiscoverySource::Supply, None),
            input("TINY", DiscoverySource::Supply, None),
        ];
        let skip = BTreeSet::from([symbol("HELD")]);
        let chosen =
            select_discovery_candidates(&inputs, &skip, quota(2, 1, 1), DiscoverySeed::new(7));
        let mut names: Vec<_> = chosen
            .iter()
            .map(|candidate| candidate.symbol().clone())
            .collect();
        names.sort();
        assert_eq!(
            names,
            vec![
                symbol("BIG1"),
                symbol("BIG2"),
                symbol("MID1"),
                symbol("TINY")
            ]
        );
        let big = chosen
            .iter()
            .find(|candidate| candidate.symbol() == &symbol("BIG1"))
            .expect("BIG1");
        assert_eq!(big.sources(), &[ranking(), sector()]);
        assert_eq!(big.band(), CapBand::Large);
        let tiny = chosen
            .iter()
            .find(|candidate| candidate.symbol() == &symbol("TINY"))
            .expect("TINY");
        assert_eq!(tiny.sources(), &[DiscoverySource::Supply]);
        assert_eq!(
            select_discovery_candidates(&inputs, &skip, quota(0, 0, 0), DiscoverySeed::new(1))
                .len(),
            0
        );
        assert_eq!(*DiscoveryQuota::DEFAULT.large.as_u8(), 2);
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(testkit::property_cases()))]
        #[test]
        fn shuffling_is_a_deterministic_permutation(seed in any::<u64>(), count in 0_u8..12) {
            let inputs: Vec<_> = (0..count)
                .map(|index| input(&format!("S{index}"), DiscoverySource::Supply, None))
                .collect();
            let first = select_discovery_candidates(&inputs, &BTreeSet::new(), quota(0, 0, 12), DiscoverySeed::new(seed));
            let again = select_discovery_candidates(&inputs, &BTreeSet::new(), quota(0, 0, 12), DiscoverySeed::new(seed));
            prop_assert_eq!(&first, &again);
            let mut names: Vec<_> = first.iter().map(|candidate| candidate.symbol().clone()).collect();
            names.sort();
            let mut expected: Vec<_> = inputs.iter().map(|item| item.symbol.clone()).collect();
            expected.sort();
            prop_assert_eq!(names, expected);
        }
    }
}
