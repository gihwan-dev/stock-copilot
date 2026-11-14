use broker_toss_schema::Account as Wire; type Internal = Option<Wire>; fn private(value: Internal) {} pub(crate) fn internal(value: Wire) {} pub struct Wrapper { value: Wire }
