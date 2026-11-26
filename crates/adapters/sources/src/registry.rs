use std::time::Duration;

use application::{AiReadPolicy, ConditionalGet, CredentialSource, UserAgentRequirement};
use domain::DataSource;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HttpDataSource {
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
    YahooFinance,
    SecEdgar,
    BankOfKorea,
    FederalReserve,
    UsTreasury,
    Fred,
}

impl HttpDataSource {
    pub const ALL: [Self; 17] = [
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
        Self::YahooFinance,
        Self::SecEdgar,
        Self::BankOfKorea,
        Self::FederalReserve,
        Self::UsTreasury,
        Self::Fred,
    ];

    #[must_use]
    pub const fn data_source(self) -> DataSource {
        match self {
            Self::KindDisclosure => DataSource::KindDisclosure,
            Self::DartDisclosureApi => DataSource::DartDisclosureApi,
            Self::DartDisclosureRss => DataSource::DartDisclosureRss,
            Self::DartFundamentals => DataSource::DartFundamentals,
            Self::KindCorpList => DataSource::KindCorpList,
            Self::Hankyung => DataSource::Hankyung,
            Self::Maekyung => DataSource::Maekyung,
            Self::Yonhap => DataSource::Yonhap,
            Self::Edaily => DataSource::Edaily,
            Self::SegyeFinance => DataSource::SegyeFinance,
            Self::GoogleNews => DataSource::GoogleNews,
            Self::YahooFinance => DataSource::YahooFinance,
            Self::SecEdgar => DataSource::SecEdgar,
            Self::BankOfKorea => DataSource::BankOfKorea,
            Self::FederalReserve => DataSource::FederalReserve,
            Self::UsTreasury => DataSource::UsTreasury,
            Self::Fred => DataSource::Fred,
        }
    }

