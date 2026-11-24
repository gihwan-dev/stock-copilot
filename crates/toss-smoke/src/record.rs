use std::fmt;
use std::fs::{self, File, OpenOptions, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use application::SecretValue;
use serde_json::{Map, Value};

const FILE_MODE: u32 = 0o600;
const REDACTED_MARKER: &str = "[REDACTED]";
const FORMAT_VERSION: u64 = 1;
const OPENAPI_VERSION: &str = "1.2.28";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordError {
    InvalidPath,
    MissingParent,
    AlreadyExists,
    InsideRepository,
    IoError,
    SecretLeakDetected,
}

impl RecordError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidPath => "녹화 파일 경로가 올바른 새 파일 이름이 아닙니다.",
            Self::MissingParent => "녹화 파일을 둘 상위 폴더가 존재하지 않습니다.",
            Self::AlreadyExists => "녹화 파일 경로가 이미 존재하거나 심볼릭 링크입니다.",
            Self::InsideRepository => {
                "녹화 파일 경로가 저장소 내부(또는 Git/Cargo 작업 영역 안)를 가리켜 거부했습니다. 저장소 밖 경로만 허용합니다."
            }
            Self::IoError => "녹화 파일을 생성하거나 기록하는 중 입출력 오류가 발생했습니다.",
            Self::SecretLeakDetected => {
                "녹화 출력 직전 검사에서 키 또는 토큰 바이트가 감지되어 파일 기록을 중단했습니다."
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedRateLimit {
    pub limit: Option<String>,
    pub remaining: Option<String>,
    pub reset: Option<String>,
    pub retry_after: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedStep {
    pub step: &'static str,
    pub path: &'static str,
    pub status: Option<u16>,
    pub outcome: &'static str,
    pub schema_status: &'static str,
    pub rate_limit: Option<RecordedRateLimit>,
    pub body: Option<Value>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum WriteState {
    Pending,
    Committed,
}

pub struct RecordHandle {
    path: PathBuf,
    file: Option<File>,
    state: WriteState,
}

impl fmt::Debug for RecordHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RecordHandle")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl Drop for RecordHandle {
    fn drop(&mut self) {
        self.file = None;
        match self.state {
            WriteState::Pending => match fs::remove_file(&self.path) {
                Ok(()) | Err(_) => {}
            },
            WriteState::Committed => {}
        }
    }
}

fn has_normal_file_name(path: &Path) -> Option<&std::ffi::OsStr> {
    match path.components().next_back() {
        Some(Component::Normal(name)) if !name.is_empty() => Some(name),
        Some(
            Component::Normal(_)
            | Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir,
        )
        | None => None,
    }
}

fn ancestor_is_repository(directory: &Path) -> bool {
    directory.ancestors().any(|ancestor| {
        fs::symlink_metadata(ancestor.join(".git")).is_ok()
            || fs::symlink_metadata(ancestor.join("Cargo.toml")).is_ok()
    })
}

fn canonical_or_owned(path: &Path) -> PathBuf {
    match fs::canonicalize(path) {
        Ok(canonical) => canonical,
        Err(_) => path.to_path_buf(),
    }
}

fn is_inside_forbidden_roots(
    canonical_parent: &Path,
    resolved_target: &Path,
    forbidden_roots: &[PathBuf],
) -> bool {
    forbidden_roots.iter().any(|root| {
        let canonical = canonical_or_owned(root);
        canonical_parent.starts_with(&canonical) || resolved_target.starts_with(&canonical)
    })
}

fn push_worktree_roots(ancestor: &Path, git_entry: &Path, roots: &mut Vec<PathBuf>) {
    let Ok(text) = fs::read_to_string(git_entry) else {
        return;
    };
    let Some(raw_gitdir) = text.trim().strip_prefix("gitdir:") else {
        return;
    };
    let gitdir = PathBuf::from(raw_gitdir.trim());
    let resolved = if gitdir.is_absolute() {
        gitdir
    } else {
        ancestor.join(gitdir)
    };
    let Ok(canon_gitdir) = fs::canonicalize(&resolved) else {
        return;
    };
    roots.push(canon_gitdir.clone());
    let main_repo = canon_gitdir
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent);
    if let Some(repo) = main_repo {
        roots.push(repo.to_path_buf());
    }
}

#[must_use]
pub fn detect_repo_roots(start: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let base = canonical_or_owned(start);
    for ancestor in base.ancestors() {
        let git_entry = ancestor.join(".git");
        let has_git = fs::symlink_metadata(&git_entry).is_ok();
        let has_cargo = fs::symlink_metadata(ancestor.join("Cargo.toml")).is_ok();
        if has_git || has_cargo {
            roots.push(ancestor.to_path_buf());
        }
        if has_git {
            push_worktree_roots(ancestor, &git_entry, &mut roots);
        }
    }
    roots
}

pub fn prepare_recording(
    path: &Path,
    forbidden_roots: &[PathBuf],
) -> Result<RecordHandle, RecordError> {
    let file_name = has_normal_file_name(path).ok_or(RecordError::InvalidPath)?;
    if fs::symlink_metadata(path).is_ok() {
        return Err(RecordError::AlreadyExists);
    }
    let parent = match path.parent() {
        Some(dir) if !dir.as_os_str().is_empty() => dir,
        Some(_) | None => Path::new("."),
    };
    let canonical_parent = fs::canonicalize(parent).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            RecordError::MissingParent
        } else {
            RecordError::IoError
        }
    })?;
    if !canonical_parent.is_dir() {
        return Err(RecordError::InvalidPath);
    }
    let resolved_target = canonical_parent.join(file_name);
    if is_inside_forbidden_roots(&canonical_parent, &resolved_target, forbidden_roots)
        || ancestor_is_repository(&canonical_parent)
    {
        return Err(RecordError::InsideRepository);
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(FILE_MODE)
        .open(&resolved_target)
        .map_err(|error| {
            if error.kind() == io::ErrorKind::AlreadyExists {
                RecordError::AlreadyExists
            } else {
                RecordError::IoError
            }
        })?;
    file.set_permissions(Permissions::from_mode(FILE_MODE))
        .map_err(|_error| RecordError::IoError)?;
    Ok(RecordHandle {
        path: resolved_target,
        file: Some(file),
        state: WriteState::Pending,
    })
}

