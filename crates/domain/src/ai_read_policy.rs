use crate::{InformationSource, NewsItem, NewsSource, NormalizedUrl, UntrustedText};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBodyReadToggle {
    Disabled,
    Enabled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiBodyReadPolicy {
    TitleOnly,
    TitleAndSummary,
    BodyAllowed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AiBodyReadSettingError {
    SourceLockedOff(NewsSource),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAiBodyReadSettings {
    hankyung: AiBodyReadToggle,
    maekyung: AiBodyReadToggle,
    edaily: AiBodyReadToggle,
    bank_of_korea: AiBodyReadToggle,
    federal_reserve: AiBodyReadToggle,
    yahoo_finance: AiBodyReadToggle,
}

impl Default for SourceAiBodyReadSettings {
    fn default() -> Self {
        Self {
            hankyung: AiBodyReadToggle::Disabled,
            maekyung: AiBodyReadToggle::Disabled,
            edaily: AiBodyReadToggle::Disabled,
            bank_of_korea: AiBodyReadToggle::Disabled,
            federal_reserve: AiBodyReadToggle::Disabled,
            yahoo_finance: AiBodyReadToggle::Disabled,
        }
    }
}

impl SourceAiBodyReadSettings {
    #[must_use]
    pub fn all_disabled() -> Self {
        Self::default()
    }

    #[must_use]
    pub const fn toggle_for(&self, source: &NewsSource) -> AiBodyReadToggle {
        match source {
            NewsSource::Hankyung => self.hankyung,
            NewsSource::Maekyung => self.maekyung,
            NewsSource::Edaily => self.edaily,
            NewsSource::BankOfKorea => self.bank_of_korea,
            NewsSource::FederalReserve => self.federal_reserve,
            NewsSource::YahooFinance => self.yahoo_finance,
            NewsSource::Yonhap | NewsSource::GoogleNews => AiBodyReadToggle::Disabled,
        }
    }

    pub fn set(
        &mut self,
        source: &NewsSource,
        toggle: AiBodyReadToggle,
    ) -> Result<(), AiBodyReadSettingError> {
        match source {
            NewsSource::Hankyung => self.hankyung = toggle,
            NewsSource::Maekyung => self.maekyung = toggle,
            NewsSource::Edaily => self.edaily = toggle,
            NewsSource::BankOfKorea => self.bank_of_korea = toggle,
            NewsSource::FederalReserve => self.federal_reserve = toggle,
            NewsSource::YahooFinance => self.yahoo_finance = toggle,
            NewsSource::Yonhap | NewsSource::GoogleNews => match toggle {
                AiBodyReadToggle::Disabled => {}
                AiBodyReadToggle::Enabled => {
                    return Err(AiBodyReadSettingError::SourceLockedOff(source.clone()));
                }
            },
        }
        Ok(())
    }

    #[must_use]
    pub const fn policy_for(&self, source: &NewsSource) -> AiBodyReadPolicy {
        match source {
            NewsSource::GoogleNews => AiBodyReadPolicy::TitleOnly,
            NewsSource::Yonhap => AiBodyReadPolicy::TitleAndSummary,
            NewsSource::Hankyung
            | NewsSource::Maekyung
            | NewsSource::Edaily
            | NewsSource::BankOfKorea
            | NewsSource::FederalReserve
            | NewsSource::YahooFinance => match self.toggle_for(source) {
                AiBodyReadToggle::Disabled => AiBodyReadPolicy::TitleAndSummary,
                AiBodyReadToggle::Enabled => AiBodyReadPolicy::BodyAllowed,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AiNewsInput {
    source: NewsSource,
    url: NormalizedUrl,
    title: UntrustedText,
    summary: Option<UntrustedText>,
    body: Option<UntrustedText>,
}

impl AiNewsInput {
    #[must_use]
    pub const fn source(&self) -> &NewsSource {
        &self.source
    }

    #[must_use]
    pub const fn url(&self) -> &NormalizedUrl {
        &self.url
    }

    #[must_use]
    pub const fn title(&self) -> &UntrustedText {
        &self.title
    }

    #[must_use]
    pub const fn summary(&self) -> Option<&UntrustedText> {
        self.summary.as_ref()
    }

    #[must_use]
    pub const fn body(&self) -> Option<&UntrustedText> {
        self.body.as_ref()
    }
}

#[must_use]
pub fn prepare_news_for_ai(
    item: &NewsItem,
    transient_body: Option<&UntrustedText>,
    settings: &SourceAiBodyReadSettings,
) -> AiNewsInput {
    let collected = item.collected();
    let source = match collected.source() {
        InformationSource::News(news_source) => news_source.clone(),
        InformationSource::Web | InformationSource::Disclosure(_) => NewsSource::GoogleNews,
    };
    let policy = settings.policy_for(&source);
    let (summary, body) = match policy {
        AiBodyReadPolicy::TitleOnly => (None, None),
        AiBodyReadPolicy::TitleAndSummary => (collected.body().cloned(), None),
        AiBodyReadPolicy::BodyAllowed => (collected.body().cloned(), transient_body.cloned()),
    };
    AiNewsInput {
        source,
        url: collected.url().clone(),
        title: collected.title().clone(),
        summary,
        body,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AiBodyReadPolicy, AiBodyReadSettingError, AiBodyReadToggle, SourceAiBodyReadSettings,
        prepare_news_for_ai,
    };
    use crate::{NewsItem, NewsSource, NormalizedUrl, UntrustedText};

    fn sample_item(source: NewsSource) -> NewsItem {
        NewsItem::new(
            source,
            NormalizedUrl::try_new("https://www.hankyung.com/article/1").expect("url"),
            UntrustedText::try_new("기사 제목").expect("title"),
            Some(UntrustedText::try_new("RSS 요약").expect("summary")),
        )
    }

    #[test]
    fn default_settings_disable_body_read_for_every_source_and_lock_yonhap_and_google() {
        let mut settings = SourceAiBodyReadSettings::all_disabled();
        for source in [
            NewsSource::Hankyung,
            NewsSource::Maekyung,
            NewsSource::Yonhap,
            NewsSource::Edaily,
            NewsSource::BankOfKorea,
            NewsSource::FederalReserve,
            NewsSource::GoogleNews,
            NewsSource::YahooFinance,
        ] {
            assert_eq!(settings.toggle_for(&source), AiBodyReadToggle::Disabled);
        }
        assert_eq!(
            settings.set(&NewsSource::Yonhap, AiBodyReadToggle::Enabled),
            Err(AiBodyReadSettingError::SourceLockedOff(NewsSource::Yonhap))
        );
        assert_eq!(
            settings.set(&NewsSource::GoogleNews, AiBodyReadToggle::Enabled),
            Err(AiBodyReadSettingError::SourceLockedOff(
                NewsSource::GoogleNews
            ))
        );
        assert_eq!(
            settings.set(&NewsSource::Yonhap, AiBodyReadToggle::Disabled),
            Ok(())
        );
        assert_eq!(
            settings.policy_for(&NewsSource::Yonhap),
            AiBodyReadPolicy::TitleAndSummary
        );
        assert_eq!(
            settings.policy_for(&NewsSource::GoogleNews),
            AiBodyReadPolicy::TitleOnly
        );
    }

    #[test]
    fn disabled_sources_pass_only_title_and_summary_while_enabled_sources_pass_transient_body() {
        let mut settings = SourceAiBodyReadSettings::all_disabled();
        let body = UntrustedText::try_new("기사 전체 본문").expect("body");

        let hk_item = sample_item(NewsSource::Hankyung);
        let disabled_input = prepare_news_for_ai(&hk_item, Some(&body), &settings);
        assert_eq!(disabled_input.source(), &NewsSource::Hankyung);
        assert_eq!(disabled_input.title().as_untrusted_str(), "기사 제목");
        assert_eq!(
            disabled_input
                .summary()
                .expect("summary")
                .as_untrusted_str(),
            "RSS 요약"
        );
        assert_eq!(disabled_input.body(), None);

        settings
            .set(&NewsSource::Hankyung, AiBodyReadToggle::Enabled)
            .expect("enable");
        let enabled_input = prepare_news_for_ai(&hk_item, Some(&body), &settings);
        assert_eq!(
            enabled_input.body().expect("body").as_untrusted_str(),
            "기사 전체 본문"
        );

        let yna_item = sample_item(NewsSource::Yonhap);
        let yna_input = prepare_news_for_ai(&yna_item, Some(&body), &settings);
        assert_eq!(yna_input.body(), None);
        assert_eq!(
            yna_input.summary().expect("summary").as_untrusted_str(),
            "RSS 요약"
        );

        let google_item = sample_item(NewsSource::GoogleNews);
        let google_input = prepare_news_for_ai(&google_item, Some(&body), &settings);
        assert_eq!(google_input.summary(), None);
        assert_eq!(google_input.body(), None);
    }
}
