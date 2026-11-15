#[cfg(test)]
mod tests {
    use application::{
        BrokerCapabilities, BrokerCapability, BugDiagnostic, CapabilitySupport, ErrorClass,
        HttpStatus, PortError, PortErrorKind, ReadEndpoint, classify_port_error,
    };
    fn compact(text: &str) -> String {
        text.chars()
            .filter(|ch| !ch.is_whitespace())
            .collect::<String>()
            .replace(",)", ")")
    }
    #[test]
    fn a21_contract_table_matches_market_trait_signatures() {
        let source = std::fs::read_to_string("src/lib.rs").expect("소스");
        let body = source
            .split_once("pub trait MarketInfo: Send + Sync {")
            .expect("포트")
            .1
            .split_once('}')
            .expect("끝")
            .0;
        let actual: Vec<String> = body
            .split(';')
            .map(compact)
            .filter(|value| !value.is_empty())
            .collect();
        let expected: Vec<String> = [
            "fn program_trades(&self, instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate) -> impl Future<Output = Result<Vec<ProgramTrade>, PortError>> + Send",
            "fn short_selling(&self, instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate) -> impl Future<Output = Result<Vec<ShortSelling>, PortError>> + Send",
            "fn credit_trades(&self, instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate) -> impl Future<Output = Result<Vec<CreditTrade>, PortError>> + Send",
            "fn securities_lending(&self, instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate) -> impl Future<Output = Result<Vec<SecuritiesLending>, PortError>> + Send",
            "fn investor_trading_rankings(&self, limit: RankingLimit) -> impl Future<Output = Result<InvestorTradingRankings, PortError>> + Send",
            "fn sectors(&self) -> impl Future<Output = Result<SectorCatalog, PortError>> + Send",
            "fn sector(&self, sector: &SectorId, market: KnownMarket) -> impl Future<Output = Result<SectorDetail, PortError>> + Send",
            "fn sector_rankings(&self, query: &SectorRankingQuery) -> impl Future<Output = Result<SectorRanking, PortError>> + Send",
            "fn sector_etfs(&self, sector: &SectorId, market: KnownMarket) -> impl Future<Output = Result<SectorEtfs, PortError>> + Send",
            "fn fx_rate(&self, pair: FxPair, kind: FxRateKind, at: Option<Timestamp>) -> impl Future<Output = Result<FxRate, PortError>> + Send",
            "fn order_fx_rates(&self) -> impl Future<Output = Result<OrderFxRates, PortError>> + Send",
            "fn indicator_quotes(&self, codes: &[IndicatorCode]) -> impl Future<Output = Result<Vec<MarketIndicatorQuote>, PortError>> + Send",
            "fn indicator_candles(&self, code: IndicatorCode, window: TimeInterval, period: CandlePeriod) -> impl Future<Output = Result<Vec<MarketIndicatorCandle>, PortError>> + Send",
            "fn rankings(&self, query: &RankingQuery) -> impl Future<Output = Result<StockRanking, PortError>> + Send",
            "fn stock_investor_flow(&self, instrument: &QuoteInstrument, from: CalendarDate, through: CalendarDate) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send",
            "fn indicator_investor_flow(&self, code: IndicatorCode, from: CalendarDate, through: CalendarDate, interval: FlowInterval) -> impl Future<Output = Result<Vec<InvestorFlow>, PortError>> + Send",
            "fn sector_composition(&self, sector: &SectorId, market: KnownMarket) -> impl Future<Output = Result<SectorComposition, PortError>> + Send",
            "fn calendar(&self, market: KnownMarket) -> impl Future<Output = Result<MarketCalendar, PortError>> + Send",
        ].into_iter().map(compact).collect();
        assert_eq!(actual, expected);
    }
    #[test]
    fn a21_capabilities_default_to_unsupported_and_preserve_error_classification() {
        let features = [
            (
                BrokerCapability::InvestorTradingRankings,
                ReadEndpoint::InvestorTradingRankings,
            ),
            (BrokerCapability::Sectors, ReadEndpoint::Sectors),
            (BrokerCapability::Sector, ReadEndpoint::Sector),
            (
                BrokerCapability::SectorRankings,
                ReadEndpoint::SectorRankings,
            ),
            (BrokerCapability::SectorEtfs, ReadEndpoint::SectorEtfs),
        ];
        let none = BrokerCapabilities::new(std::collections::BTreeSet::new());
        for (capability, endpoint) in features {
            assert_eq!(none.support_for(capability), CapabilitySupport::Unsupported);
            let only = BrokerCapabilities::new(std::iter::once(capability).collect());
            for (other, _) in features {
                assert_eq!(
                    only.support_for(other),
                    if other == capability {
                        CapabilitySupport::Supported
                    } else {
                        CapabilitySupport::Unsupported
                    }
                );
            }
            let error = PortError::new(
                endpoint,
                HttpStatus::BadRequest,
                PortErrorKind::Unsupported(capability),
            );
            assert_eq!(error.endpoint(), endpoint);
            assert_eq!(
                classify_port_error(error),
                ErrorClass::Bug(BugDiagnostic::UnsupportedCapability(capability))
            );
        }
    }
}
