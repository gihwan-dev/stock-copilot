use crate::{CollectedItem, InformationSource, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NormalizedUrl(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlError {
    InvalidFormat,
}

impl NormalizedUrl {
    pub fn try_new(value: &str) -> Result<Self, UrlError> {
        if value.len() > 8192
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || b":/?#[]@!$&'()*+,;=%-._~".contains(&byte)
            })
        {
            return Err(UrlError::InvalidFormat);
        }
        let (scheme, rest) = value.split_once("://").ok_or(UrlError::InvalidFormat)?;
        let normalized_scheme = scheme.to_ascii_lowercase();
        if !["http", "https"].contains(&normalized_scheme.as_str()) {
            return Err(UrlError::InvalidFormat);
        }
        let host = rest
            .chars()
            .take_while(|character| !['/', '?', '#'].contains(character))
            .collect::<String>();
        let authority = UrlAuthority::try_new(&host)?;
        let suffix = UrlSuffix::try_new(rest.trim_start_matches(host.as_str()))?;
        Ok(Self(format!(
            "{normalized_scheme}://{}{}",
            authority.0, suffix.0
        )))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
struct UrlAuthority(String);

impl UrlAuthority {
    fn try_new(value: &str) -> Result<Self, UrlError> {
        if value.len() > 253
            || value
                .rsplit('.')
                .next()
                .is_some_and(|label| label.bytes().all(|byte| byte.is_ascii_digit()))
            || value.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            })
        {
            return Err(UrlError::InvalidFormat);
        }
        Ok(Self(value.to_ascii_lowercase()))
    }
}

#[derive(Debug)]
struct UrlSuffix(String);

