pub enum Event {
    Filled(u32),
    Partial { remaining: i16 },
    Closed,
}
