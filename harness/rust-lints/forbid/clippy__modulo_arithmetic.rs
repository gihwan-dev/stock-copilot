pub fn rest(left: i8, right: i8) -> Option<i8> {
    left.checked_rem(right).map(|_rest| left % right)
}
