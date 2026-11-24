use std::ffi::OsString;
use std::io::{self, Write};
use std::process::ExitCode;

use mcp_bridge::bridge::{BridgeError, BridgeToken, parse_socket_target, run_bridge_with_io};

fn write_error_line(error: BridgeError) {
    let mut stderr = io::stderr().lock();
    match writeln!(stderr, "mcp-bridge: {}", error.fixed_message()) {
        Ok(()) | Err(_) => {}
    }
}

fn execute() -> Result<(), BridgeError> {
    let args: Vec<OsString> = std::env::args_os().collect();
    let socket_target = parse_socket_target(&args)?;
    let raw_token = std::env::var("STOCK_MCP_TOKEN").map_err(|_error| BridgeError::MissingToken)?;
    let token = BridgeToken::try_new(raw_token)?;
    run_bridge_with_io(&socket_target, &token, io::stdin(), io::stdout())
}

fn main() -> ExitCode {
    match execute() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            write_error_line(error);
            error.exit_code()
        }
    }
}
