pub mod types;

pub fn untested(value: u32) -> u32 {
    value.saturating_add(1)
}
