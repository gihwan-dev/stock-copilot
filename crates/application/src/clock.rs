use domain::Timestamp;

pub trait Clock: Send + Sync {
    fn now(&self) -> Timestamp;
}
