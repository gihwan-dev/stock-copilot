use broker_toss_schema::Account as Wire; pub fn leak(value: Vec<Option<Wire>>) -> Result<Wire, ()> { loop {} }
