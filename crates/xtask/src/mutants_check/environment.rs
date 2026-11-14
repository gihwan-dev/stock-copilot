pub(super) fn jobs_override() -> Result<Option<String>, String> {
    match std::env::var("STOCK_COPILOT_MUTANTS_JOBS") {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(String::from(
            "STOCK_COPILOT_MUTANTS_JOBS는 UTF-8 정수여야 한다",
        )),
    }
}