fn hex_digit_upper(nibble: u8) -> u8 {
    if nibble < 10 {
        b'0'.saturating_add(nibble)
    } else {
        b'A'.saturating_add(nibble.saturating_sub(10))
    }
}

fn hex_digit_lower(nibble: u8) -> u8 {
    if nibble < 10 {
        b'0'.saturating_add(nibble)
    } else {
        b'a'.saturating_add(nibble.saturating_sub(10))
    }
}

fn percent_encode(bytes: &[u8], digit_fn: fn(u8) -> u8) -> SecretValue {
    let mut encoded = Vec::with_capacity(bytes.len().saturating_mul(3));
    for byte in bytes {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(*byte);
            }
            b' ' => encoded.push(b'+'),
            _ => {
                encoded.push(b'%');
                encoded.push(digit_fn(byte >> 4_u32));
                encoded.push(digit_fn(byte & 15));
            }
        }
    }
    SecretValue::new(encoded)
}

fn push_unique_variant(variants: &mut Vec<SecretValue>, candidate: SecretValue) {
    if candidate.as_exposed_secret().is_empty() {
        return;
    }
    let duplicate = variants
        .iter()
        .any(|existing| existing.as_exposed_secret() == candidate.as_exposed_secret());
    if !duplicate {
        variants.push(candidate);
    }
}

fn build_secret_variants(secrets: &[&SecretValue]) -> Vec<SecretValue> {
    let mut variants = Vec::new();
    for secret in secrets {
        let raw = secret.as_exposed_secret();
        if raw.is_empty() {
            continue;
        }
        push_unique_variant(&mut variants, SecretValue::new(raw.to_vec()));
        push_unique_variant(&mut variants, percent_encode(raw, hex_digit_upper));
        push_unique_variant(&mut variants, percent_encode(raw, hex_digit_lower));
        if let Ok(text) = std::str::from_utf8(raw)
            && let Ok(quoted) = serde_json::to_string(text)
            && let Some(inner) = quoted
                .strip_prefix('"')
                .and_then(|value| value.strip_suffix('"'))
        {
            push_unique_variant(&mut variants, SecretValue::new(inner.as_bytes().to_vec()));
        }
    }
    variants
}

