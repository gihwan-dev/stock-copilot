use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use application::Clock;
use broker_toss::{TokioRequestWait, TossEndpoint, TossHttpError};
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use toss_smoke::RuntimeContext;
use toss_smoke::cli::{Command, TargetEndpoint, USAGE_TEXT, parse_args};
use toss_smoke::exit_code::SmokeExitCode;
use toss_smoke::input::StdinKind;
use toss_smoke::lock::{APP_DATA_DIR_NAME, LOCK_FILE_NAME};
use toss_smoke::record::detect_repo_roots;

const MIN_SECONDS: i64 = -62_135_596_800;
const MAX_SECONDS: i64 = 253_402_300_799;

#[derive(Debug)]
struct SmokeSystemClock {
    minimum: Timestamp,
    maximum: Timestamp,
}

impl SmokeSystemClock {
    fn new() -> Option<Self> {
        Some(Self {
            minimum: Timestamp::from_parts(
                UnixSeconds::new(MIN_SECONDS),
                Nanosecond::new(0),
                OffsetSeconds::new(0),
            )
            .ok()?,
            maximum: Timestamp::from_parts(
                UnixSeconds::new(MAX_SECONDS),
                Nanosecond::new(999_999_999),
                OffsetSeconds::new(0),
            )
            .ok()?,
        })
    }

    fn convert(&self, value: SystemTime) -> Timestamp {
        let (seconds, nanos) = match value.duration_since(UNIX_EPOCH) {
            Ok(duration) => (i128::from(duration.as_secs()), duration.subsec_nanos()),
            Err(error) => {
                let duration = error.duration();
                let negative = i128::from(duration.as_secs()).saturating_neg();
                match duration.subsec_nanos() {
                    0 => (negative, 0),
                    sub => (
                        negative.saturating_sub(1),
                        1_000_000_000_u32.saturating_sub(sub),
                    ),
                }
            }
        };
        if seconds < i128::from(MIN_SECONDS) {
            return self.minimum;
        }
        if seconds > i128::from(MAX_SECONDS) {
            return self.maximum;
        }
        match i64::try_from(seconds).ok().and_then(|whole| {
            Timestamp::from_parts(
                UnixSeconds::new(whole),
                Nanosecond::new(nanos),
                OffsetSeconds::new(0),
            )
            .ok()
        }) {
            Some(timestamp) => timestamp,
            None => self.maximum,
        }
    }
}

impl Clock for SmokeSystemClock {
    fn now(&self) -> Timestamp {
        self.convert(SystemTime::now())
    }
}

fn build_endpoint(target: TargetEndpoint) -> Result<TossEndpoint, TossHttpError> {
    match target {
        TargetEndpoint::Live => Ok(TossEndpoint::production()),
        TargetEndpoint::LocalPort(port) => TossEndpoint::local(port),
    }
}

fn write_stderr_line(text: &str) {
    let mut stderr = io::stderr().lock();
    match writeln!(stderr, "{text}") {
        Ok(()) | Err(_) => {}
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    match parse_args(&args) {
        Ok(Command::Usage) => {
            let mut stdout = io::stdout().lock();
            match writeln!(stdout, "{}", USAGE_TEXT.trim_end()) {
                Ok(()) | Err(_) => {}
            }
            return ExitCode::from(SmokeExitCode::Success.as_u8());
        }
        Err(error) => {
            write_stderr_line(error.message());
            return ExitCode::from(SmokeExitCode::UsageError.as_u8());
        }
        Ok(Command::Check(_)) => {}
    }
    let Some(clock) = SmokeSystemClock::new() else {
        write_stderr_line("시스템 시계 초기화에 실패했습니다.");
        return ExitCode::from(SmokeExitCode::RuntimeError.as_u8());
    };
    let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    else {
        write_stderr_line("비동기 런타임 초기화에 실패했습니다.");
        return ExitCode::from(SmokeExitCode::RuntimeError.as_u8());
    };
    let stdin_kind = if io::stdin().is_terminal() {
        StdinKind::Terminal
    } else {
        StdinKind::NonTerminal
    };
    let default_lock_path =
        dirs::data_dir().map(|base| base.join(APP_DATA_DIR_NAME).join(LOCK_FILE_NAME));
    let repo_roots = match std::env::current_dir() {
        Ok(dir) => detect_repo_roots(&dir),
        Err(_) => Vec::new(),
    };
    let mut context = RuntimeContext {
        stdin_kind,
        stdin: io::stdin().lock(),
        stdout: io::stdout().lock(),
        stderr: io::stderr().lock(),
        default_lock_path,
        repo_roots,
        clock: Arc::new(clock),
        wait: Arc::new(TokioRequestWait),
        build_endpoint,
    };
    let code = runtime.block_on(toss_smoke::execute(&args, &mut context));
    ExitCode::from(code.as_u8())
}
