pub struct Name(String);

impl Name {
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
}
