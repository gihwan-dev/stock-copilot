use crate::{CollectedItem, InformationSource, NormalizedUrl, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DisclosureSource {
    Kind,
    Dart,
    SecEdgar,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReceiptNo(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptNoError {
    InvalidFormat,
}

impl ReceiptNo {
    pub fn try_new(value: &str) -> Result<Self, ReceiptNoError> {
        if value.is_empty()
            || value.len() > 64
            || !value.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ReceiptNoError::InvalidFormat);
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DisclosureKey {
    source: DisclosureSource,
    receipt_no: ReceiptNo,
}

impl DisclosureKey {
    #[must_use]
    pub const fn source(&self) -> &DisclosureSource {
        &self.source
    }

    #[must_use]
    pub const fn receipt_no(&self) -> &ReceiptNo {
        &self.receipt_no
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    key: DisclosureKey,
    collected: CollectedItem,
}

impl Disclosure {
    #[must_use]
    pub fn new(
        source: DisclosureSource,
        receipt_no: ReceiptNo,
        url: NormalizedUrl,
        title: UntrustedText,
        body: Option<UntrustedText>,
    ) -> Self {
        Self {
            key: DisclosureKey {
                source: source.clone(),
                receipt_no,
            },
            collected: CollectedItem::new(InformationSource::Disclosure(source), url, title, body),
        }
    }

    #[must_use]
    pub const fn key(&self) -> &DisclosureKey {
        &self.key
    }

    #[must_use]
    pub const fn collected(&self) -> &CollectedItem {
        &self.collected
    }
}

#[cfg(test)]
mod tests {
    use super::{Disclosure, DisclosureSource, ReceiptNo, ReceiptNoError};
    use crate::{InformationSource, NormalizedUrl, UntrustedText};

    #[test]
    fn disclosure_identity_is_source_and_receipt_number() {
        let receipt = ReceiptNo::try_new("20261113000001").expect("접수번호");
        let url = NormalizedUrl::try_new("https://example.com/1").expect("주소");
        let title = UntrustedText::try_new("공시").expect("제목");
        let first = Disclosure::new(
            DisclosureSource::Dart,
            receipt.clone(),
            url.clone(),
            title.clone(),
            None,
        );
        let same = Disclosure::new(
            DisclosureSource::Dart,
            receipt.clone(),
            url.clone(),
            UntrustedText::try_new("수정 제목").expect("제목"),
            None,
        );
        let other = Disclosure::new(DisclosureSource::Kind, receipt, url, title, None);
        assert_eq!(first.key(), same.key());
        assert_ne!(first.key(), other.key());
        assert_eq!(first.key().source(), &DisclosureSource::Dart);
        assert_eq!(first.key().receipt_no().as_str(), "20261113000001");
        assert_eq!(
            first.collected().source(),
            &InformationSource::Disclosure(DisclosureSource::Dart)
        );
    }

    #[test]
    fn receipt_numbers_preserve_external_formats_and_enforce_bounds() {
        for value in ["20261113000001", "0000320193-26-000001", "A"] {
            assert_eq!(
                ReceiptNo::try_new(value).expect("외부 접수번호").as_str(),
                value
            );
        }
        let boundary = "a".repeat(64);
        assert_eq!(
            ReceiptNo::try_new(&boundary).expect("64바이트").as_str(),
            boundary
        );
        for value in [
            String::new(),
            "a b".to_owned(),
            "a
"
            .to_owned(),
            "한글".to_owned(),
            "a".repeat(65),
        ] {
            assert_eq!(
                ReceiptNo::try_new(&value),
                Err(ReceiptNoError::InvalidFormat)
            );
        }
    }
}
