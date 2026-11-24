pub mod cli;
pub mod exit_code;
pub mod input;
pub mod lock;
pub mod record;
pub mod report;
pub mod shape;
pub mod steps;

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::Arc;

use application::Clock;
use broker_toss::{RequestWait, TossEndpoint, TossHttpClient};

use crate::cli::{Command, parse_args, usage_text};
use crate::exit_code::SmokeExitCode;
use crate::input::{InputError, StdinKind, read_input};
use crate::lock::{LockError, acquire_token_lock};
use crate::record::{RecordHandle, prepare_recording};
use crate::report::write_reports;
use crate::steps::run_all;

pub struct RuntimeContext<R, Out, Err> {
    pub stdin_kind: StdinKind,
    pub stdin: R,
    pub stdout: Out,
    pub stderr: Err,
    pub lock_path: Option<PathBuf>,
    pub repo_roots: Vec<PathBuf>,
    pub clock: Arc<dyn Clock>,
    pub wait: Arc<dyn RequestWait>,
    pub endpoint: Option<TossEndpoint>,
}

impl<R, Out, Err> std::fmt::Debug for RuntimeContext<R, Out, Err> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RuntimeContext")
            .field("stdin_kind", &self.stdin_kind)
            .finish_non_exhaustive()
    }
}

fn write_stderr_line<W: Write>(writer: &mut W, text: &str) {
    match writeln!(writer, "{text}") {
        Ok(()) | Err(_) => {}
    }
}

fn lock_error_exit(error: LockError) -> SmokeExitCode {
    match error {
        LockError::AppRunning => SmokeExitCode::AppRunning,
        LockError::UnconfirmedIndeterminate => SmokeExitCode::AppStateUnconfirmed,
        LockError::MissingDefaultPath | LockError::UnsafePath | LockError::IoError => {
            SmokeExitCode::LockIoError
        }
    }
}

fn input_error_exit(error: InputError) -> SmokeExitCode {
    match error {
        InputError::TerminalStdin => SmokeExitCode::TerminalInputRejected,
        InputError::ReadFailed
        | InputError::TooLarge
        | InputError::InvalidUtf8
        | InputError::MissingClientId
        | InputError::MissingClientSecret
        | InputError::InvalidClientId
        | InputError::InvalidClientSecret
        | InputError::InvalidConfirmation
        | InputError::UnexpectedExtraInput => SmokeExitCode::InputInvalid,
    }
}

fn promote_if_success(base: SmokeExitCode, fallback: SmokeExitCode) -> SmokeExitCode {
    match base {
        SmokeExitCode::Success => fallback,
        other @ (SmokeExitCode::UsageError
        | SmokeExitCode::TerminalInputRejected
        | SmokeExitCode::InputInvalid
        | SmokeExitCode::AppRunning
        | SmokeExitCode::AppStateUnconfirmed
        | SmokeExitCode::RecordPathRejected
        | SmokeExitCode::LockIoError
        | SmokeExitCode::RuntimeError
        | SmokeExitCode::TokenStepFailed
        | SmokeExitCode::AccountsStepFailed
        | SmokeExitCode::HoldingsStepFailed
        | SmokeExitCode::AdditionalStepFailed
        | SmokeExitCode::RecordWriteFailed) => other,
    }
}

fn prepare_optional_recording<Err: Write>(
    record_path: Option<&PathBuf>,
    repo_roots: &[PathBuf],
    stderr: &mut Err,
) -> Result<Option<RecordHandle>, SmokeExitCode> {
    let Some(path) = record_path else {
        return Ok(None);
    };
    match prepare_recording(path, repo_roots) {
        Ok(handle) => {
            write_stderr_line(
                stderr,
                "안내: 녹화 파일에는 읽기 응답 본문이 저장되며 키·토큰은 제외됩니다. 저장소 밖에서만 보관하고 T-14 비식별화 전까지 저장소에 넣지 마세요.",
            );
            Ok(Some(handle))
        }
        Err(error) => {
            write_stderr_line(stderr, error.message());
            Err(SmokeExitCode::RecordPathRejected)
        }
    }
}

fn persist_recording<Err: Write>(
    record_handle: Option<&mut RecordHandle>,
    smoke_input: &input::SmokeInput,
    run_result: &steps::RunResult,
    stderr: &mut Err,
) -> Result<Option<PathBuf>, SmokeExitCode> {
    let Some(handle) = record_handle else {
        return Ok(None);
    };
    let mut secrets = vec![smoke_input.client_id(), smoke_input.client_secret()];
    if let Some(token) = run_result.access_token() {
        secrets.push(token);
    }
    match handle.write_all(run_result.recordings(), &secrets) {
        Ok(()) => Ok(Some(handle.path().to_path_buf())),
        Err(error) => {
            write_stderr_line(stderr, error.message());
            Err(SmokeExitCode::RecordWriteFailed)
        }
    }
}

