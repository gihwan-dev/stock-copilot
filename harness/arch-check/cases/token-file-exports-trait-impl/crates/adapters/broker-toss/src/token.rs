pub(crate) struct Sender;

impl Send for Sender {
    fn verb(&self) -> Method {
        Method::POST
    }
}
