use std::fmt;
use std::io;
use std::path::PathBuf;

#[derive(Debug)]
pub(crate) enum CheckFailure {
    Io(PathBuf, io::Error),
    Manifest(String, toml::de::Error),
    Metadata(cargo_metadata::Error),
    Allowlist(String),
    OutsideRoot(PathBuf),
}

impl fmt::Display for CheckFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(path, error) => write!(f, "{}: {error}", path.display()),
            Self::Manifest(path, error) => write!(f, "{path}: {error}"),
            Self::Metadata(error) => write!(f, "cargo metadata: {error}"),
            Self::Allowlist(message) => f.write_str(message),
            Self::OutsideRoot(path) => write!(f, "{}: 저장소 밖의 경로", path.display()),
        }
    }
}
