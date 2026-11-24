use std::fmt;
use std::io::Read;

use application::SecretValue;

pub const CONFIRM_APP_STOPPED_PHRASE: &str = "앱을 껐습니다";

pub const NO_ECHO_GUIDE_COMMANDS: &str = "( trap 'unset id sec confirm' EXIT; printf 'Client ID: ' >&2 && IFS= read -rs id && printf '\\nClient Secret: ' >&2 && IFS= read -rs sec && printf '\\n앱 종료 확인(앱을 껐습니다): ' >&2 && IFS= read -r confirm && printf '%s\\n%s\\n%s\\n' \"$id\" \"$sec\" \"$confirm\" | cargo run --locked -p toss-smoke -- check )";

#[must_use]
pub fn no_echo_guide() -> String {
    format!(
        "\
터미널 표준 입력에서는 키를 직접 읽지 않고 중단합니다 (화면 노출 방지).
화면에 되비침 없이 파이프로 전달하려면 아래 한 줄을 그대로 실행하세요 (zsh/bash):

{NO_ECHO_GUIDE_COMMANDS}

안내:
  - 한 줄 서브셸로 실행되어 macOS bash/zsh 어디에 붙여넣어도 키가 셸 기록·화면에 남지 않습니다.
  - Ctrl-C로 중간 중단하면 서브셸이 즉시 종료되고 터미널 되비침이 복구되며 현재 셸에 키가 남지 않습니다.
  - 앱이 꺼진 것을 직접 확인한 뒤 프롬프트에 `앱을 껐습니다`를 입력하세요(잠금 파일로 종료가 증명되면 빈 줄 Enter 가능).
  - 응답 녹화가 필요하면 `check` 뒤에 `--record <저장소_밖_파일_경로>`를 붙이세요.
"
    )
}

const MAX_INPUT_BYTES: usize = 8192;
const MAX_CLIENT_ID_BYTES: usize = 512;
const MAX_CLIENT_SECRET_BYTES: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StdinKind {
    Terminal,
    NonTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AppStopConfirmation {
    NotProvided,
    Confirmed,
}

pub struct SmokeInput {
    client_id: SecretValue,
    client_secret: SecretValue,
    confirmation: AppStopConfirmation,
}

impl fmt::Debug for SmokeInput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SmokeInput(<redacted>)")
    }
}

impl SmokeInput {
    #[must_use]
    pub const fn client_id(&self) -> &SecretValue {
        &self.client_id
    }

    #[must_use]
    pub const fn client_secret(&self) -> &SecretValue {
        &self.client_secret
    }

    #[must_use]
    pub const fn confirmation(&self) -> AppStopConfirmation {
        self.confirmation
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputError {
    TerminalStdin,
    ReadFailed,
    TooLarge,
    InvalidUtf8,
    MissingClientId,
    MissingClientSecret,
    InvalidClientId,
    InvalidClientSecret,
    InvalidConfirmation,
    UnexpectedExtraInput,
}

impl InputError {
    #[must_use]
    pub fn message(self) -> String {
        match self {
            Self::TerminalStdin => no_echo_guide(),
            Self::ReadFailed => String::from("표준 입력을 읽는 중 입출력 오류가 발생했습니다."),
            Self::TooLarge => String::from("표준 입력 크기가 허용 상한(8192바이트)을 넘었습니다."),
            Self::InvalidUtf8 => String::from("표준 입력이 올바른 UTF-8 문자열이 아닙니다."),
            Self::MissingClientId => String::from("표준 입력 1줄(client_id)이 비어 있습니다."),
            Self::MissingClientSecret => String::from("표준 입력 2줄(client_secret)이 없습니다."),
            Self::InvalidClientId => String::from(
                "표준 입력 1줄(client_id)에 공백·제어 문자·비ASCII 문자가 있거나 길이가 초과되었습니다.",
            ),
            Self::InvalidClientSecret => String::from(
                "표준 입력 2줄(client_secret)에 공백·제어 문자·비ASCII 문자가 있거나 길이가 초과되었습니다.",
            ),
            Self::InvalidConfirmation => String::from(
                "표준 입력 3줄의 앱 종료 확인 문구가 정확히 `앱을 껐습니다`와 일치하지 않습니다.",
            ),
            Self::UnexpectedExtraInput => {
                String::from("표준 입력에 예상하지 못한 추가 줄이 있습니다.")
            }
        }
    }
}

const MAX_INPUT_READ_LIMIT: u64 = 8193;

fn read_bounded<R: Read>(reader: &mut R) -> Result<SecretValue, InputError> {
    let mut raw = Vec::with_capacity(MAX_INPUT_BYTES.saturating_add(1));
    let mut limited = reader.take(MAX_INPUT_READ_LIMIT);
    let result = limited.read_to_end(&mut raw);
    let secret = SecretValue::new(raw);
    result.map_err(|_error| InputError::ReadFailed)?;
    if secret.as_exposed_secret().len() > MAX_INPUT_BYTES {
        return Err(InputError::TooLarge);
    }
    Ok(secret)
}

fn validate_credential_bytes(
    bytes: &[u8],
    max_len: usize,
    empty_error: InputError,
    invalid_error: InputError,
) -> Result<SecretValue, InputError> {
    if bytes.is_empty() {
        return Err(empty_error);
    }
    if bytes.len() > max_len || !bytes.iter().all(|byte| (0x21..=0x7E).contains(byte)) {
        return Err(invalid_error);
    }
    Ok(SecretValue::new(bytes.to_vec()))
}

fn trim_trailing_cr(line: &str) -> &str {
    line.strip_suffix('\r').unwrap_or(line)
}

fn parse_confirmation(line: Option<&str>) -> Result<AppStopConfirmation, InputError> {
    match line {
        None | Some("") => Ok(AppStopConfirmation::NotProvided),
        Some(text) if text == CONFIRM_APP_STOPPED_PHRASE => Ok(AppStopConfirmation::Confirmed),
        Some(_) => Err(InputError::InvalidConfirmation),
    }
}

pub fn read_input<R: Read>(
    stdin_kind: StdinKind,
    reader: &mut R,
) -> Result<SmokeInput, InputError> {
    match stdin_kind {
        StdinKind::Terminal => return Err(InputError::TerminalStdin),
        StdinKind::NonTerminal => {}
    }
    let raw = read_bounded(reader)?;
    let text =
        std::str::from_utf8(raw.as_exposed_secret()).map_err(|_error| InputError::InvalidUtf8)?;
    let trimmed_end = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if trimmed_end.is_empty() {
        return Err(InputError::MissingClientId);
    }
    let lines: Vec<&str> = trimmed_end.split('\n').map(trim_trailing_cr).collect();
    if lines.len() > 3 {
        return Err(InputError::UnexpectedExtraInput);
    }
    let first = lines.first().copied().ok_or(InputError::MissingClientId)?;
    let second = lines
        .get(1)
        .copied()
        .ok_or(InputError::MissingClientSecret)?;
    let client_id = validate_credential_bytes(
        first.as_bytes(),
        MAX_CLIENT_ID_BYTES,
        InputError::MissingClientId,
        InputError::InvalidClientId,
    )?;
    let client_secret = validate_credential_bytes(
        second.as_bytes(),
        MAX_CLIENT_SECRET_BYTES,
        InputError::MissingClientSecret,
        InputError::InvalidClientSecret,
    )?;
    let confirmation = parse_confirmation(lines.get(2).copied())?;
    Ok(SmokeInput {
        client_id,
        client_secret,
        confirmation,
    })
}
