fn invoke<F: Fn() -> T, T>(f: F) -> T { f() }
pub fn g() { let _ = invoke(domain::UserIdentityConfirmation::issue_after_user_confirmation); }
