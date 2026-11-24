use std::ffi::OsString;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use application::Clock;
use broker_toss::{TokioRequestWait, TossEndpoint};
use domain::{Nanosecond, OffsetSeconds, Timestamp, UnixSeconds};
use toss_smoke::RuntimeContext;
use toss_smoke::cli::{Command, parse_args, usage_text};
use toss_smoke::deidentify::{DEIDENTIFY_USAGE, deidentify_files};
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

fn write_stderr_line(text: &str) {
    let mut stderr = io::stderr().lock();
    match writeln!(stderr, "{text}") {
        Ok(()) | Err(_) => {}
    }
}

fn deidentify(rest: &[OsString]) -> ExitCode {
    let [input, output] = rest else {
        write_stderr_line(DEIDENTIFY_USAGE);
        return ExitCode::from(SmokeExitCode::UsageError.as_u8());
    };
    match deidentify_files(Path::new(input), Path::new(output)) {
        Ok(count) => {
            let mut stdout = io::stdout().lock();
            match writeln!(stdout, "픽스처 {count}개를 만들었습니다.") {
                Ok(()) => ExitCode::from(SmokeExitCode::Success.as_u8()),
                Err(_) => ExitCode::from(SmokeExitCode::RuntimeError.as_u8()),
            }
        }
        Err(message) => {
            write_stderr_line(message);
            ExitCode::from(SmokeExitCode::RecordWriteFailed.as_u8())
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    if let Some((first, rest)) = args.split_first()
        && first == "deidentify"
    {
        return deidentify(rest);
    }
    match parse_args(&args) {
        Ok(Command::Usage) => {
            let mut stdout = io::stdout().lock();
            let text = usage_text();
            if writeln!(stdout, "{}", text.trim_end()).is_ok() {
                return ExitCode::from(SmokeExitCode::Success.as_u8());
            }
            write_stderr_line("표준 출력에 사용법을 기록하지 못했습니다.");
            return ExitCode::from(SmokeExitCode::RuntimeError.as_u8());
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
    let lock_path = dirs::data_dir().map(|base| base.join(APP_DATA_DIR_NAME).join(LOCK_FILE_NAME));
    let repo_roots = match std::env::current_dir() {
        Ok(dir) => detect_repo_roots(&dir),
        Err(_) => Vec::new(),
    };
    let mut context = RuntimeContext {
        stdin_kind,
        stdin: io::stdin().lock(),
        stdout: io::stdout().lock(),
        stderr: io::stderr().lock(),
        lock_path,
        repo_roots,
        clock: Arc::new(clock),
        wait: Arc::new(TokioRequestWait),
        endpoint: Some(TossEndpoint::production()),
    };
    let code = runtime.block_on(toss_smoke::execute(&args, &mut context));
    ExitCode::from(code.as_u8())
}
