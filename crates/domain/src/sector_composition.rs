use crate::{
    KnownMarket, MarketDataError, MarketObservation, MasterInstrument, QuoteInstrument, SectorId,
    Symbol, Timestamp,
};
use std::collections::BTreeSet;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorCompositionScope {
    IncludingDescendants,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorComposition {
    sector: SectorId,
    market: KnownMarket,
    symbols: Vec<Symbol>,
    observation: MarketObservation,
    scope: SectorCompositionScope,
}
impl SectorComposition {
    pub fn from_stocks_response(
        sector: SectorId,
        market: KnownMarket,
        symbols: Vec<Symbol>,
        collected: Timestamp,
    ) -> Result<Self, MarketDataError> {
        let mut seen = BTreeSet::new();
        for symbol in &symbols {
            crate::market_ranking::validate_market_symbol(symbol, market)?;
            if !seen.insert(symbol.clone()) {
                return Err(MarketDataError::DuplicateInstrument);
            }
        }
        Ok(Self {
            sector,
            market,
            symbols,
            observation: MarketObservation::collected_only(collected),
            scope: SectorCompositionScope::IncludingDescendants,
        })
    }
    #[must_use]
    pub const fn sector(&self) -> &SectorId {
        &self.sector
    }
    #[must_use]
    pub const fn market(&self) -> KnownMarket {
        self.market
    }
    #[must_use]
    pub fn symbols(&self) -> &[Symbol] {
        &self.symbols
    }
    #[must_use]
    pub const fn observation(&self) -> MarketObservation {
        self.observation
    }
    #[must_use]
    pub const fn scope(&self) -> SectorCompositionScope {
        self.scope
    }
    pub fn identified_member(
        &self,
        master: &MasterInstrument,
    ) -> Result<QuoteInstrument, MarketDataError> {
        let identified = master
            .quote_instrument()
            .or(Err(MarketDataError::InvalidSymbol))?;
        if identified.market() != &self.market || !self.symbols.contains(identified.symbol()) {
            return Err(MarketDataError::InvalidSymbol);
        }
        Ok(identified)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        KnownMarket, Nanosecond, ObservationAlignment, OffsetSeconds, SectorComposition,
        SectorCompositionScope, SectorId, Symbol, Timestamp, UnixSeconds,
    };
    #[test]
    fn sector_stocks_preserve_direction_and_unknown_server_time() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let symbols = vec![
            Symbol::try_new("005930").expect("종목"),
            Symbol::try_new("000660").expect("종목"),
        ];
        let value = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            symbols.clone(),
            stamp,
        )
        .expect("구성");
        assert_eq!(value.sector().as_str(), "100");
        assert_eq!(value.market(), KnownMarket::Kr);
        assert_eq!(value.symbols(), symbols);
        assert_eq!(
            value.observation().alignment(),
            ObservationAlignment::Unknown
        );
        assert_eq!(value.scope(), SectorCompositionScope::IncludingDescendants);
        SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            vec![symbols.first().expect("종목").clone(); 2],
            stamp,
        )
        .expect_err("거부");
    }

    fn master(symbol: &str, listing: &str) -> crate::MasterInstrument {
        crate::MasterInstrument::try_new(crate::MasterInstrumentRow {
            listing_market: crate::MasterCode::try_new(listing).expect("상장 시장"),
            symbol: Symbol::try_new(symbol).expect("심볼"),
            instrument_id: crate::InstrumentId::try_new("stable").expect("ID"),
            name: crate::DisplayInstrumentName::try_new("종목").expect("이름"),
            security_type: crate::MasterSecurityType::from_code(
                crate::MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: crate::Currency::Krw,
            listing_status: crate::ListingStatus::Active,
            common_share: crate::CommonShare::Common,
            korean_detail: None,
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(0),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .expect("시각"),
        })
        .expect("마스터")
    }
    #[test]
    fn sector_member_is_identified_only_by_matching_master() {
        let stamp = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        let input = master("005930", "KOSPI");
        let composition = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            vec![input.row().symbol.clone()],
            stamp,
        )
        .expect("구성");
        assert_eq!(
            composition
                .identified_member(&input)
                .expect("일치")
                .instrument_id(),
            &input.row().instrument_id
        );
        composition
            .identified_member(&master("000660", "KOSDAQ"))
            .expect_err("미소속");
        composition
            .identified_member(&master("005930", "OTHER"))
            .expect_err("미지 시장");
        let foreign = SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Us,
            vec![Symbol::try_new("AAPL").expect("종목")],
            stamp,
        )
        .expect("구성");
        foreign.identified_member(&input).expect_err("시장 불일치");
        SectorComposition::from_stocks_response(
            SectorId::try_new("100").expect("섹터"),
            KnownMarket::Kr,
            vec![Symbol::try_new("AAPL").expect("종목")],
            stamp,
        )
        .expect_err("심볼 불일치");
    }
}
