pub struct Read; pub trait Source { type Value; } impl Source for Read { type Value = broker_toss_schema::Account; }
