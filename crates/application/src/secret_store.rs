use secrecy::{ExposeSecret, SecretSlice};

pub struct SecretValue(SecretSlice<u8>);

impl std::fmt::Debug for SecretValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretValue(<redacted>)")
    }
}

impl SecretValue {
    #[must_use]
    pub fn new(value: Vec<u8>) -> Self {
        Self(SecretSlice::from(value))
    }
    #[must_use]
    pub fn as_exposed_secret(&self) -> &[u8] {
        self.0.expose_secret()
    }
}
