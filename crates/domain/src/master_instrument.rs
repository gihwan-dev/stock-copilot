use crate::{
    AllocationInstrument, Currency, DisplayInstrumentName, ExcludedSecurityType, InstrumentId,
    InstrumentNameFact, Market, MarketCode, PolicySecurityFacts, ResolvedOrderInstrument, Symbol,
    Timestamp,
};
use std::{collections::BTreeMap, fmt};

#[derive(Clone, PartialEq, Eq)]
pub struct MasterCode(String);
impl MasterCode {
    pub fn try_new(value: &str) -> Result<Self, MasterInstrumentError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(MasterInstrumentError::InvalidCode);
        }
        Ok(Self(value.to_owned()))
    }
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for MasterCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterCode").finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityTypeKnowledge {
    Known(ExcludedSecurityType),
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterSecurityType {
    code: MasterCode,
    known: SecurityTypeKnowledge,
}
impl MasterSecurityType {
    #[must_use]
    pub fn from_code(code: MasterCode) -> Self {
        let known = match code.as_str() {
            "STOCK" => SecurityTypeKnowledge::Known(ExcludedSecurityType::Stock),
            "FOREIGN_STOCK" => SecurityTypeKnowledge::Known(ExcludedSecurityType::ForeignStock),
            "DEPOSITARY_RECEIPT" => {
                SecurityTypeKnowledge::Known(ExcludedSecurityType::DepositaryReceipt)
            }
            "INFRASTRUCTURE_FUND" => {
                SecurityTypeKnowledge::Known(ExcludedSecurityType::InfrastructureFund)
            }
            "REIT" => SecurityTypeKnowledge::Known(ExcludedSecurityType::Reit),
            "ETF" => SecurityTypeKnowledge::Known(ExcludedSecurityType::Etf),
            "FOREIGN_ETF" => SecurityTypeKnowledge::Known(ExcludedSecurityType::ForeignEtf),
            "ETN" => SecurityTypeKnowledge::Known(ExcludedSecurityType::Etn),
            "STOCK_WARRANTS" => SecurityTypeKnowledge::Known(ExcludedSecurityType::StockWarrants),
            _ => SecurityTypeKnowledge::Unknown,
        };
        Self { code, known }
    }
    #[must_use]
    pub const fn code(&self) -> &MasterCode {
        &self.code
    }
    #[must_use]
    pub const fn known(&self) -> Option<ExcludedSecurityType> {
        match self.known {
            SecurityTypeKnowledge::Known(kind) => Some(kind),
            SecurityTypeKnowledge::Unknown => None,
        }
    }
    #[must_use]
    pub const fn knowledge(&self) -> SecurityTypeKnowledge {
        self.known
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListingStatus {
    Scheduled,
    Active,
    Delisted,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommonShare {
    Common,
    Other,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidationTrading {
    Regular,
    Liquidation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TradingSuspension {
    Clear,
    Suspended,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NxtTrading {
    Unsupported,
    Supported(TradingSuspension),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KoreanMasterDetail {
    pub liquidation: LiquidationTrading,
    pub krx: TradingSuspension,
    pub nxt: NxtTrading,
}
#[derive(Clone, PartialEq, Eq)]
pub struct MasterInstrumentRow {
    pub listing_market: MasterCode,
    pub symbol: Symbol,
    pub instrument_id: InstrumentId,
    pub name: DisplayInstrumentName,
    pub security_type: MasterSecurityType,
    pub currency: Currency,
    pub listing_status: ListingStatus,
    pub common_share: CommonShare,
    pub korean_detail: Option<KoreanMasterDetail>,
    pub observed_at: Timestamp,
}
impl fmt::Debug for MasterInstrumentRow {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterInstrumentRow")
            .finish_non_exhaustive()
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MasterInstrumentError {
    InvalidCode,
    CurrencyMismatch,
    SymbolMismatch,
    UnexpectedKoreanDetail,
    UnknownMarket,
    UnknownSecurityType,
    DuplicateIdentity,
    InvalidPriceObservation,
    InvalidNxtStatus,
    NotListed,
    TradingSuspended,
    LiquidationBuy,
    MissingTradingStatus,
}
#[derive(Clone, PartialEq, Eq)]
pub struct MasterInstrument {
    row: MasterInstrumentRow,
    market: Market,
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuoteInstrument {
    market: crate::KnownMarket,
    currency: Currency,
    symbol: Symbol,
    instrument_id: InstrumentId,
}
impl fmt::Debug for QuoteInstrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuoteInstrument").finish_non_exhaustive()
    }
}
impl QuoteInstrument {
    #[must_use]
    pub const fn market(&self) -> &crate::KnownMarket {
        &self.market
    }
    #[must_use]
    pub const fn currency(&self) -> Currency {
        self.currency
    }
    #[must_use]
    pub const fn symbol(&self) -> &Symbol {
        &self.symbol
    }
    #[must_use]
    pub const fn instrument_id(&self) -> &InstrumentId {
        &self.instrument_id
    }
}
impl fmt::Debug for MasterInstrument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MasterInstrument").finish_non_exhaustive()
    }
}
impl MasterInstrument {
    pub fn try_new(row: MasterInstrumentRow) -> Result<Self, MasterInstrumentError> {
        let market = match row.listing_market.as_str() {
            "KR" | "US" => return Err(MasterInstrumentError::InvalidCode),
            "KOSPI" | "KOSDAQ" | "KR_ETC" => Market::Kr,
            "NYSE" | "NASDAQ" | "AMEX" | "US_ETC" => Market::Us,
            _ => Market::new(
                MarketCode::try_new(row.listing_market.as_str())
                    .or(Err(MasterInstrumentError::InvalidCode))?,
            ),
        };
        if row
            .symbol
            .as_str()
            .bytes()
            .any(|byte| byte.is_ascii_lowercase())
        {
            return Err(MasterInstrumentError::SymbolMismatch);
        }
        match market {
            Market::Kr => {
                if row.currency != Currency::Krw {
                    return Err(MasterInstrumentError::CurrencyMismatch);
                }
                if row.symbol.as_str().len() != 6
                    || !row
                        .symbol
                        .as_str()
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric())
                {
                    return Err(MasterInstrumentError::SymbolMismatch);
                }
            }
            Market::Us => {
                if row.currency != Currency::Usd {
                    return Err(MasterInstrumentError::CurrencyMismatch);
                }
                if !row
                    .symbol
                    .as_str()
                    .bytes()
                    .next()
                    .is_some_and(|byte| byte.is_ascii_alphabetic())
                {
                    return Err(MasterInstrumentError::SymbolMismatch);
                }
                if row.korean_detail.is_some() {
                    return Err(MasterInstrumentError::UnexpectedKoreanDetail);
                }
            }
            Market::Unknown(_) => {
                if row.korean_detail.is_some() {
                    return Err(MasterInstrumentError::UnexpectedKoreanDetail);
                }
            }
        }
        Ok(Self { row, market })
    }
    #[must_use]
    pub const fn row(&self) -> &MasterInstrumentRow {
        &self.row
    }
    pub fn quote_instrument(&self) -> Result<QuoteInstrument, MasterInstrumentError> {
        let market = crate::KnownMarket::try_from_market(&self.market)
            .or(Err(MasterInstrumentError::UnknownMarket))?;
        Ok(QuoteInstrument {
            market,
            currency: self.row.currency,
            symbol: self.row.symbol.clone(),
            instrument_id: self.row.instrument_id.clone(),
        })
    }
    fn resolved_instrument(&self) -> Result<ResolvedOrderInstrument, MasterInstrumentError> {
        ResolvedOrderInstrument::from_master_lookup_result(
            self.row.symbol.clone(),
            self.market.clone(),
        )
        .or(Err(MasterInstrumentError::UnknownMarket))
    }
    pub fn order_gate_facts(&self) -> Result<crate::MasterOrderGateFacts, MasterInstrumentError> {
        Ok(crate::master_order_gate::from_master(
            self.resolved_instrument()?,
            self.row.clone(),
        ))
    }
    pub fn order_instrument(&self) -> Result<ResolvedOrderInstrument, MasterInstrumentError> {
        self.order_gate_facts()?
            .instrument_for(crate::OrderSide::Buy)
    }
    pub fn price_observation(
        &self,
        price: crate::Price,
        observed_at: Timestamp,
    ) -> Result<crate::PriceObservation, MasterInstrumentError> {
        self.price_observation_for(crate::OrderSide::Buy, price, observed_at)
    }
    pub fn price_observation_for(
        &self,
        side: crate::OrderSide,
        price: crate::Price,
        observed_at: Timestamp,
    ) -> Result<crate::PriceObservation, MasterInstrumentError> {
        if price.as_money().currency() != self.row.currency {
            return Err(MasterInstrumentError::CurrencyMismatch);
        }
        crate::PriceObservation::from_identified_quote_parts(
            self.row.instrument_id.clone(),
            self.order_gate_facts()?.instrument_for(side)?,
            price,
            observed_at,
        )
        .or(Err(MasterInstrumentError::InvalidPriceObservation))
    }
    pub fn name_fact(&self) -> Result<InstrumentNameFact, MasterInstrumentError> {
        self.name_fact_for(crate::OrderSide::Buy)
    }
    pub fn name_fact_for(
        &self,
        side: crate::OrderSide,
    ) -> Result<InstrumentNameFact, MasterInstrumentError> {
        Ok(InstrumentNameFact::from_master_lookup_result(
            self.order_gate_facts()?.instrument_for(side)?,
            self.row.name.clone(),
        ))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterInstrumentFacts(Vec<MasterInstrument>);
impl MasterInstrumentFacts {
    pub fn try_new(instruments: Vec<MasterInstrument>) -> Result<Self, MasterInstrumentError> {
        let mut ids = std::collections::BTreeSet::new();
        let mut symbols = std::collections::BTreeSet::new();
        for instrument in &instruments {
            if !ids.insert(instrument.row.instrument_id.clone())
                || !symbols.insert((instrument.market.clone(), instrument.row.symbol.clone()))
            {
                return Err(MasterInstrumentError::DuplicateIdentity);
            }
        }
        Ok(Self(instruments))
    }
    #[must_use]
    pub fn instruments(&self) -> &[MasterInstrument] {
        &self.0
    }
    #[must_use]
    pub fn policy_security_facts(&self) -> PolicySecurityFacts {
        let mut values = BTreeMap::new();
        for master in &self.0 {
            if let (Some(kind), Ok(instrument)) = (
                master.row.security_type.known(),
                master.resolved_instrument(),
            ) {
                values.insert(AllocationInstrument::new(&instrument), kind);
            }
        }
        PolicySecurityFacts::from_master_lookup_results(values)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        AllocationInstrument, CommonShare, Currency, DisplayInstrumentName, ExcludedSecurityType,
        InstrumentId, KnownMarket, KoreanMasterDetail, LiquidationTrading, ListingStatus, Market,
        MasterCode, MasterInstrument, MasterInstrumentError, MasterInstrumentFacts,
        MasterInstrumentRow, MasterSecurityType, Nanosecond, NumericInput, NxtTrading,
        OffsetSeconds, PolicySecurityFacts, Price, SecurityTypeKnowledge, Symbol, Timestamp,
        TradingSuspension, UnixSeconds, UnknownKnownMarket,
    };
    use proptest::{prop_assert_eq, proptest, sample, test_runner::Config};

    fn row() -> MasterInstrumentRow {
        MasterInstrumentRow {
            listing_market: MasterCode::try_new("KOSPI").expect("상장 시장"),
            symbol: Symbol::try_new("0101N0").expect("심볼"),
            instrument_id: InstrumentId::try_new("stable-id").expect("내부 키"),
            name: DisplayInstrumentName::try_new("가상 종목").expect("이름"),
            security_type: MasterSecurityType::from_code(
                MasterCode::try_new("STOCK").expect("유형"),
            ),
            currency: Currency::Krw,
            listing_status: ListingStatus::Active,
            common_share: CommonShare::Common,
            korean_detail: Some(KoreanMasterDetail {
                liquidation: LiquidationTrading::Regular,
                krx: TradingSuspension::Clear,
                nxt: NxtTrading::Unsupported,
            }),
            observed_at: Timestamp::from_parts(
                UnixSeconds::new(0),
                Nanosecond::new(0),
                OffsetSeconds::new(32400),
            )
            .expect("시각"),
        }
    }

    #[test]
    fn master_fields_and_derived_identity_are_preserved() {
        let input = row();
        let master = MasterInstrument::try_new(input.clone()).expect("마스터");
        assert_eq!(master.row(), &input);
        assert_eq!(format!("{:?}", input.listing_market), "MasterCode { .. }");
        assert_eq!(format!("{input:?}"), "MasterInstrumentRow { .. }");
        let query = master.quote_instrument().expect("시세 식별");
        assert_eq!(query.instrument_id(), &input.instrument_id);
        assert_eq!(query.symbol(), &input.symbol);
        assert_eq!(query.market(), &KnownMarket::Kr);
        assert_eq!(Market::from(KnownMarket::Kr), Market::Kr);
        assert_eq!(Market::from(KnownMarket::Us), Market::Us);
        let order = master.order_instrument().expect("주문 식별");
        assert_eq!(order.symbol(), &input.symbol);
        assert_eq!(order.market(), Market::Kr);
        let name = master.name_fact().expect("이름 사실");
        assert_eq!(name.into_parts(), (order.clone(), input.name));
        let facts = MasterInstrumentFacts::try_new(vec![master.clone()]).expect("목록");
        assert_eq!(facts.instruments(), std::slice::from_ref(&master));
        assert_eq!(
            facts
                .policy_security_facts()
                .security_type(&AllocationInstrument::new(&order)),
            Some(ExcludedSecurityType::Stock)
        );
        assert_eq!(format!("{master:?}"), "MasterInstrument { .. }");
        assert_eq!(format!("{query:?}"), "QuoteInstrument { .. }");
    }

    #[test]
    fn inconsistent_market_currency_symbol_and_details_are_rejected() {
        for (market, symbol, currency, expected) in [
            (
                "KOSPI",
                "005930",
                Currency::Usd,
                MasterInstrumentError::CurrencyMismatch,
            ),
            (
                "NYSE",
                "AAPL",
                Currency::Krw,
                MasterInstrumentError::CurrencyMismatch,
            ),
            (
                "KOSDAQ",
                "AAPL",
                Currency::Krw,
                MasterInstrumentError::SymbolMismatch,
            ),
            (
                "KR_ETC",
                "00.930",
                Currency::Krw,
                MasterInstrumentError::SymbolMismatch,
            ),
            (
                "NASDAQ",
                "005930",
                Currency::Usd,
                MasterInstrumentError::SymbolMismatch,
            ),
        ] {
            let mut input = row();
            input.listing_market = MasterCode::try_new(market).expect("시장");
            input.symbol = Symbol::try_new(symbol).expect("심볼");
            input.currency = currency;
            assert_eq!(MasterInstrument::try_new(input), Err(expected));
        }
        let mut input = row();
        input.listing_market = MasterCode::try_new("AMEX").expect("시장");
        input.symbol = Symbol::try_new("BRK.B").expect("심볼");
        input.currency = Currency::Usd;
        input.korean_detail = Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        });
        assert_eq!(
            MasterInstrument::try_new(input),
            Err(MasterInstrumentError::UnexpectedKoreanDetail)
        );
    }

    #[test]
    fn master_codes_are_bounded_and_do_not_echo_errors() {
        for code in ["", "a b", "한글", "X\n", &"X".repeat(65)] {
            assert_eq!(
                MasterCode::try_new(code),
                Err(MasterInstrumentError::InvalidCode)
            );
        }
        assert_eq!(
            MasterCode::try_new(&"X".repeat(64)).expect("상한").as_str(),
            "X".repeat(64)
        );
    }

    #[test]
    fn duplicates_never_replace_another_master_fact() {
        let master = MasterInstrument::try_new(row()).expect("마스터");
        let mut other = row();
        other.instrument_id = InstrumentId::try_new("other").expect("키");
        assert_eq!(
            MasterInstrumentFacts::try_new(vec![
                master.clone(),
                MasterInstrument::try_new(other).expect("마스터")
            ]),
            Err(MasterInstrumentError::DuplicateIdentity)
        );
        let mut same_id = row();
        same_id.symbol = Symbol::try_new("005930").expect("심볼");
        assert_eq!(
            MasterInstrumentFacts::try_new(vec![
                master,
                MasterInstrument::try_new(same_id).expect("마스터")
            ]),
            Err(MasterInstrumentError::DuplicateIdentity)
        );
    }

    proptest! {
        #![proptest_config(Config::with_cases(4096))]
        #[test]
        fn derivations_round_trip_one_row(symbol in "[0-9A-Z]{6}", id in "[a-z]{1,32}", name in "[가-힣]{1,20}", security in sample::select(vec![("ETF", ExcludedSecurityType::Etf), ("ETN", ExcludedSecurityType::Etn), ("FOREIGN_STOCK", ExcludedSecurityType::ForeignStock)])) {
            let mut input = row();
            input.symbol = Symbol::try_new(&symbol).expect("심볼");
            input.instrument_id = InstrumentId::try_new(&id).expect("키");
            input.name = DisplayInstrumentName::try_new(&name).expect("이름");
            input.security_type = MasterSecurityType::from_code(MasterCode::try_new(security.0).expect("유형"));
            let master = MasterInstrument::try_new(input.clone()).expect("마스터");
            let query = master.quote_instrument().expect("시세 식별");
            let order = master.order_instrument().expect("주문");
            let (named, display) = master.name_fact().expect("이름 사실").into_parts();
            prop_assert_eq!(query.instrument_id(), &input.instrument_id);
            prop_assert_eq!(query.symbol(), order.symbol());
            prop_assert_eq!(&Market::from(*query.market()), &order.market());
            let policy = MasterInstrumentFacts::try_new(vec![master]).expect("목록").policy_security_facts();
            prop_assert_eq!(policy.security_type(&AllocationInstrument::new(&order)), Some(security.1));
            prop_assert_eq!(named, order);
            prop_assert_eq!(display, input.name);
        }
        #[test]
        fn unknown_codes_survive_but_never_become_known_facts(suffix in "[a-z]{1,20}") {
            let code = format!("FUTURE_{suffix}");
            let mut input = row();
            input.security_type = MasterSecurityType::from_code(MasterCode::try_new(&code).expect("유형"));
            let known_market = MasterInstrument::try_new(input.clone()).expect("미지 유형 보존");
            prop_assert_eq!(known_market.row().security_type.code().as_str(), &code);
            prop_assert_eq!(known_market.row().security_type.known(), None);
            prop_assert_eq!(known_market.row().security_type.knowledge(), SecurityTypeKnowledge::Unknown);
            prop_assert_eq!(MasterInstrumentFacts::try_new(vec![known_market]).expect("목록").policy_security_facts(), PolicySecurityFacts::default());
            input.listing_market = MasterCode::try_new(&code).expect("시장");
            input.korean_detail = None;
            let master = MasterInstrument::try_new(input).expect("미지 시장 보존");
            let market_code = master.row().listing_market.clone();
            prop_assert_eq!(market_code.as_str(), code);
            prop_assert_eq!(KnownMarket::try_from_market(&master.market), Err(UnknownKnownMarket));
            prop_assert_eq!(master.order_instrument(), Err(MasterInstrumentError::UnknownMarket));
            prop_assert_eq!(master.name_fact(), Err(MasterInstrumentError::UnknownMarket));
        }
    }

    #[test]
    fn all_schema_types_and_listing_markets_have_exact_mappings() {
        for (code, expected) in [
            ("STOCK", ExcludedSecurityType::Stock),
            ("FOREIGN_STOCK", ExcludedSecurityType::ForeignStock),
            (
                "DEPOSITARY_RECEIPT",
                ExcludedSecurityType::DepositaryReceipt,
            ),
            (
                "INFRASTRUCTURE_FUND",
                ExcludedSecurityType::InfrastructureFund,
            ),
            ("REIT", ExcludedSecurityType::Reit),
            ("ETF", ExcludedSecurityType::Etf),
            ("FOREIGN_ETF", ExcludedSecurityType::ForeignEtf),
            ("ETN", ExcludedSecurityType::Etn),
            ("STOCK_WARRANTS", ExcludedSecurityType::StockWarrants),
        ] {
            let kind = MasterSecurityType::from_code(MasterCode::try_new(code).expect("유형"));
            assert_eq!(kind.code().as_str(), code);
            assert_eq!(kind.known(), Some(expected));
            assert_eq!(kind.knowledge(), SecurityTypeKnowledge::Known(expected));
        }
    }

    #[test]
    fn listing_markets_have_exact_mappings() {
        for (code, expected, currency, symbol, known) in [
            (
                "KOSPI",
                Market::Kr,
                Currency::Krw,
                "005930",
                KnownMarket::Kr,
            ),
            (
                "KOSDAQ",
                Market::Kr,
                Currency::Krw,
                "0101N0",
                KnownMarket::Kr,
            ),
            (
                "KR_ETC",
                Market::Kr,
                Currency::Krw,
                "A12345",
                KnownMarket::Kr,
            ),
            ("NYSE", Market::Us, Currency::Usd, "BRK.B", KnownMarket::Us),
            ("NASDAQ", Market::Us, Currency::Usd, "AAPL", KnownMarket::Us),
            ("AMEX", Market::Us, Currency::Usd, "BRK-B", KnownMarket::Us),
            ("US_ETC", Market::Us, Currency::Usd, "A", KnownMarket::Us),
        ] {
            let mut input = row();
            input.listing_market = MasterCode::try_new(code).expect("시장");
            input.currency = currency;
            if currency == Currency::Usd {
                input.korean_detail = None;
            }
            input.symbol = Symbol::try_new(symbol).expect("심볼");
            let master = MasterInstrument::try_new(input).expect("마스터");
            assert_eq!(
                &Market::from(*master.quote_instrument().expect("시세 식별").market()),
                &expected
            );
            assert_eq!(KnownMarket::try_from_market(&expected), Ok(known));
        }
        for code in ["KR", "US", &"X".repeat(33)] {
            let mut input = row();
            input.listing_market = MasterCode::try_new(code).expect("코드");
            assert_eq!(
                MasterInstrument::try_new(input),
                Err(MasterInstrumentError::InvalidCode)
            );
        }
    }

    #[test]
    fn status_details_and_price_mapping_preserve_source_and_reject_invalid_time() {
        for status in [
            ListingStatus::Scheduled,
            ListingStatus::Active,
            ListingStatus::Delisted,
        ] {
            for common in [CommonShare::Common, CommonShare::Other] {
                check_details(status, common);
            }
        }
    }

    #[test]
    fn price_mapping_preserves_identity_and_rejects_invalid_observation() {
        let input = row();
        let master = MasterInstrument::try_new(input.clone()).expect("마스터");
        let price = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        let wrong_price = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Usd)
            .expect("가격");
        assert_eq!(
            master.price_observation(wrong_price, input.observed_at),
            Err(MasterInstrumentError::CurrencyMismatch)
        );
        let observation = master
            .price_observation(price, input.observed_at)
            .expect("관측");
        assert_eq!(observation.instrument_id(), Some(&input.instrument_id));
        assert_eq!(
            observation.instrument(),
            &master.order_instrument().expect("주문")
        );
        assert_eq!(observation.price(), price);
        let invalid = Timestamp::from_parts(
            UnixSeconds::new(0),
            Nanosecond::new(0),
            OffsetSeconds::new(0),
        )
        .expect("시각");
        assert_eq!(
            master.price_observation(price, invalid),
            Err(MasterInstrumentError::InvalidPriceObservation)
        );
        assert_eq!(
            MasterInstrumentFacts::try_new(vec![])
                .expect("빈 목록")
                .policy_security_facts(),
            PolicySecurityFacts::default()
        );
    }

    #[test]
    fn unknown_market_rejects_details_and_policy_facts() {
        let mut unknown = row();
        unknown.listing_market = MasterCode::try_new("JP").expect("시장");
        unknown.korean_detail = Some(KoreanMasterDetail {
            liquidation: LiquidationTrading::Regular,
            krx: TradingSuspension::Clear,
            nxt: NxtTrading::Unsupported,
        });
        assert_eq!(
            MasterInstrument::try_new(unknown.clone()),
            Err(MasterInstrumentError::UnexpectedKoreanDetail)
        );
        unknown.korean_detail = None;
        let unknown_master = MasterInstrument::try_new(unknown.clone()).expect("미지 시장");
        let price = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        assert_eq!(
            unknown_master.price_observation(price, unknown_master.row().observed_at),
            Err(MasterInstrumentError::UnknownMarket)
        );

        assert_eq!(
            MasterInstrumentFacts::try_new(vec![
                MasterInstrument::try_new(unknown).expect("미지 시장")
            ])
            .expect("목록")
            .policy_security_facts(),
            PolicySecurityFacts::default()
        );
    }

    #[test]
    fn reviewed_boundaries_reject_unsafe_orders_and_unknown_quotes() {
        for status in [ListingStatus::Scheduled, ListingStatus::Delisted] {
            let mut input = row();
            input.listing_status = status;
            let master = MasterInstrument::try_new(input).expect("마스터");
            assert_eq!(
                master.order_instrument(),
                Err(MasterInstrumentError::NotListed)
            );
            let gate = master.order_gate_facts().expect("게이트");
            assert_eq!(gate.row().listing_status, status);
            assert_eq!(
                gate.instrument_for(crate::OrderSide::Sell),
                Err(MasterInstrumentError::NotListed)
            );
            assert_eq!(master.name_fact(), Err(MasterInstrumentError::NotListed));
            let price = Price::parse(
                &NumericInput::try_new("100").expect("가격 입력"),
                Currency::Krw,
            )
            .expect("가격");
            assert_eq!(
                master.price_observation(price, master.row().observed_at),
                Err(MasterInstrumentError::NotListed)
            );
            assert_eq!(
                gate.instrument_for(crate::OrderSide::Buy),
                Err(MasterInstrumentError::NotListed)
            );
        }
        let mut foreign_input = row();
        foreign_input.listing_market = MasterCode::try_new("JP").expect("시장");
        foreign_input.korean_detail = None;
        assert_eq!(
            MasterInstrument::try_new(foreign_input)
                .expect("마스터")
                .quote_instrument(),
            Err(MasterInstrumentError::UnknownMarket)
        );
        for symbol in ["aapl", "a12345"] {
            let mut input = row();
            input.symbol = Symbol::try_new(symbol).expect("심볼");
            assert_eq!(
                MasterInstrument::try_new(input),
                Err(MasterInstrumentError::SymbolMismatch)
            );
        }
    }

    #[test]
    fn gate_facts_keep_directional_restrictions_and_source() {
        for (liquidation, suspension, buy, sell) in [
            (
                LiquidationTrading::Regular,
                TradingSuspension::Clear,
                Ok(()),
                Ok(()),
            ),
            (
                LiquidationTrading::Liquidation,
                TradingSuspension::Clear,
                Err(MasterInstrumentError::LiquidationBuy),
                Ok(()),
            ),
            (
                LiquidationTrading::Regular,
                TradingSuspension::Suspended,
                Err(MasterInstrumentError::TradingSuspended),
                Err(MasterInstrumentError::TradingSuspended),
            ),
        ] {
            let mut input = row();
            input.korean_detail = Some(KoreanMasterDetail {
                liquidation,
                krx: suspension,
                nxt: NxtTrading::Supported(TradingSuspension::Suspended),
            });
            let master = MasterInstrument::try_new(input.clone()).expect("마스터");
            let gate = master.order_gate_facts().expect("게이트");
            assert_eq!(gate.row(), &input);
            assert_eq!(format!("{gate:?}"), "MasterOrderGateFacts { .. }");
            assert_eq!(gate.instrument_for(crate::OrderSide::Buy).map(|_| ()), buy);
            assert_eq!(
                gate.instrument_for(crate::OrderSide::Sell).map(|_| ()),
                sell
            );
            check_directional_facts(&master, crate::OrderSide::Buy, buy);
            check_directional_facts(&master, crate::OrderSide::Sell, sell);
        }
        let mut input = row();
        input.korean_detail = None;
        assert_eq!(
            MasterInstrument::try_new(input)
                .expect("마스터")
                .order_instrument(),
            Err(MasterInstrumentError::MissingTradingStatus)
        );
        let mut unknown_input = row();
        unknown_input.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("FUTURE").expect("유형"));
        let master = MasterInstrument::try_new(unknown_input).expect("마스터");
        assert_eq!(
            master.order_instrument(),
            Err(MasterInstrumentError::UnknownSecurityType)
        );
        assert_eq!(
            master.quote_instrument().expect("시세 식별").currency(),
            Currency::Krw
        );
    }

    #[test]
    fn nxt_nullable_contract_has_exactly_two_valid_shapes() {
        for state in [TradingSuspension::Clear, TradingSuspension::Suspended] {
            assert_eq!(
                NxtTrading::try_from_parts(crate::NxtSupport::Supported, Some(state)),
                Ok(NxtTrading::Supported(state))
            );
            assert_eq!(
                NxtTrading::try_from_parts(crate::NxtSupport::Unsupported, Some(state)),
                Err(MasterInstrumentError::InvalidNxtStatus)
            );
        }
        assert_eq!(
            NxtTrading::try_from_parts(crate::NxtSupport::Supported, None),
            Err(MasterInstrumentError::InvalidNxtStatus)
        );
        assert_eq!(
            NxtTrading::try_from_parts(crate::NxtSupport::Unsupported, None),
            Ok(NxtTrading::Unsupported)
        );
    }

    #[test]
    fn partial_policy_facts_and_quotes_keep_good_rows_without_unknown_rows() {
        let valid = MasterInstrument::try_new(row()).expect("마스터");
        let key = AllocationInstrument::new(&valid.order_instrument().expect("주문"));
        let query = valid.quote_instrument().expect("시세");
        assert_eq!(query.currency(), Currency::Krw);
        let mut unknown = row();
        unknown.symbol = Symbol::try_new("005930").expect("심볼");
        unknown.instrument_id = InstrumentId::try_new("unknown").expect("키");
        unknown.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("FUTURE").expect("유형"));
        let unknown_master = MasterInstrument::try_new(unknown).expect("마스터");
        let unknown_key =
            AllocationInstrument::new(&unknown_master.resolved_instrument().expect("내부 조회"));
        let mut unknown_market = row();
        unknown_market.listing_market = MasterCode::try_new("JP").expect("시장");
        unknown_market.korean_detail = None;
        unknown_market.instrument_id = InstrumentId::try_new("jp").expect("키");
        unknown_market.security_type =
            MasterSecurityType::from_code(MasterCode::try_new("ETF").expect("유형"));
        let facts = MasterInstrumentFacts::try_new(vec![
            valid,
            unknown_master,
            MasterInstrument::try_new(unknown_market).expect("마스터"),
        ])
        .expect("목록");
        let policy = facts.policy_security_facts();
        assert_eq!(
            policy.security_type(&key),
            Some(ExcludedSecurityType::Stock)
        );
        assert_eq!(policy.security_type(&unknown_key), None);
        assert_eq!(facts.instruments().len(), 3);
    }

    #[test]
    fn us_lowercase_aliases_are_rejected_before_collection() {
        for symbol in ["aapl", "AaPL"] {
            let mut input = row();
            input.listing_market = MasterCode::try_new("NASDAQ").expect("시장");
            input.currency = Currency::Usd;
            input.korean_detail = None;
            input.symbol = Symbol::try_new(symbol).expect("심볼");
            assert_eq!(
                MasterInstrument::try_new(input),
                Err(MasterInstrumentError::SymbolMismatch)
            );
        }
    }

    fn check_directional_facts(
        master: &MasterInstrument,
        side: crate::OrderSide,
        expected: Result<(), MasterInstrumentError>,
    ) {
        let price = Price::parse(&NumericInput::try_new("100").expect("입력"), Currency::Krw)
            .expect("가격");
        if side == crate::OrderSide::Buy {
            assert_eq!(master.order_instrument().map(|_| ()), expected);
            assert_eq!(master.name_fact().map(|_| ()), expected);
            assert_eq!(
                master
                    .price_observation(price, master.row().observed_at)
                    .map(|_| ()),
                expected
            );
        }
        assert_eq!(master.name_fact_for(side).map(|_| ()), expected);
        assert_eq!(
            master
                .price_observation_for(side, price, master.row().observed_at)
                .map(|_| ()),
            expected
        );
    }

    fn korean_details() -> Vec<KoreanMasterDetail> {
        let mut details = Vec::new();
        for liquidation in [LiquidationTrading::Regular, LiquidationTrading::Liquidation] {
            for krx in [TradingSuspension::Clear, TradingSuspension::Suspended] {
                append_nxt_details(&mut details, liquidation, krx);
            }
        }
        details
    }
    fn check_details(status: ListingStatus, common: CommonShare) {
        for detail in korean_details() {
            let mut input = row();
            input.listing_status = status;
            input.common_share = common;
            input.korean_detail = Some(detail);
            assert_eq!(
                MasterInstrument::try_new(input.clone())
                    .expect("사실")
                    .row(),
                &input
            );
        }
    }
    fn append_nxt_details(
        details: &mut Vec<KoreanMasterDetail>,
        liquidation: LiquidationTrading,
        krx: TradingSuspension,
    ) {
        for nxt in [
            NxtTrading::Unsupported,
            NxtTrading::Supported(TradingSuspension::Clear),
            NxtTrading::Supported(TradingSuspension::Suspended),
        ] {
            details.push(KoreanMasterDetail {
                liquidation,
                krx,
                nxt,
            });
        }
    }
}
