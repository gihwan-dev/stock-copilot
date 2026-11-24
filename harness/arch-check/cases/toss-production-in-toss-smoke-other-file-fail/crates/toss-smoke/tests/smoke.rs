fn leaked_test() {
    let endpoint = broker_toss::TossEndpoint::production();
    let _ = endpoint;
}
