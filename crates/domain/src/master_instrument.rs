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
    Supported(Option<TradingSuspension>),
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
}
#[derive(Clone, PartialEq, Eq)]
pub struct MasterInstrument {
    row: MasterInstrumentRow,
    identity: QuoteInstrument,
}
#[derive(Clone, PartialEq, Eq)]
pub struct QuoteInstrument {
    market: Market,
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
    pub const fn market(&self) -> &Market {
        &self.market
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
        let identity = QuoteInstrument {
            market,
            symbol: row.symbol.clone(),
            instrument_id: row.instrument_id.clone(),
        };
        Ok(Self { row, identity })
    }
    #[must_use]
    pub const fn row(&self) -> &MasterInstrumentRow {
        &self.row
    }
    #[must_use]
    pub fn quote_instrument(&self) -> QuoteInstrument {
        self.identity.clone()
    }
    pub fn order_instrument(&self) -> Result<ResolvedOrderInstrument, MasterInstrumentError> {
        ResolvedOrderInstrument::from_master_lookup_result(
            self.row.symbol.clone(),
            self.identity.market.clone(),
        )
        .or(Err(MasterInstrumentError::UnknownMarket))
    }
    pub fn price_observation(
        &self,
        price: crate::Price,
        observed_at: Timestamp,
    ) -> Result<crate::PriceObservation, MasterInstrumentError> {
        if price.as_money().currency() != self.row.currency {
            return Err(MasterInstrumentError::CurrencyMismatch);
        }
        crate::PriceObservation::from_identified_quote_parts(
            self.row.instrument_id.clone(),
            self.order_instrument()?,
            price,
            observed_at,
        )
        .or(Err(MasterInstrumentError::InvalidPriceObservation))
    }
    pub fn name_fact(&self) -> Result<InstrumentNameFact, MasterInstrumentError> {
        Ok(InstrumentNameFact::from_master_lookup_result(
            self.order_instrument()?,
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
            if !ids.insert(instrument.identity.instrument_id.clone())
                || !symbols.insert((
                    instrument.identity.market.clone(),
                    instrument.row.symbol.clone(),
                ))
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
    pub fn policy_security_facts(&self) -> Result<PolicySecurityFacts, MasterInstrumentError> {
        let mut values = BTreeMap::new();
        for master in &self.0 {
            let kind = master
                .row
                .security_type
                .known()
                .ok_or(MasterInstrumentError::UnknownSecurityType)?;
            values.insert(AllocationInstrument::new(&master.order_instrument()?), kind);
        }
        Ok(PolicySecurityFacts::from_master_lookup_results(values))
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
            korean_detail: None,
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
        let query = master.quote_instrument();
        assert_eq!(query.instrument_id(), &input.instrument_id);
        assert_eq!(query.symbol(), &input.symbol);
        assert_eq!(query.market(), &Market::Kr);
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
                .expect("유형 사실")
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
        fn derivations_round_trip_one_row(symbol in "[0-9A-Za-z]{6}", id in "[a-z]{1,32}", name in "[가-힣]{1,20}", security in sample::select(vec![("ETF", ExcludedSecurityType::Etf), ("ETN", ExcludedSecurityType::Etn), ("FOREIGN_STOCK", ExcludedSecurityType::ForeignStock)])) {
            let mut input = row();
            input.symbol = Symbol::try_new(&symbol).expect("심볼");
            input.instrument_id = InstrumentId::try_new(&id).expect("키");
            input.name = DisplayInstrumentName::try_new(&name).expect("이름");
            input.security_type = MasterSecurityType::from_code(MasterCode::try_new(security.0).expect("유형"));
            let master = MasterInstrument::try_new(input.clone()).expect("마스터");
            let query = master.quote_instrument();
            let order = master.order_instrument().expect("주문");
            let (named, display) = master.name_fact().expect("이름 사실").into_parts();
            prop_assert_eq!(query.instrument_id(), &input.instrument_id);
            prop_assert_eq!(query.symbol(), order.symbol());
            prop_assert_eq!(query.market(), &order.market());
            let policy = MasterInstrumentFacts::try_new(vec![master]).expect("목록").policy_security_facts().expect("유형 사실");
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
            prop_assert_eq!(MasterInstrumentFacts::try_new(vec![known_market]).expect("목록").policy_security_facts(), Err(MasterInstrumentError::UnknownSecurityType));
            input.listing_market = MasterCode::try_new(&code).expect("시장");
            let master = MasterInstrument::try_new(input).expect("미지 시장 보존");
            let market_code = master.quote_instrument().market().as_code();
            prop_assert_eq!(market_code.as_str(), code);
            prop_assert_eq!(KnownMarket::try_from_market(master.quote_instrument().market()), Err(UnknownKnownMarket));
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
                "a12345",
                KnownMarket::Kr,
            ),
            ("NYSE", Market::Us, Currency::Usd, "BRK.B", KnownMarket::Us),
            ("NASDAQ", Market::Us, Currency::Usd, "aapl", KnownMarket::Us),
            ("AMEX", Market::Us, Currency::Usd, "BRK-B", KnownMarket::Us),
            ("US_ETC", Market::Us, Currency::Usd, "A", KnownMarket::Us),
        ] {
            let mut input = row();
            input.listing_market = MasterCode::try_new(code).expect("시장");
            input.currency = currency;
            input.symbol = Symbol::try_new(symbol).expect("심볼");
            let master = MasterInstrument::try_new(input).expect("마스터");
            assert_eq!(master.quote_instrument().market(), &expected);
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
            Ok(PolicySecurityFacts::default())
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
            Err(MasterInstrumentError::UnknownMarket)
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
            NxtTrading::Supported(None),
            NxtTrading::Supported(Some(TradingSuspension::Clear)),
            NxtTrading::Supported(Some(TradingSuspension::Suspended)),
        ] {
            details.push(KoreanMasterDetail {
                liquidation,
                krx,
                nxt,
            });
        }
    }
}