fn acquire_lock_for_run<Err: Write>(
    lock_path: Option<&PathBuf>,
    confirmation: input::AppStopConfirmation,
    stderr: &mut Err,
) -> Result<lock::LockGuard, SmokeExitCode> {
    let Some(path) = lock_path else {
        write_stderr_line(stderr, LockError::MissingDefaultPath.message());
        return Err(SmokeExitCode::LockIoError);
    };
    match acquire_token_lock(path, confirmation) {
        Ok(guard) => Ok(guard),
        Err(error) => {
            write_stderr_line(stderr, error.message());
            Err(lock_error_exit(error))
        }
    }
}

fn finalize_run<Out: Write, Err: Write>(
    record_handle: Option<&mut RecordHandle>,
    smoke_input: &input::SmokeInput,
    run_result: &steps::RunResult,
    stdout: &mut Out,
    stderr: &mut Err,
) -> SmokeExitCode {
    let (final_code, recorded_path) =
        match persist_recording(record_handle, smoke_input, run_result, stderr) {
            Ok(path) => (run_result.exit_code(), path),
            Err(write_code) => (promote_if_success(run_result.exit_code(), write_code), None),
        };
    if write_reports(
        stdout,
        run_result.reports(),
        final_code,
        recorded_path.as_deref(),
    )
    .is_ok()
    {
        final_code
    } else {
        write_stderr_line(stderr, "표준 출력에 결과를 기록하지 못했습니다.");
        promote_if_success(final_code, SmokeExitCode::RuntimeError)
    }
}

fn build_client<R, Out, Err>(
    context: &mut RuntimeContext<R, Out, Err>,
) -> Result<TossHttpClient, SmokeExitCode>
where
    Err: Write,
{
    let Some(endpoint) = context.endpoint.as_ref() else {
        write_stderr_line(&mut context.stderr, "엔드포인트 구성에 실패했습니다.");
        return Err(SmokeExitCode::RuntimeError);
    };
    match TossHttpClient::new(
        endpoint,
        Arc::clone(&context.clock),
        Arc::clone(&context.wait),
    ) {
        Ok(built) => Ok(built),
        Err(_error) => {
            write_stderr_line(
                &mut context.stderr,
                "HTTP 클라이언트 초기화에 실패했습니다.",
            );
            Err(SmokeExitCode::RuntimeError)
        }
    }
}

pub async fn execute<R, Out, Err>(
    args: &[OsString],
    context: &mut RuntimeContext<R, Out, Err>,
) -> SmokeExitCode
where
    R: Read,
    Out: Write,
    Err: Write,
{
    let options = match parse_args(args) {
        Ok(Command::Usage) => {
            let text = usage_text();
            if writeln!(&mut context.stdout, "{}", text.trim_end()).is_ok() {
                return SmokeExitCode::Success;
            }
            write_stderr_line(
                &mut context.stderr,
                "표준 출력에 사용법을 기록하지 못했습니다.",
            );
            return SmokeExitCode::RuntimeError;
        }
        Ok(Command::Check(parsed)) => parsed,
        Err(error) => {
            write_stderr_line(&mut context.stderr, error.message());
            return SmokeExitCode::UsageError;
        }
    };
    let smoke_input = match read_input(context.stdin_kind, &mut context.stdin) {
        Ok(parsed) => parsed,
        Err(error) => {
            let message = error.message();
            write_stderr_line(&mut context.stderr, &message);
            return input_error_exit(error);
        }
    };
    let mut record_handle = match prepare_optional_recording(
        options.record_path.as_ref(),
        &context.repo_roots,
        &mut context.stderr,
    ) {
        Ok(handle) => handle,
        Err(code) => return code,
    };
    let _lock_guard = match acquire_lock_for_run(
        context.lock_path.as_ref(),
        smoke_input.confirmation(),
        &mut context.stderr,
    ) {
        Ok(guard) => guard,
        Err(code) => return code,
    };
    let client = match build_client(context) {
        Ok(built) => built,
        Err(code) => return code,
    };
    let run_result = run_all(&client, &smoke_input).await;
    finalize_run(
        record_handle.as_mut(),
        &smoke_input,
        &run_result,
        &mut context.stdout,
        &mut context.stderr,
    )
}
