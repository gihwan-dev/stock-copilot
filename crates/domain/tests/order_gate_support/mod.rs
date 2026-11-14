#[cfg(test)]
pub(crate) mod tests {
    use domain::{
        CommonShare, Currency, DisplayInstrumentName, InstrumentId, KnownMarket,
        KoreanMasterDetail, LiquidationTrading, ListingStatus, MasterCode, MasterInstrument,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NxtTrading, OffsetSeconds,
        ResolvedOrderInstrument, Symbol, Timestamp, TradingSuspension, UnixSeconds,
    };
    pub(crate) fn at() -> Timestamp {
        Timestamp::from_parts(
            UnixSeconds::new(100),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각")
    }
    pub(crate) fn instrument(market: KnownMarket, symbol: &str) -> ResolvedOrderInstrument {
        let (listing, currency, detail) = match market {
            KnownMarket::Kr => (
                "KOSPI",
                Currency::Krw,
                Some(KoreanMasterDetail {
                    liquidation: LiquidationTrading::Regular,
                    krx: TradingSuspension::Clear,
                    nxt: NxtTrading::Unsupported,
                }),
            ),
            KnownMarket::Us => ("NASDAQ", Currency::Usd, None),
        };
        MasterInstrument::try_new(MasterInstrumentRow {
            listing_market: MasterCode::try_new(listing).expect("시장"),
            symbol: Symbol::try_new(symbol).expect("종목"),
            instrument_id: InstrumentId::try_new(symbol).expect("키"),
            name: DisplayInstrumentName::try_new("테스트 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: detail,
            observed_at: at(),
        })
        .expect("마스터")
        .order_instrument()
        .expect("주문 종목")
    }
}
