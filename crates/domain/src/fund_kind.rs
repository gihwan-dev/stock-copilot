use crate::{DisplayInstrumentName, ExcludedSecurityType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FundKind {
    SingleSecurity,
    BroadIndexFund,
    LeveragedOrInverse,
    SectorOrThemeFund,
}

impl FundKind {
    #[must_use]
    pub fn classify(
        security: Option<ExcludedSecurityType>,
        name: Option<&DisplayInstrumentName>,
        symbol: Option<&crate::Symbol>,
    ) -> Self {
        match security {
            Some(ExcludedSecurityType::Etf | ExcludedSecurityType::ForeignEtf) => {
                Self::classify_fund(name, symbol, Self::SectorOrThemeFund)
            }
            Some(ExcludedSecurityType::Etn) => {
                match Self::classify_fund(name, symbol, Self::SectorOrThemeFund) {
                    Self::LeveragedOrInverse => Self::LeveragedOrInverse,
                    Self::SingleSecurity | Self::BroadIndexFund | Self::SectorOrThemeFund => {
                        Self::SectorOrThemeFund
                    }
                }
            }
            Some(
                ExcludedSecurityType::Stock
                | ExcludedSecurityType::ForeignStock
                | ExcludedSecurityType::DepositaryReceipt
                | ExcludedSecurityType::InfrastructureFund
                | ExcludedSecurityType::Reit
                | ExcludedSecurityType::StockWarrants,
            )
            | None => Self::SingleSecurity,
        }
    }

    fn classify_fund(
        name: Option<&DisplayInstrumentName>,
        symbol: Option<&crate::Symbol>,
        fallback: Self,
    ) -> Self {
        let upper = FundName::new(name);
        if upper.leveraged() == Marker::Present {
            return Self::LeveragedOrInverse;
        }
        let broad_symbol = symbol.is_some_and(|value| {
            [
                "SPY", "VOO", "IVV", "VTI", "VT", "QQQ", "QQQM", "SPLG", "AGG",
            ]
            .contains(&value.as_str())
        });
        if upper.broad() == Marker::Present || broad_symbol {
            Self::BroadIndexFund
        } else {
            fallback
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Present,
    Absent,
}

impl Marker {
    fn of(mut found: impl Iterator<Item = Self>) -> Self {
        match found.next() {
            Some(marker) => marker,
            None => Self::Absent,
        }
    }
}

struct FundName(String);

impl FundName {
    fn new(name: Option<&DisplayInstrumentName>) -> Self {
        Self(name.map_or_else(String::new, |value| {
            format!(" {} ", value.as_str().to_uppercase())
        }))
    }

    fn leveraged(&self) -> Marker {
        Marker::of(
            [
                "레버리지",
                "인버스",
                "곱버스",
                "2X",
                "3X",
                "-1X",
                "ULTRA",
                " BULL ",
                " BEAR ",
                "DAILY",
                "LEVERAGED",
                "INVERSE",
            ]
            .iter()
            .filter(|marker| self.0.contains(*marker))
            .map(|_marker| Marker::Present),
        )
    }

    fn broad(&self) -> Marker {
        let trimmed = self.0.trim_end();
        Marker::of(
            [
                "S&P500",
                "S&P 500",
                "나스닥100",
                "NASDAQ100",
                "NASDAQ-100",
                "NASDAQ 100",
                "QQQ",
                "코스피200",
                "KOSPI200",
                "KOSPI 200",
                "코스닥150",
                "KOSDAQ150",
                "MSCI WORLD",
                "MSCI ACWI",
                "MSCI EM",
                "ACWI",
                "전세계",
                "TOTAL STOCK",
                "TOTAL WORLD",
                "TOTAL MARKET",
                "다우존스",
                "DOW JONES",
                "니케이225",
                "NIKKEI",
                "유로스탁스50",
                "STOXX",
                "국고채",
                "TREASURY",
                "종합채권",
                "AGGREGATE",
            ]
            .iter()
            .filter(|marker| self.0.contains(*marker))
            .chain(
                [" 200", " 200TR", " 200 TR"]
                    .iter()
                    .filter(|suffix| trimmed.ends_with(*suffix)),
            )
            .map(|_marker| Marker::Present),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        DisplayInstrumentName, ExcludedSecurityType, FundKind, NumericInput, ProfileWeight, Symbol,
        WeightCaps,
    };

    fn name(text: &str) -> DisplayInstrumentName {
        DisplayInstrumentName::try_new(text).expect("이름")
    }

    fn kind(security: ExcludedSecurityType, text: &str, symbol: &str) -> FundKind {
        FundKind::classify(
            Some(security),
            Some(&name(text)),
            Some(&Symbol::try_new(symbol).expect("심볼")),
        )
    }

    fn percent(text: &str) -> ProfileWeight {
        ProfileWeight::parse_percent(&NumericInput::try_new(text).expect("입력")).expect("비중")
    }

    #[test]
    fn broad_index_funds_are_recognized_by_name_or_symbol() {
        for (security, text, symbol) in [
            (ExcludedSecurityType::Etf, "TIGER 미국S&P500", "360750"),
            (ExcludedSecurityType::Etf, "KODEX 200", "069500"),
            (ExcludedSecurityType::Etf, "TIGER 미국나스닥100", "133690"),
            (ExcludedSecurityType::Etf, "KOSEF 국고채10년", "148070"),
            (
                ExcludedSecurityType::ForeignEtf,
                "Vanguard S&P 500 ETF",
                "VOO",
            ),
            (ExcludedSecurityType::ForeignEtf, "Invesco QQQ Trust", "QQQ"),
            (ExcludedSecurityType::ForeignEtf, "SPDR Portfolio", "SPLG"),
        ] {
            assert_eq!(
                kind(security, text, symbol),
                FundKind::BroadIndexFund,
                "{text}"
            );
        }
    }

    #[test]
    fn leverage_wins_over_a_broad_index_name() {
        for (security, text, symbol) in [
            (ExcludedSecurityType::Etf, "KODEX 레버리지", "122630"),
            (ExcludedSecurityType::Etf, "KODEX 200선물인버스2X", "252670"),
            (
                ExcludedSecurityType::ForeignEtf,
                "ProShares UltraPro QQQ",
                "TQQQ",
            ),
            (
                ExcludedSecurityType::ForeignEtf,
                "Direxion Daily Semiconductor Bull 3X",
                "SOXL",
            ),
            (
                ExcludedSecurityType::Etn,
                "삼성 레버리지 WTI원유 선물 ETN",
                "530031",
            ),
        ] {
            assert_eq!(
                kind(security, text, symbol),
                FundKind::LeveragedOrInverse,
                "{text}"
            );
        }
    }

    #[test]
    fn sector_theme_funds_and_stocks_keep_the_single_instrument_cap() {
        assert_eq!(
            kind(ExcludedSecurityType::Etf, "KODEX 반도체", "091160"),
            FundKind::SectorOrThemeFund
        );
        assert_eq!(
            kind(ExcludedSecurityType::Etf, "KODEX 200 IT", "000001"),
            FundKind::SectorOrThemeFund
        );
        assert_eq!(
            kind(ExcludedSecurityType::Etn, "신한 구리 선물 ETN", "500011"),
            FundKind::SectorOrThemeFund
        );
        assert_eq!(
            kind(ExcludedSecurityType::Stock, "삼성전자 200", "005930"),
            FundKind::SingleSecurity
        );
        assert_eq!(
            FundKind::classify(Some(ExcludedSecurityType::Etf), None, None),
            FundKind::SectorOrThemeFund
        );
        assert_eq!(
            FundKind::classify(None, None, None),
            FundKind::SingleSecurity
        );
    }

    #[test]
    fn limits_follow_the_saved_caps_for_each_kind() {
        let caps = WeightCaps::default();
        let single = caps.limits_for(FundKind::SingleSecurity);
        assert_eq!(
            (single.instrument_cap(), single.instrument_warning()),
            (percent("20"), percent("10"))
        );
        let theme = caps.limits_for(FundKind::SectorOrThemeFund);
        assert_eq!(
            (theme.instrument_cap(), theme.instrument_warning()),
            (percent("20"), percent("10"))
        );
        let broad = caps.limits_for(FundKind::BroadIndexFund);
        assert_eq!(
            (broad.instrument_cap(), broad.instrument_warning()),
            (percent("40"), percent("20"))
        );
        let levered = caps.limits_for(FundKind::LeveragedOrInverse);
        assert_eq!(
            (levered.instrument_cap(), levered.instrument_warning()),
            (percent("10"), percent("10"))
        );
    }

    #[test]
    fn limits_move_with_user_changes() {
        let caps = WeightCaps::try_new(percent("30"), percent("15"), percent("50")).expect("한도");
        let broad = caps.limits_for(FundKind::BroadIndexFund);
        assert_eq!(
            (broad.instrument_cap(), broad.instrument_warning()),
            (percent("50"), percent("30"))
        );
        let levered = caps.limits_for(FundKind::LeveragedOrInverse);
        assert_eq!(
            (levered.instrument_cap(), levered.instrument_warning()),
            (percent("15"), percent("15"))
        );
    }
}
