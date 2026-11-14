mod internal; use internal::Alias as Local; pub fn leak() -> Local { loop {} }
