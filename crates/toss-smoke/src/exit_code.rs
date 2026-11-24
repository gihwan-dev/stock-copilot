#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeExitCode {
    Success,
    UsageError,
    TerminalInputRejected,
    InputInvalid,
    AppRunning,
    AppStateUnconfirmed,
    RecordPathRejected,
    LockIoError,
    RuntimeError,
    TokenStepFailed,
    AccountsStepFailed,
    HoldingsStepFailed,
    AdditionalStepFailed,
    RecordWriteFailed,
}

impl SmokeExitCode {
    #[must_use]
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Success => 0,
            Self::UsageError => 2,
            Self::TerminalInputRejected => 3,
            Self::InputInvalid => 4,
            Self::AppRunning => 5,
            Self::AppStateUnconfirmed => 6,
            Self::RecordPathRejected => 7,
            Self::LockIoError => 8,
            Self::RuntimeError => 9,
            Self::TokenStepFailed => 10,
            Self::AccountsStepFailed => 11,
            Self::HoldingsStepFailed => 12,
            Self::AdditionalStepFailed => 13,
            Self::RecordWriteFailed => 14,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Success => "성공",
            Self::UsageError => "인자 오류",
            Self::TerminalInputRejected => "터미널 입력 거부",
            Self::InputInvalid => "표준 입력 형식 오류",
            Self::AppRunning => "앱 실행 중 거부",
            Self::AppStateUnconfirmed => "앱 종료 미확인 거부",
            Self::RecordPathRejected => "녹화 경로 거부",
            Self::LockIoError => "잠금 파일 입출력 오류",
            Self::RuntimeError => "실행 환경 오류",
            Self::TokenStepFailed => "토큰 발급 실패",
            Self::AccountsStepFailed => "계좌 조회 실패",
            Self::HoldingsStepFailed => "보유 주식 조회 실패",
            Self::AdditionalStepFailed => "추가 읽기 조회 실패",
            Self::RecordWriteFailed => "녹화 파일 쓰기 실패",
        }
    }
}
