fn construct() { let _clock = platform_macos::SystemClock::new(); }
use platform_macos::SystemClock as Native;
pub mod child;
