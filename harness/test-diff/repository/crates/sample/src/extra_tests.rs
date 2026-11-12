#[test]
fn adds_zero() {
    assert_eq!(super::add(4, 0), 4, "zero");
}

#[test]
fn adds_large() {
    assert_eq!(super::add(9, 9), 18, "large");
}
