pub const SELECT_HOLDINGS: &str =
    "SELECT amount, currency FROM holdings WHERE account_id = ?1 ORDER BY seq";

pub const INSERT_LOT: &str = r#"INSERT INTO lots (quantity, cost, currency) VALUES (?1, ?2, ?3)"#;

pub fn label() -> &'static str {
    "보유 종목"
}
