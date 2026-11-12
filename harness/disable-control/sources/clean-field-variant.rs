pub struct Holding {
    #[expect(clippy::struct_field_names, reason = "필드 하나")]
    holding_amount: Amount,
}

pub enum Signal {
    #[expect(clippy::large_stack_arrays, reason = "변형 하나")]
    Buy(Window),
    Hold,
}
