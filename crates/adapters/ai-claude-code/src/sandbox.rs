use std::fmt;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const SYSTEM_SANDBOX_EXEC: &str = "/usr/bin/sandbox-exec";
const PROBE_ALLOWED_TOKEN: &str = "sandbox-probe-allowed";
const PROBE_DENIED_TOKEN: &str = "sandbox-probe-denied";
const PROBE_OUTPUT_LIMIT: u64 = 4_096_u64;
const PROBE_WAIT_STEPS: u32 = 500_u32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxProfileError {
    NoDeniedPaths,
    RelativePath,
    UnsafeCharacter,
    RootPath,
    NotNormalized,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SandboxProfile {
    denied: Vec<PathBuf>,
    text: String,
}

impl fmt::Debug for SandboxProfile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SandboxProfile")
            .field("denied_count", &self.denied.len())
            .finish_non_exhaustive()
    }
}

impl SandboxProfile {
    pub fn try_new(denied: &[PathBuf]) -> Result<Self, SandboxProfileError> {
        if denied.is_empty() {
            return Err(SandboxProfileError::NoDeniedPaths);
        }
        let mut resolved = Vec::with_capacity(denied.len());
        for path in denied {
            sbpl_path_literal(path)?;
            resolved.push(resolve_real_path(path)?);
        }
        let mut text = String::from("(version 1)\n(allow default)\n");
        for path in &resolved {
            let literal = sbpl_path_literal(path)?;
            text.push_str("(deny file-read* file-write* (subpath \"");
            text.push_str(literal);
            text.push_str("\"))\n");
        }
        Ok(Self {
            denied: resolved,
            text,
        })
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn denied_paths(&self) -> &[PathBuf] {
        &self.denied
    }

    fn with_extra_denied(&self, extra: &Path) -> Result<Self, SandboxProfileError> {
        let mut denied = self.denied.clone();
        denied.push(extra.to_path_buf());
        Self::try_new(&denied)
    }
}

fn resolve_real_path(path: &Path) -> Result<PathBuf, SandboxProfileError> {
    if path
        .components()
        .any(|component| matches_dot_component(component))
    {
        return Err(SandboxProfileError::NotNormalized);
    }
    let mut existing = path.to_path_buf();
    let mut missing = Vec::new();
    loop {
        match fs::canonicalize(&existing) {
            Ok(real) => {
                let mut full = real;
                for part in missing.iter().rev() {
                    full.push(part);
                }
                return Ok(full);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = existing.file_name().map(std::ffi::OsStr::to_os_string) else {
                    return Err(SandboxProfileError::NotNormalized);
                };
                missing.push(name);
                if !existing.pop() {
                    return Err(SandboxProfileError::NotNormalized);
                }
            }
            Err(_) => return Err(SandboxProfileError::NotNormalized),
        }
    }
}

const fn matches_dot_component(component: std::path::Component<'_>) -> bool {
    match component {
        std::path::Component::CurDir | std::path::Component::ParentDir => true,
        std::path::Component::Prefix(_)
        | std::path::Component::RootDir
        | std::path::Component::Normal(_) => false,
    }
}

fn sbpl_path_literal(path: &Path) -> Result<&str, SandboxProfileError> {
    if !path.is_absolute() {
        return Err(SandboxProfileError::RelativePath);
    }
    let Some(text) = path.to_str() else {
        return Err(SandboxProfileError::UnsafeCharacter);
    };
    if text
        .chars()
        .any(|ch| ch == '"' || ch == '\\' || ch.is_control())
    {
        return Err(SandboxProfileError::UnsafeCharacter);
    }
    if path.parent().is_none() {
        return Err(SandboxProfileError::RootPath);
    }
    Ok(text)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SandboxLayer {
    sandbox_exec: PathBuf,
    profile: SandboxProfile,
}

impl SandboxLayer {
    #[must_use]
    pub const fn new(sandbox_exec: PathBuf, profile: SandboxProfile) -> Self {
        Self {
            sandbox_exec,
            profile,
        }
    }

    #[must_use]
    pub fn system(profile: SandboxProfile) -> Self {
        Self::new(PathBuf::from(SYSTEM_SANDBOX_EXEC), profile)
    }

    #[must_use]
    pub fn sandbox_exec(&self) -> &Path {
        &self.sandbox_exec
    }

    #[must_use]
    pub const fn profile(&self) -> &SandboxProfile {
        &self.profile
    }

    #[must_use]
    pub fn wrap(&self, executable: &Path, args: &[String]) -> (PathBuf, Vec<String>) {
        let mut wrapped = Vec::with_capacity(args.len().saturating_add(3_usize));
        wrapped.push(String::from("-p"));
        wrapped.push(self.profile.text.clone());
        wrapped.push(executable.to_string_lossy().into_owned());
        wrapped.extend(args.iter().cloned());
        (self.sandbox_exec.clone(), wrapped)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SandboxProbeOutcome {
    Applied,
    NotApplied,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeExit {
    Success,
    Failure,
}

#[must_use]
pub fn classify_sandbox_probe(exit: ProbeExit, stdout: &str) -> SandboxProbeOutcome {
    let ran = stdout.contains(PROBE_ALLOWED_TOKEN);
    let leaked = stdout.contains(PROBE_DENIED_TOKEN);
    match (ran, leaked, exit) {
        (_, true, _) => SandboxProbeOutcome::NotApplied,
        (true, false, ProbeExit::Failure) => SandboxProbeOutcome::Applied,
        (true, false, ProbeExit::Success) | (false, false, _) => SandboxProbeOutcome::Unavailable,
    }
}

#[must_use]
pub fn probe_sandbox_layer(layer: &SandboxLayer, scratch: &Path) -> SandboxProbeOutcome {
    let allowed_dir = scratch.join("probe-allowed");
    let denied_dir = scratch.join("probe-denied");
    let outcome = run_probe(layer, &allowed_dir, &denied_dir);
    remove_quietly(&allowed_dir);
    remove_quietly(&denied_dir);
    outcome
}

fn run_probe(layer: &SandboxLayer, allowed_dir: &Path, denied_dir: &Path) -> SandboxProbeOutcome {
    if fs::create_dir_all(allowed_dir).is_err() || fs::create_dir_all(denied_dir).is_err() {
        return SandboxProbeOutcome::Unavailable;
    }
    let allowed_file = allowed_dir.join("canary");
    let denied_file = denied_dir.join("canary");
    if fs::write(&allowed_file, PROBE_ALLOWED_TOKEN).is_err()
        || fs::write(&denied_file, PROBE_DENIED_TOKEN).is_err()
    {
        return SandboxProbeOutcome::Unavailable;
    }
    let Ok(profile) = layer.profile.with_extra_denied(denied_dir) else {
        return SandboxProbeOutcome::Unavailable;
    };
    let probe_layer = SandboxLayer::new(layer.sandbox_exec.clone(), profile);
    let (executable, args) = probe_layer.wrap(
        Path::new("/bin/cat"),
        &[
            allowed_file.to_string_lossy().into_owned(),
            denied_file.to_string_lossy().into_owned(),
        ],
    );
    let Ok(mut child) = Command::new(executable)
        .args(args)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return SandboxProbeOutcome::Unavailable;
    };
    let Some(status) = wait_with_deadline(&mut child) else {
        return SandboxProbeOutcome::Unavailable;
    };
    let mut stdout = String::new();
    let read_ok = child.stdout.take().is_some_and(|pipe| {
        pipe.take(PROBE_OUTPUT_LIMIT)
            .read_to_string(&mut stdout)
            .is_ok()
    });
    if !read_ok {
        return SandboxProbeOutcome::Unavailable;
    }
    let exit = if status.success() {
        ProbeExit::Success
    } else {
        ProbeExit::Failure
    };
    classify_sandbox_probe(exit, &stdout)
}

fn wait_with_deadline(child: &mut std::process::Child) -> Option<std::process::ExitStatus> {
    let (_keep, pause) = std::sync::mpsc::channel::<()>();
    for _attempt in 0_u32..PROBE_WAIT_STEPS {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) => match pause.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(()) | Err(_) => {}
            },
            Err(_) => break,
        }
    }
    match child.kill() {
        Ok(()) | Err(_) => {}
    }
    match child.wait() {
        Ok(_) | Err(_) => None,
    }
}

fn remove_quietly(path: &Path) {
    match fs::remove_dir_all(path) {
        Ok(()) | Err(_) => {}
    }
}
