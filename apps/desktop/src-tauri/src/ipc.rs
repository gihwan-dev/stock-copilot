pub mod app_version;
#[expect(
    clippy::disallowed_macros,
    reason = "derive(specta::Type)가 serde 컨테이너 속성이 있는 타입에 unreachable!을 펼친다 (ADR 0016)"
)]
pub mod error;
pub mod registry;

#[cfg(test)]
mod tests;
