use crate::{CollectedItem, InformationSource, NormalizedUrl, UntrustedText};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DisclosureSource {
    Kind,
    Dart,
    SecEdgar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptNoError {
    InvalidFormat,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DartReceiptNo(String);

impl DartReceiptNo {
    pub fn try_new(value: &str) -> Result<Self, ReceiptNoError> {
        if value.len() != 14 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
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
pub struct KindReceiptNo(String);

impl KindReceiptNo {
    pub fn try_new(value: &str) -> Result<Self, ReceiptNoError> {
        if value.len() != 14 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
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
pub struct SecEdgarReceiptNo(String);

impl SecEdgarReceiptNo {
    pub fn try_new(value: &str) -> Result<Self, ReceiptNoError> {
        if value.len() != 20
            || !value.bytes().enumerate().all(|(index, byte)| {
                if [10, 13].contains(&index) {
                    byte == b'-'
                } else {
                    byte.is_ascii_digit()
                }
            })
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
pub enum ReceiptNo {
    Dart(DartReceiptNo),
    Kind(KindReceiptNo),
    SecEdgar(SecEdgarReceiptNo),
}

impl ReceiptNo {
    #[must_use]
    pub const fn source(&self) -> DisclosureSource {
        match self {
            Self::Dart(_) => DisclosureSource::Dart,
            Self::Kind(_) => DisclosureSource::Kind,
            Self::SecEdgar(_) => DisclosureSource::SecEdgar,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DisclosureKey(ReceiptNo);

impl DisclosureKey {
    #[must_use]
    pub const fn source(&self) -> DisclosureSource {
        self.0.source()
    }

    #[must_use]
    pub const fn receipt_no(&self) -> &ReceiptNo {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Disclosure {
    key: DisclosureKey,
    collected: CollectedItem,
}

impl Disclosure {
    #[must_use]
    pub const fn new(
        receipt_no: ReceiptNo,
        url: NormalizedUrl,
        title: UntrustedText,
        body: Option<UntrustedText>,
    ) -> Self {
        let key = DisclosureKey(receipt_no);
        let source = key.source();
        Self {
            key,
            collected: CollectedItem::from_source(
                InformationSource::Disclosure(source),
                url,
                title,
                body,
            ),
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
    use super::{
        DartReceiptNo, Disclosure, DisclosureSource, KindReceiptNo, ReceiptNo, ReceiptNoError,
        SecEdgarReceiptNo,
    };
    use crate::{InformationSource, NormalizedUrl, UntrustedText};

    #[test]
    fn disclosure_identity_is_source_and_receipt_number() {
        let dart = ReceiptNo::Dart(DartReceiptNo::try_new("20261113000001").expect("DART"));
        let kind = ReceiptNo::Kind(KindReceiptNo::try_new("20261113000001").expect("KIND"));
        let url = NormalizedUrl::try_new("https://example.com/1").expect("주소");
        let title = UntrustedText::try_new("공시").expect("제목");
        let first = Disclosure::new(dart.clone(), url.clone(), title.clone(), None);
        let same = Disclosure::new(
            dart.clone(),
            url.clone(),
            UntrustedText::try_new("수정").expect("제목"),
            None,
        );
        let other = Disclosure::new(kind, url, title, None);
        assert_eq!(first.key(), same.key());
        assert_ne!(first.key(), other.key());
        assert_eq!(first.key().receipt_no(), &dart);
        assert_eq!(first.key().source(), DisclosureSource::Dart);
    }

    #[test]
    fn every_disclosure_source_survives_construction() {
        for (receipt, source) in [
            (
                ReceiptNo::Dart(DartReceiptNo::try_new("20261113000001").expect("DART")),
                DisclosureSource::Dart,
            ),
            (
                ReceiptNo::Kind(KindReceiptNo::try_new("20261113000001").expect("KIND")),
                DisclosureSource::Kind,
            ),
            (
                ReceiptNo::SecEdgar(
                    SecEdgarReceiptNo::try_new("0000320193-26-000001").expect("SEC"),
                ),
                DisclosureSource::SecEdgar,
            ),
        ] {
            let item = Disclosure::new(
                receipt,
                NormalizedUrl::try_new("https://example.com/1").expect("주소"),
                UntrustedText::try_new("공시").expect("제목"),
                None,
            );
            assert_eq!(item.key().source(), source);
            assert_eq!(
                item.collected().source(),
                &InformationSource::Disclosure(source)
            );
        }
    }

    #[test]
    fn domestic_receipts_require_exactly_fourteen_ascii_digits() {
        for valid in ["00000000000000", "20261113000001", "99999999999999"] {
            assert_eq!(DartReceiptNo::try_new(valid).expect("DART").as_str(), valid);
            assert_eq!(KindReceiptNo::try_new(valid).expect("KIND").as_str(), valid);
        }
        for value in [
            "",
            "2026111300000",
            "202611130000001",
            "2026111300000A",
            "２０２６１１１３０００００１",
            "2026&x=1#",
            "../../x",
            "0000320193-26-000001",
            "2026111300000\n",
        ] {
            assert_eq!(
                DartReceiptNo::try_new(value),
                Err(ReceiptNoError::InvalidFormat)
            );
            assert_eq!(
                KindReceiptNo::try_new(value),
                Err(ReceiptNoError::InvalidFormat)
            );
        }
        let huge = "1".repeat(1_000_000);
        assert_eq!(
            DartReceiptNo::try_new(&huge),
            Err(ReceiptNoError::InvalidFormat)
        );
        assert_eq!(
            KindReceiptNo::try_new(&huge),
            Err(ReceiptNoError::InvalidFormat)
        );
    }

    #[test]
    fn edgar_receipts_accept_only_the_conservative_accession_shape() {
        let valid = "0000320193-26-000001";
        assert_eq!(
            SecEdgarReceiptNo::try_new(valid).expect("SEC").as_str(),
            valid
        );
        for value in [
            "",
            "0000320193-26-00000",
            "0000320193-26-0000011",
            "0000320193_26-000001",
            "0000320193-26_000001",
            "A000320193-26-000001",
            "0000320193-AA-000001",
            "0000320193-26-00000A",
            "20261113000001",
            "2026&x=1#",
            "../../x",
        ] {
            assert_eq!(
                SecEdgarReceiptNo::try_new(value),
                Err(ReceiptNoError::InvalidFormat)
            );
        }
        assert_eq!(
            SecEdgarReceiptNo::try_new(&"1".repeat(1_000_000)),
            Err(ReceiptNoError::InvalidFormat)
        );
    }
}
