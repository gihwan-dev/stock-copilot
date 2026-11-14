use broker_toss_schema as wire; type Internal = wire::Account; type Second = Option<Internal>; pub type Leak = Vec<Second>;
