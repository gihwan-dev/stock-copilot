use crate::NewsSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataSourceCategory {
    Disclosure,
    Fundamentals,
    InstrumentMaster,
    News,
    Macro,
    WebSearch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataSource {
    KindDisclosure,
    DartDisclosureApi,
    DartDisclosureRss,
    DartFundamentals,
    KindCorpList,
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    SegyeFinance,
    GoogleNews,
    WebSearch,
    YahooFinance,
    SecEdgar,
    BankOfKorea,
    FederalReserve,
    UsTreasury,
    Fred,
}

impl DataSource {
    #[must_use]
    pub const fn all() -> [Self; 18] {
        [
            Self::KindDisclosure,
            Self::DartDisclosureApi,
            Self::DartDisclosureRss,
            Self::DartFundamentals,
            Self::KindCorpList,
            Self::Hankyung,
            Self::Maekyung,
            Self::Yonhap,
            Self::Edaily,
            Self::SegyeFinance,
            Self::GoogleNews,
            Self::WebSearch,
            Self::YahooFinance,
            Self::SecEdgar,
            Self::BankOfKorea,
            Self::FederalReserve,
            Self::UsTreasury,
            Self::Fred,
        ]
    }

    #[must_use]
    pub const fn from_news(source: &NewsSource) -> Self {
        match source {
            NewsSource::Hankyung => Self::Hankyung,
            NewsSource::Maekyung => Self::Maekyung,
            NewsSource::Yonhap => Self::Yonhap,
            NewsSource::Edaily => Self::Edaily,
            NewsSource::BankOfKorea => Self::BankOfKorea,
            NewsSource::FederalReserve => Self::FederalReserve,
            NewsSource::GoogleNews => Self::GoogleNews,
            NewsSource::YahooFinance => Self::YahooFinance,
            NewsSource::SegyeFinance => Self::SegyeFinance,
        }
    }

    #[must_use]
    pub const fn category(self) -> DataSourceCategory {
        match self {
            Self::KindDisclosure
            | Self::DartDisclosureApi
            | Self::DartDisclosureRss
            | Self::SecEdgar => DataSourceCategory::Disclosure,
            Self::DartFundamentals => DataSourceCategory::Fundamentals,
            Self::KindCorpList => DataSourceCategory::InstrumentMaster,
            Self::Hankyung
            | Self::Maekyung
            | Self::Yonhap
            | Self::Edaily
            | Self::SegyeFinance
            | Self::GoogleNews
            | Self::YahooFinance => DataSourceCategory::News,
            Self::BankOfKorea | Self::FederalReserve | Self::UsTreasury | Self::Fred => {
                DataSourceCategory::Macro
            }
            Self::WebSearch => DataSourceCategory::WebSearch,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DataSource, DataSourceCategory};
    use crate::NewsSource;
    use std::collections::BTreeSet;

    #[test]
    fn all_sources_are_unique_and_cover_every_category() {
        let all = DataSource::all();
        assert_eq!(all.len(), 18);
        let unique: BTreeSet<DataSource> = all.into_iter().collect();
        assert_eq!(unique.len(), 18);
        for (source, expected) in [
            (DataSource::KindDisclosure, DataSourceCategory::Disclosure),
            (
                DataSource::DartDisclosureApi,
                DataSourceCategory::Disclosure,
            ),
            (
                DataSource::DartDisclosureRss,
                DataSourceCategory::Disclosure,
            ),
            (
                DataSource::DartFundamentals,
                DataSourceCategory::Fundamentals,
            ),
            (
                DataSource::KindCorpList,
                DataSourceCategory::InstrumentMaster,
            ),
            (DataSource::Hankyung, DataSourceCategory::News),
            (DataSource::Maekyung, DataSourceCategory::News),
            (DataSource::Yonhap, DataSourceCategory::News),
            (DataSource::Edaily, DataSourceCategory::News),
            (DataSource::GoogleNews, DataSourceCategory::News),
            (DataSource::WebSearch, DataSourceCategory::WebSearch),
            (DataSource::YahooFinance, DataSourceCategory::News),
            (DataSource::SecEdgar, DataSourceCategory::Disclosure),
            (DataSource::BankOfKorea, DataSourceCategory::Macro),
            (DataSource::FederalReserve, DataSourceCategory::Macro),
            (DataSource::UsTreasury, DataSourceCategory::Macro),
            (DataSource::Fred, DataSourceCategory::Macro),
        ] {
            assert_eq!(source.category(), expected);
        }
    }

    #[test]
    fn every_news_source_maps_one_to_one_to_data_source() {
        for (news, expected) in [
            (NewsSource::Hankyung, DataSource::Hankyung),
            (NewsSource::Maekyung, DataSource::Maekyung),
            (NewsSource::Yonhap, DataSource::Yonhap),
            (NewsSource::Edaily, DataSource::Edaily),
            (NewsSource::BankOfKorea, DataSource::BankOfKorea),
            (NewsSource::FederalReserve, DataSource::FederalReserve),
            (NewsSource::GoogleNews, DataSource::GoogleNews),
            (NewsSource::YahooFinance, DataSource::YahooFinance),
        ] {
            assert_eq!(DataSource::from_news(&news), expected);
        }
    }
}
