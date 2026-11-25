use application::SecretKey;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainKind {
    DataProtection,
    Login,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildKind {
    Development,
    Release,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceName(&'static str);

impl ServiceName {
    pub const PRODUCT: Self = Self("dev.stockcopilot.desktop");
    pub const DEVELOPMENT: Self = Self("dev.stockcopilot.desktop.development-fake-keys");

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessGroup {
    EntitlementDefault,
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeychainProfile {
    service: ServiceName,
    kind: KeychainKind,
    access_group: AccessGroup,
}

impl KeychainProfile {
    #[must_use]
    pub const fn product() -> Self {
        Self {
            service: ServiceName::PRODUCT,
            kind: KeychainKind::DataProtection,
            access_group: AccessGroup::EntitlementDefault,
        }
    }

    #[must_use]
    pub const fn product_login() -> Self {
        Self {
            service: ServiceName::PRODUCT,
            kind: KeychainKind::Login,
            access_group: AccessGroup::EntitlementDefault,
        }
    }

    #[must_use]
    pub const fn development() -> Self {
        Self {
            service: ServiceName::DEVELOPMENT,
            kind: KeychainKind::Login,
            access_group: AccessGroup::EntitlementDefault,
        }
    }

    #[must_use]
    pub const fn for_build(build: BuildKind) -> Self {
        match build {
            BuildKind::Development => Self::development(),
            BuildKind::Release => Self::product(),
        }
    }

    #[must_use]
    pub fn with_access_group(mut self, group: AccessGroup) -> Self {
        self.access_group = group;
        self
    }

    #[must_use]
    pub const fn service(&self) -> &ServiceName {
        &self.service
    }

    #[must_use]
    pub const fn kind(&self) -> KeychainKind {
        self.kind
    }

    #[must_use]
    pub const fn access_group(&self) -> &AccessGroup {
        &self.access_group
    }

    #[must_use]
    pub fn item(&self, key: SecretKey) -> crate::KeychainItem {
        crate::KeychainItem::new(self.clone(), account_name(key))
    }
}

const fn account_name(key: SecretKey) -> &'static str {
    match key {
        SecretKey::TossClientId => "toss.client-id",
        SecretKey::TossClientSecret => "toss.client-secret",
        SecretKey::TossAccessToken => "toss.access-token",
        SecretKey::DatabaseEncryptionKey => "database.encryption-key",
        SecretKey::AuditHmacKey => "audit.hmac-key",
        SecretKey::DartApiKey => "dart.api-key",
        SecretKey::FredApiKey => "fred.api-key",
    }
}
