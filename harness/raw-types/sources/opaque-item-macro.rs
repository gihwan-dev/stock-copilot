proptest! {
    fn quantity_round_trips(value in 0..100) {
        check(value);
    }
}
