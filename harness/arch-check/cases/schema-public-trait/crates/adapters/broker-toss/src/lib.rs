use broker_toss_schema::Account; pub trait Leak { type Row: Into<Account>; fn read(&self) -> Vec<Account>; }
