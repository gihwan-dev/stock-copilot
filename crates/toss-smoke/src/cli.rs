use std::ffi::OsString;
use std::path::PathBuf;

use crate::input::NO_ECHO_GUIDE_COMMANDS;

#[must_use]
pub fn usage_text() -> String {
    format!(
        "\
사용법:
  toss-smoke
  toss-smoke --help
  toss-smoke check [--record <저장소_밖_파일_경로>]

설명:
  인자 없이 실행하면 네트워크와 표준 입력에 닿지 않고 이 안내만 출력하고 종료(0)합니다.
  실제 연결 확인은 반드시 `check` 하위 명령으로 실행합니다.
  키는 명령행 인자·환경 변수·설정 파일로 받지 않으며, 표준 입력(파이프·리다이렉트)으로만 받습니다.

표준 입력 형식 (줄바꿈 구분):
  1줄: client_id
  2줄: client_secret
  3줄(선택): 앱을 껐습니다  (앱 종료 상태를 잠금 파일로 증명할 수 없을 때 필수)

화면에 되비침 없이 실행하는 예 (zsh/bash):
{NO_ECHO_GUIDE_COMMANDS}
"
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckOptions {
    pub record_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Usage,
    Check(CheckOptions),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliError {
    UnknownCommand,
    UnknownOption,
    MissingValue,
    DuplicateOption,
    UnexpectedArgument,
    ForbiddenSecretOption,
    NonUtf8Argument,
}

impl CliError {
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::UnknownCommand => "알 수 없는 하위 명령입니다. `toss-smoke --help`를 확인하세요.",
            Self::UnknownOption => "알 수 없는 선택지입니다. `toss-smoke --help`를 확인하세요.",
            Self::MissingValue => "선택지에 필요한 값이 비어 있습니다.",
            Self::DuplicateOption => "같은 선택지를 두 번 지정할 수 없습니다.",
            Self::UnexpectedArgument => "허용되지 않은 위치 인자가 있습니다.",
            Self::ForbiddenSecretOption => {
                "키·토큰·환경 설정은 명령행 인자로 받지 않습니다. 표준 입력 파이프로만 전달하세요."
            }
            Self::NonUtf8Argument => "UTF-8로 해석할 수 없는 명령행 인자가 포함되어 있습니다.",
        }
    }
}

const SECRET_OPTION_NAMES: [&str; 17] = [
    "--client-id",
    "--client-secret",
    "--client_id",
    "--client_secret",
    "--secret",
    "--token",
    "--access-token",
    "--key",
    "--api-key",
    "--password",
    "--pass",
    "--config",
    "--env",
    "--env-file",
    "--credential",
    "--credentials",
    "-k",
];

fn is_forbidden_secret_option(token: &str) -> bool {
    let base = token
        .split_once('=')
        .map_or(token, |(prefix, _value)| prefix);
    SECRET_OPTION_NAMES.contains(&base) || base == "-s" || base == "-t" || base == "-p"
}

fn parse_non_empty_path(raw: &OsString) -> Result<PathBuf, CliError> {
    if raw.is_empty() {
        return Err(CliError::MissingValue);
    }
    Ok(PathBuf::from(raw))
}

fn take_next(args: &[OsString], index: &mut usize) -> Result<OsString, CliError> {
    let next_index = index.saturating_add(1);
    let value = args.get(next_index).ok_or(CliError::MissingValue)?;
    *index = next_index;
    Ok(value.clone())
}

fn parse_check_options(args: &[OsString]) -> Result<Command, CliError> {
    let mut record_path: Option<PathBuf> = None;
    let mut index = 0_usize;
    while let Some(raw_arg) = args.get(index) {
        let token = raw_arg.to_str().ok_or(CliError::NonUtf8Argument)?;
        if token == "-h" || token == "--help" {
            return Ok(Command::Usage);
        }
        if is_forbidden_secret_option(token) {
            return Err(CliError::ForbiddenSecretOption);
        }
        match token {
            "--record" => {
                if record_path.is_some() {
                    return Err(CliError::DuplicateOption);
                }
                let next_val = take_next(args, &mut index)?;
                record_path = Some(parse_non_empty_path(&next_val)?);
            }
            other if other.starts_with('-') => return Err(CliError::UnknownOption),
            _ => return Err(CliError::UnexpectedArgument),
        }
        index = index.saturating_add(1);
    }
    Ok(Command::Check(CheckOptions { record_path }))
}

pub fn parse_args(args: &[OsString]) -> Result<Command, CliError> {
    for raw in args {
        let token = raw.to_str().ok_or(CliError::NonUtf8Argument)?;
        if is_forbidden_secret_option(token) {
            return Err(CliError::ForbiddenSecretOption);
        }
    }
    let Some(first) = args.first() else {
        return Ok(Command::Usage);
    };
    let head = first.to_str().ok_or(CliError::NonUtf8Argument)?;
    match head {
        "-h" | "--help" | "help" => {
            if args.len() == 1 {
                Ok(Command::Usage)
            } else {
                Err(CliError::UnexpectedArgument)
            }
        }
        "check" => parse_check_options(args.get(1..).unwrap_or(&[])),
        other if other.starts_with('-') => Err(CliError::UnknownOption),
        _ => Err(CliError::UnknownCommand),
    }
}
