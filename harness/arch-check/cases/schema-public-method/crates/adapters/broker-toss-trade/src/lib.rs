use broker_toss_schema::Account; pub struct Read; impl Read { pub fn read(&self, value: Account) -> Account { value } }
