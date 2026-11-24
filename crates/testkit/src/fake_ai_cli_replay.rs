use crate::fake_ai_cli::{InvocationRecord, RedactedToken, ReplayScenario, RunOutcome, TurnStop};
use std::fs::OpenOptions;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;

pub(crate) fn scrub_optional_token(text: &str, token: Option<&RedactedToken>) -> String {
    match token {
        Some(secret) if !secret.expose_for_auth().is_empty() => {
            text.replace(secret.expose_for_auth(), "[REDACTED_TOKEN]")
        }
        Some(_) | None => String::from(text),
    }
}

pub(crate) fn append_invocation_record(path: &Path, record: &InvocationRecord) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let serialized = serde_json::to_string(record)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(file, "{serialized}")?;
    file.flush()
}

pub(crate) fn write_stdout_lines(lines: &[String]) -> io::Result<()> {
    let mut stdout = io::stdout().lock();
    for line in lines {
        writeln!(stdout, "{line}")?;
    }
    stdout.flush()
}

pub(crate) fn write_stderr_lines(lines: &[String]) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    for line in lines {
        writeln!(stderr, "{line}")?;
    }
    stderr.flush()
}

pub(crate) fn run_replay_mode(
    scenario: &ReplayScenario,
    token: Option<&RedactedToken>,
) -> io::Result<RunOutcome> {
    write_stdout_lines(&scenario.immediate_lines)?;
    let mut stdin_lines = Vec::new();
    if !scenario.turns.is_empty() {
        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin.lock());
        for turn in &scenario.turns {
            let mut buffer = String::new();
            let read_bytes = reader.read_line(&mut buffer)?;
            if read_bytes == 0_usize {
                break;
            }
            let cleaned = buffer.trim_end_matches(['\r', '\n']);
            stdin_lines.push(scrub_optional_token(cleaned, token));
            write_stdout_lines(&turn.stdout_lines)?;
            write_stderr_lines(&turn.stderr_lines)?;
            if turn.stop_after == TurnStop::Terminate {
                break;
            }
        }
    }
    if let Some(trailing) = &scenario.trailing_without_newline {
        let mut stdout = io::stdout().lock();
        stdout.write_all(trailing.as_bytes())?;
        stdout.flush()?;
    }
    write_stderr_lines(&scenario.final_stderr_lines)?;
    Ok(RunOutcome {
        stdin_lines,
        mcp_tools_discovered: Vec::new(),
        mcp_calls_executed: 0_usize,
        exit_code: scenario.exit_code,
    })
}