    #[must_use]
    pub const fn try_from_data_source(source: DataSource) -> Option<Self> {
        match source {
            DataSource::KindDisclosure => Some(Self::KindDisclosure),
            DataSource::DartDisclosureApi => Some(Self::DartDisclosureApi),
            DataSource::DartDisclosureRss => Some(Self::DartDisclosureRss),
            DataSource::DartFundamentals => Some(Self::DartFundamentals),
            DataSource::KindCorpList => Some(Self::KindCorpList),
            DataSource::Hankyung => Some(Self::Hankyung),
            DataSource::Maekyung => Some(Self::Maekyung),
            DataSource::Yonhap => Some(Self::Yonhap),
            DataSource::Edaily => Some(Self::Edaily),
            DataSource::SegyeFinance => Some(Self::SegyeFinance),
            DataSource::GoogleNews => Some(Self::GoogleNews),
            DataSource::WebSearch => None,
            DataSource::YahooFinance => Some(Self::YahooFinance),
            DataSource::SecEdgar => Some(Self::SecEdgar),
            DataSource::BankOfKorea => Some(Self::BankOfKorea),
            DataSource::FederalReserve => Some(Self::FederalReserve),
            DataSource::UsTreasury => Some(Self::UsTreasury),
            DataSource::Fred => Some(Self::Fred),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RateLimitBucket {
    Kind,
    OpenDart,
    DartRss,
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    SegyeFinance,
    GoogleNews,
    YahooFinance,
    SecEdgar,
    BankOfKorea,
    FederalReserve,
    UsTreasury,
    Fred,
}

impl RateLimitBucket {
    pub const ALL: [Self; 15] = [
        Self::Kind,
        Self::OpenDart,
        Self::DartRss,
        Self::Hankyung,
        Self::Maekyung,
        Self::Yonhap,
        Self::Edaily,
        Self::SegyeFinance,
        Self::GoogleNews,
        Self::YahooFinance,
        Self::SecEdgar,
        Self::BankOfKorea,
        Self::FederalReserve,
        Self::UsTreasury,
        Self::Fred,
    ];

    #[must_use]
    pub const fn min_interval(self) -> Duration {
        match self {
            Self::Kind
            | Self::OpenDart
            | Self::DartRss
            | Self::Hankyung
            | Self::Maekyung
            | Self::Yonhap
            | Self::Edaily
            | Self::SegyeFinance
            | Self::GoogleNews
            | Self::YahooFinance
            | Self::SecEdgar
            | Self::BankOfKorea
            | Self::FederalReserve
            | Self::UsTreasury
            | Self::Fred => Duration::from_secs(5),
        }
    }

    #[must_use]
    pub const fn max_calls_per_second(self) -> Option<u32> {
        match self {
            Self::SecEdgar => Some(10),
            Self::Kind
            | Self::OpenDart
            | Self::DartRss
            | Self::Hankyung
            | Self::Maekyung
            | Self::Yonhap
            | Self::Edaily
            | Self::SegyeFinance
            | Self::GoogleNews
            | Self::YahooFinance
            | Self::BankOfKorea
            | Self::FederalReserve
            | Self::UsTreasury
            | Self::Fred => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialRequirement {
    None,
    Optional(CredentialSource),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiReadSettingPolicy {
    ConfigurableDefaultOff,
    LockedOff,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostAllowance {
    Allowed,
    Denied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceSpec {
    source: HttpDataSource,
    bucket: RateLimitBucket,
    allowed_hosts: &'static [&'static str],
    conditional_get: ConditionalGet,
    user_agent: UserAgentRequirement,
    min_interval: Duration,
    max_calls_per_second: Option<u32>,
    credential: CredentialRequirement,
    default_ai_read: AiReadPolicy,
    ai_read_policy: AiReadSettingPolicy,
}

impl SourceSpec {
    #[must_use]
    pub const fn source(&self) -> HttpDataSource {
        self.source
    }

    #[must_use]
    pub const fn data_source(&self) -> DataSource {
        self.source.data_source()
    }

    #[must_use]
    pub const fn bucket(&self) -> RateLimitBucket {
        self.bucket
    }

    #[must_use]
    pub const fn allowed_hosts(&self) -> &'static [&'static str] {
        self.allowed_hosts
    }

    #[must_use]
    pub const fn conditional_get(&self) -> ConditionalGet {
        self.conditional_get
    }

    #[must_use]
    pub const fn user_agent(&self) -> UserAgentRequirement {
        self.user_agent
    }

    #[must_use]
    pub const fn min_interval(&self) -> Duration {
        self.min_interval
    }

    #[must_use]
    pub const fn max_calls_per_second(&self) -> Option<u32> {
        self.max_calls_per_second
    }

    #[must_use]
    pub const fn credential(&self) -> CredentialRequirement {
        self.credential
    }

    #[must_use]
    pub const fn default_ai_read(&self) -> AiReadPolicy {
        self.default_ai_read
    }

    #[must_use]
    pub const fn ai_read_policy(&self) -> AiReadSettingPolicy {
        self.ai_read_policy
    }

    #[must_use]
    pub fn host_allowance(&self, host: &str) -> HostAllowance {
        if host.is_empty() || host.ends_with('.') {
            return HostAllowance::Denied;
        }
        let lower = host.to_ascii_lowercase();
        if self.allowed_hosts.contains(&lower.as_str()) {
            HostAllowance::Allowed
        } else {
            HostAllowance::Denied
        }
    }
}

const MIN_INTERVAL: Duration = Duration::from_secs(5);

const SPECS: [SourceSpec; 17] = [
    SourceSpec {
        source: HttpDataSource::KindDisclosure,
        bucket: RateLimitBucket::Kind,
        allowed_hosts: &["kind.krx.co.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::DartDisclosureApi,
        bucket: RateLimitBucket::OpenDart,
        allowed_hosts: &["opendart.fss.or.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::Optional(CredentialSource::Dart),
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::DartDisclosureRss,
        bucket: RateLimitBucket::DartRss,
        allowed_hosts: &["dart.fss.or.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::DartFundamentals,
        bucket: RateLimitBucket::OpenDart,
        allowed_hosts: &["opendart.fss.or.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::Optional(CredentialSource::Dart),
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::KindCorpList,
        bucket: RateLimitBucket::Kind,
        allowed_hosts: &["kind.krx.co.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::Hankyung,
        bucket: RateLimitBucket::Hankyung,
        allowed_hosts: &["www.hankyung.com"],
        conditional_get: ConditionalGet::Supported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::Maekyung,
        bucket: RateLimitBucket::Maekyung,
        allowed_hosts: &["www.mk.co.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::Yonhap,
        bucket: RateLimitBucket::Yonhap,
        allowed_hosts: &["www.yna.co.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::LockedOff,
    },
    SourceSpec {
        source: HttpDataSource::Edaily,
        bucket: RateLimitBucket::Edaily,
        allowed_hosts: &["rss.edaily.co.kr", "www.edaily.co.kr"],
        conditional_get: ConditionalGet::Supported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::SegyeFinance,
        bucket: RateLimitBucket::SegyeFinance,
        allowed_hosts: &["www.segyefn.com"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::LockedOff,
    },
    SourceSpec {
        source: HttpDataSource::GoogleNews,
        bucket: RateLimitBucket::GoogleNews,
        allowed_hosts: &["news.google.com"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleOnly,
        ai_read_policy: AiReadSettingPolicy::LockedOff,
    },
    SourceSpec {
        source: HttpDataSource::YahooFinance,
        bucket: RateLimitBucket::YahooFinance,
        allowed_hosts: &["feeds.finance.yahoo.com", "finance.yahoo.com"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Browser,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::SecEdgar,
        bucket: RateLimitBucket::SecEdgar,
        allowed_hosts: &["data.sec.gov", "www.sec.gov"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Contact,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: Some(10),
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::BankOfKorea,
        bucket: RateLimitBucket::BankOfKorea,
        allowed_hosts: &["www.bok.or.kr"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::FederalReserve,
        bucket: RateLimitBucket::FederalReserve,
        allowed_hosts: &["www.federalreserve.gov"],
        conditional_get: ConditionalGet::Supported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::UsTreasury,
        bucket: RateLimitBucket::UsTreasury,
        allowed_hosts: &["home.treasury.gov"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::None,
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
    SourceSpec {
        source: HttpDataSource::Fred,
        bucket: RateLimitBucket::Fred,
        allowed_hosts: &["api.stlouisfed.org"],
        conditional_get: ConditionalGet::Unsupported,
        user_agent: UserAgentRequirement::Standard,
        min_interval: MIN_INTERVAL,
        max_calls_per_second: None,
        credential: CredentialRequirement::Optional(CredentialSource::Fred),
        default_ai_read: AiReadPolicy::TitleAndSummary,
        ai_read_policy: AiReadSettingPolicy::ConfigurableDefaultOff,
    },
];

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SourceRegistry;

impl SourceRegistry {
    #[must_use]
    pub const fn all() -> &'static [SourceSpec; 17] {
        &SPECS
    }

    #[must_use]
    pub const fn spec(source: HttpDataSource) -> &'static SourceSpec {
        let [
            kind_disclosure,
            dart_disclosure_api,
            dart_disclosure_rss,
            dart_fundamentals,
            kind_corp_list,
            hankyung,
            maekyung,
            yonhap,
            edaily,
            segye_finance,
            google_news,
            yahoo_finance,
            sec_edgar,
            bank_of_korea,
            federal_reserve,
            us_treasury,
            fred,
        ] = &SPECS;
        match source {
            HttpDataSource::KindDisclosure => kind_disclosure,
            HttpDataSource::DartDisclosureApi => dart_disclosure_api,
            HttpDataSource::DartDisclosureRss => dart_disclosure_rss,
            HttpDataSource::DartFundamentals => dart_fundamentals,
            HttpDataSource::KindCorpList => kind_corp_list,
            HttpDataSource::Hankyung => hankyung,
            HttpDataSource::Maekyung => maekyung,
            HttpDataSource::Yonhap => yonhap,
            HttpDataSource::Edaily => edaily,
            HttpDataSource::SegyeFinance => segye_finance,
            HttpDataSource::GoogleNews => google_news,
            HttpDataSource::YahooFinance => yahoo_finance,
            HttpDataSource::SecEdgar => sec_edgar,
            HttpDataSource::BankOfKorea => bank_of_korea,
            HttpDataSource::FederalReserve => federal_reserve,
            HttpDataSource::UsTreasury => us_treasury,
            HttpDataSource::Fred => fred,
        }
    }
}
