use std::ffi::OsString;
use std::process::ExitCode;
use testkit::fake_ai_cli::{
    EnvObservation, EnvPresence, NonessentialTrafficFlag, ProcessEnvironment, RedactedToken,
    run_from_process,
};

fn presence_of(result: &Result<String, std::env::VarError>) -> EnvPresence {
    if result.is_ok() {
        EnvPresence::Present
    } else {
        EnvPresence::Absent
    }
}

fn read_nonessential_flag(result: &Result<String, std::env::VarError>) -> NonessentialTrafficFlag {
    match result.as_deref() {
        Ok("1") => NonessentialTrafficFlag::Disabled,
        Ok(_) => NonessentialTrafficFlag::Other,
        Err(_) => NonessentialTrafficFlag::Unset,
    }
}

fn main() -> ExitCode {
    let raw_args: Vec<OsString> = std::env::args_os().collect();
    let mcp_token = match std::env::var("STOCK_MCP_TOKEN") {
        Ok(raw) => Some(RedactedToken::new(raw)),
        Err(_) => None,
    };
    let observation = EnvObservation {
        mcp_token: if mcp_token.is_some() {
            EnvPresence::Present
        } else {
            EnvPresence::Absent
        },
        anthropic_api_key: presence_of(&std::env::var("ANTHROPIC_API_KEY")),
        openai_api_key: presence_of(&std::env::var("OPENAI_API_KEY")),
        user: presence_of(&std::env::var("USER")),
        disable_nonessential_traffic: read_nonessential_flag(&std::env::var(
            "CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC",
        )),
    };
    let env = ProcessEnvironment {
        mcp_token,
        observation,
    };
    let Ok(executable) = testkit::test_executable() else {
        return ExitCode::from(2_u8);
    };
    run_from_process(&executable, &raw_args, &env)
}
