use crate::{AccessGroup, KeychainKind, KeychainProfile, ServiceName};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeychainItem {
    profile: KeychainProfile,
    account: &'static str,
}

impl KeychainItem {
    pub(crate) const fn new(profile: KeychainProfile, account: &'static str) -> Self {
        Self { profile, account }
    }

    #[must_use]
    pub const fn service(&self) -> &ServiceName {
        self.profile.service()
    }

    #[must_use]
    pub const fn account(&self) -> &'static str {
        self.account
    }

    #[must_use]
    pub const fn kind(&self) -> KeychainKind {
        self.profile.kind()
    }

    #[must_use]
    pub const fn access_group(&self) -> &AccessGroup {
        self.profile.access_group()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainFailure {
    MissingEntitlement,
    Locked,
    Denied,
    Malformed,
    Unavailable,
}

pub trait KeychainBackend: Send + Sync {
    fn read(&self, item: &KeychainItem) -> Result<Option<Vec<u8>>, KeychainFailure>;
    fn write(&self, item: &KeychainItem, value: &[u8]) -> Result<(), KeychainFailure>;
    fn delete(&self, item: &KeychainItem) -> Result<(), KeychainFailure>;
}
