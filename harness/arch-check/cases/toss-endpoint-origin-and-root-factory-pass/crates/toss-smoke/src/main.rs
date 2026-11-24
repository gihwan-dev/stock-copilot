mod other;
fn main() {
    let endpoint = broker_toss::TossEndpoint::production();
    other::local_only();
}
