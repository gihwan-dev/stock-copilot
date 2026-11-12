use std::ffi::{CStr, CString, OsStr, OsString};
use std::path::{Path, PathBuf};

pub struct Symbol(String);

impl Symbol {
    pub fn try_new(raw: &str) -> Result<Self, SymbolError> {
        Text::check(raw).map(|text| Self(text.owned()))
    }

    pub fn as_text(&self) -> &String {
        &self.0
    }
}

pub struct Location(PathBuf);

impl Location {
    pub fn new(path: &Path) -> Self {
        Self(path.to_path_buf())
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

pub struct Variable(OsString);

impl Variable {
    pub fn new(value: &OsStr) -> Self {
        Self(value.to_os_string())
    }
}

pub struct Native(CString);

impl Native {
    pub fn new(value: &CStr) -> Self {
        Self(value.to_owned())
    }
}

pub struct Payload(Vec<u8>);

impl Payload {
    pub fn new(bytes: &[u8]) -> Self {
        Self(bytes.to_vec())
    }
}

pub struct Count(u64);

impl Count {
    pub fn as_ref_value(&self) -> &u64 {
        &self.0
    }
}