fn scrub_string(input: &str, secret_texts: &[&str]) -> String {
    let mut cleaned = input.to_owned();
    for secret in secret_texts {
        if !secret.is_empty() && cleaned.contains(secret) {
            cleaned = cleaned.replace(secret, REDACTED_MARKER);
        }
    }
    cleaned
}

fn scrub_optional(input: Option<&str>, secret_texts: &[&str]) -> Value {
    input.map_or(Value::Null, |text| {
        Value::String(scrub_string(text, secret_texts))
    })
}

fn scrub_value(value: Value, secret_texts: &[&str]) -> Value {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) => value,
        Value::String(text) => Value::String(scrub_string(&text, secret_texts)),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(|item| scrub_value(item, secret_texts))
                .collect(),
        ),
        Value::Object(entries) => {
            let mut scrubbed = Map::new();
            for (key, entry_value) in entries {
                let safe_key = scrub_string(&key, secret_texts);
                let safe_value = scrub_value(entry_value, secret_texts);
                scrubbed.insert(safe_key, safe_value);
            }
            Value::Object(scrubbed)
        }
    }
}

fn step_to_value(step: &RecordedStep, secret_texts: &[&str]) -> Value {
    let mut object = Map::new();
    object.insert(String::from("step"), Value::String(step.step.to_owned()));
    object.insert(String::from("path"), Value::String(step.path.to_owned()));
    object.insert(
        String::from("status"),
        step.status.map_or(Value::Null, Value::from),
    );
    object.insert(
        String::from("outcome"),
        Value::String(step.outcome.to_owned()),
    );
    object.insert(
        String::from("schemaStatus"),
        Value::String(step.schema_status.to_owned()),
    );
    let rate_limit_value = step.rate_limit.as_ref().map_or(Value::Null, |headers| {
        let mut rate = Map::new();
        rate.insert(
            String::from("limit"),
            scrub_optional(headers.limit.as_deref(), secret_texts),
        );
        rate.insert(
            String::from("remaining"),
            scrub_optional(headers.remaining.as_deref(), secret_texts),
        );
        rate.insert(
            String::from("reset"),
            scrub_optional(headers.reset.as_deref(), secret_texts),
        );
        rate.insert(
            String::from("retryAfter"),
            scrub_optional(headers.retry_after.as_deref(), secret_texts),
        );
        Value::Object(rate)
    });
    object.insert(String::from("rateLimit"), rate_limit_value);
    let body_value = step
        .body
        .clone()
        .map_or(Value::Null, |raw| scrub_value(raw, secret_texts));
    object.insert(String::from("body"), body_value);
    Value::Object(object)
}

fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() || needle.len() > haystack.len() {
        return false;
    }
    let last_start = haystack.len().saturating_sub(needle.len());
    (0..=last_start)
        .any(|start| haystack.get(start..start.saturating_add(needle.len())) == Some(needle))
}

impl RecordHandle {
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_all(
        &mut self,
        steps: &[RecordedStep],
        secrets: &[&SecretValue],
    ) -> Result<(), RecordError> {
        let variants = build_secret_variants(secrets);
        let secret_texts: Vec<&str> = variants
            .iter()
            .filter_map(|secret| std::str::from_utf8(secret.as_exposed_secret()).ok())
            .filter(|text| !text.is_empty())
            .collect();
        let mut root = Map::new();
        root.insert(String::from("formatVersion"), Value::from(FORMAT_VERSION));
        root.insert(
            String::from("openapiVersion"),
            Value::String(OPENAPI_VERSION.to_owned()),
        );
        let step_values: Vec<Value> = steps
            .iter()
            .map(|step| step_to_value(step, &secret_texts))
            .collect();
        root.insert(String::from("steps"), Value::Array(step_values));
        let mut bytes = serde_json::to_vec_pretty(&Value::Object(root))
            .map_err(|_error| RecordError::IoError)?;
        bytes.push(b'\n');
        for variant in &variants {
            if contains_subslice(&bytes, variant.as_exposed_secret()) {
                return Err(RecordError::SecretLeakDetected);
            }
        }
        let file = self.file.as_mut().ok_or(RecordError::IoError)?;
        file.write_all(&bytes)
            .map_err(|_error| RecordError::IoError)?;
        file.sync_all().map_err(|_error| RecordError::IoError)?;
        self.state = WriteState::Committed;
        Ok(())
    }
}