impl UrlSuffix {
    fn try_new(value: &str) -> Result<Self, UrlError> {
        if value.split('%').skip(1).any(|part| {
            part.len() < 2 || !part.bytes().take(2).all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(UrlError::InvalidFormat);
        }
        let (before_fragment, fragment) = value
            .split_once('#')
            .map_or((value, None), |(head, tail)| (head, Some(tail)));
        let (path, query) = before_fragment
            .split_once('?')
            .map_or((before_fragment, None), |(head, tail)| (head, Some(tail)));
        if path.split('/').any(|segment| {
            [".", ".."].contains(&segment.to_ascii_lowercase().replace("%2e", ".").as_str())
        }) {
            return Err(UrlError::InvalidFormat);
        }
        let mut normalized = if path.is_empty() {
            String::from("/")
        } else {
            path.to_owned()
        };
        if let Some(query_text) = query {
            let retained = query_text
                .split('&')
                .filter(|part| {
                    let name = part.split_once('=').map_or(*part, |(name, _)| name);
                    ![
                        "utm_source",
                        "utm_medium",
                        "utm_campaign",
                        "utm_term",
                        "utm_content",
                    ]
                    .contains(&name)
                })
                .collect::<Vec<_>>();
            if !retained.is_empty() {
                normalized.push('?');
                normalized.push_str(&retained.join("&"));
            }
        }
        if let Some(fragment_text) = fragment {
            normalized.push('#');
            normalized.push_str(fragment_text);
        }
        Ok(Self(normalized))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewsSource {
    Hankyung,
    Maekyung,
    Yonhap,
    Edaily,
    BankOfKorea,
    FederalReserve,
    GoogleNews,
    YahooFinance,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsItem(CollectedItem);

impl NewsItem {
    #[must_use]
    pub const fn new(
        source: NewsSource,
        url: NormalizedUrl,
        title: UntrustedText,
        body: Option<UntrustedText>,
    ) -> Self {
        Self(CollectedItem::new(
            InformationSource::News(source),
            url,
            title,
            body,
        ))
    }

    #[must_use]
    pub const fn collected(&self) -> &CollectedItem {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{NewsItem, NewsSource, NormalizedUrl, UrlError};
    use crate::UntrustedText;

    #[test]
    fn normalization_preserves_article_identity_and_removes_bounded_tracking_family() {
        let expected = "https://www.mk.co.kr/news/stock/1?id=7&ref=keep#section";
        assert_eq!(
            NormalizedUrl::try_new(
                "https://www.mk.co.kr/news/stock/1?id=7&utm_source=rss&ref=keep#section"
            )
            .expect("주소")
            .as_str(),
            expected
        );
        for other in [
            "http://www.mk.co.kr/news/stock/1?id=7&ref=keep#section",
            "https://m.mk.co.kr/news/stock/1?id=7&ref=keep#section",
            "https://www.mk.co.kr/news/stock/2?id=7&ref=keep#section",
            "https://www.mk.co.kr/news/stock/1?id=8&ref=keep#section",
            "https://www.mk.co.kr/news/stock/1?ref=keep&id=7#section",
            "https://www.mk.co.kr/news/stock/1?id=7&ref=keep#other",
        ] {
            assert_ne!(
                NormalizedUrl::try_new(other).expect("다른 주소").as_str(),
                expected
            );
        }
    }

    #[test]
    fn normalization_properties_hold_over_article_ids_and_tracking_values() {
        for id in 0_u16..128 {
            let expected = format!("https://example.com/article?id={id}&lang=ko");
            let key = NormalizedUrl::try_new(&expected).expect("기사 키");
            let variants = [
                "utm_source",
                "utm_medium",
                "utm_campaign",
                "utm_term",
                "utm_content",
            ]
            .into_iter()
            .flat_map(|name| ["rss", "", "a%26id%3D999", "%ED%95%9C"].map(|value| (name, value)));
            for (name, value) in variants {
                let input = format!("{expected}&{name}={value}");
                let normalized = NormalizedUrl::try_new(&input).expect("추적 주소");
                assert_eq!(normalized, key);
                assert_eq!(
                    NormalizedUrl::try_new(normalized.as_str()).expect("멱등"),
                    normalized
                );
            }
            let distinct = format!("https://example.com/article?id={id}&lang=en");
            assert_ne!(NormalizedUrl::try_new(&distinct).expect("다른 기사"), key);
        }
    }

    #[test]
    fn urls_reject_unsafe_or_unbounded_inputs() {
        for input in [
            "",
            "javascript:alert(1)",
            "file:///tmp/a",
            "https://user:pass@example.com/a",
            "https://example.com/
article",
            "https://example.com/ article",
            r"https://example.com/\article",
            "https:///",
        ] {
            assert_eq!(NormalizedUrl::try_new(input), Err(UrlError::InvalidFormat));
        }
        assert_eq!(
            NormalizedUrl::try_new(&"a".repeat(8193)),
            Err(UrlError::InvalidFormat)
        );
    }

    #[test]
    fn news_keeps_source_url_and_untrusted_text() {
        let url = NormalizedUrl::try_new("https://example.com/a").expect("주소");
        let title = UntrustedText::try_new("제목").expect("제목");
        let item = NewsItem::new(NewsSource::Hankyung, url.clone(), title.clone(), None);
        assert_eq!(
            item.collected().source(),
            &crate::InformationSource::News(NewsSource::Hankyung)
        );
        assert_eq!(item.collected().url(), &url);
        assert_eq!(item.collected().title(), &title);
        assert_eq!(item.collected().body(), None);
    }
    #[test]
    fn url_boundaries_and_ambiguous_syntax_are_explicit() {
        let base = "https://example.com/";
        let boundary = format!("{base}{}", "a".repeat(8172));
        assert_eq!(boundary.len(), 8192);
        assert_eq!(
            NormalizedUrl::try_new(&boundary).expect("상한").as_str(),
            boundary
        );
        assert_eq!(
            NormalizedUrl::try_new(&format!("{boundary}a")),
            Err(UrlError::InvalidFormat)
        );
        for input in [
            "example.com/a",
            "ftp://example.com/a",
            "https://",
            "https://.com/a",
            "https://a..com/a",
            "https://-a.com/a",
            "https://a-.com/a",
            "https://a_.com/a",
            "https://example.com:443/a",
            "https://[::1]/",
            "https://127.1/a",
            "https://example.com/%",
            "https://example.com/%A",
            "https://example.com/%GG",
            "https://example.com/../a",
            "https://example.com/%2e/a",
            "https://example.com/.%2e/a",
            "https://example.com/<article>",
            "https://example.com/한글",
        ] {
            assert_eq!(
                NormalizedUrl::try_new(input),
                Err(UrlError::InvalidFormat),
                "{input}"
            );
        }
    }

    #[test]
    fn dns_host_length_boundaries_are_checked() {
        let label = "a".repeat(63);
        let accepted = format!("https://{label}.com/");
        assert_eq!(
            NormalizedUrl::try_new(&accepted)
                .expect("63자리 호스트")
                .as_str(),
            accepted
        );
        assert_eq!(
            NormalizedUrl::try_new(&format!("https://{}.com/", "a".repeat(64))),
            Err(UrlError::InvalidFormat)
        );
        let host = [
            "a".repeat(63),
            "b".repeat(63),
            "c".repeat(63),
            "d".repeat(61),
        ]
        .join(".");
        let accepted_host = format!("https://{host}/");
        assert_eq!(
            NormalizedUrl::try_new(&accepted_host)
                .expect("253자리 호스트")
                .as_str(),
            accepted_host
        );
        assert_eq!(
            NormalizedUrl::try_new(&format!("https://{host}d/")),
            Err(UrlError::InvalidFormat)
        );
    }

    #[test]
    fn normalization_keeps_unknown_query_keys_and_encoded_separators() {
        for (input, expected) in [
            ("HTTPS://Example.COM", "https://example.com/"),
            (
                "http://example.com?utm_source=rss#part",
                "http://example.com/#part",
            ),
            (
                "https://example.com/a?utm_source=rss",
                "https://example.com/a",
            ),
            (
                "https://example.com/a?utm_source=rss&id=1",
                "https://example.com/a?id=1",
            ),
            (
                "https://example.com/a?utm_source=rss&utm_medium=feed",
                "https://example.com/a",
            ),
            ("https://example.com/a?utm_source", "https://example.com/a"),
            ("https://example.com/a?", "https://example.com/a?"),
            (
                "https://example.com/a?id=1&id=2",
                "https://example.com/a?id=1&id=2",
            ),
            (
                "https://example.com/a?ref=rss&source=1&UTM_source=a&%75tm_source=b",
                "https://example.com/a?ref=rss&source=1&UTM_source=a&%75tm_source=b",
            ),
            (
                "https://example.com/a%2Fb?x=a%26b%3Dc#utm_source=rss",
                "https://example.com/a%2Fb?x=a%26b%3Dc#utm_source=rss",
            ),
        ] {
            let url = NormalizedUrl::try_new(input).expect("정규화");
            assert_eq!(url.as_str(), expected);
            assert_eq!(NormalizedUrl::try_new(url.as_str()).expect("멱등"), url);
        }
    }
}
