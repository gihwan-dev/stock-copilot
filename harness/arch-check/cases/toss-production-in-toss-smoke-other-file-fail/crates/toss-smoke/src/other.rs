pub fn leaked() {
    let endpoint = broker_toss::TossEndpoint::production();
    let _ = endpoint;
}
