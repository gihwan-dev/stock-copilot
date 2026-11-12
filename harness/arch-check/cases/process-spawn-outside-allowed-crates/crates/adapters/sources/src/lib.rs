use tokio::process;

pub(crate) fn run() {
    process::Command::new("cli");
}
