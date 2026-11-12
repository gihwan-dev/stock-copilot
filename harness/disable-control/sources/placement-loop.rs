pub fn drain(queue: &mut Queue) {
    #[expect(clippy::redundant_clone, reason = "반복 전체")]
    loop {
        queue.next();
    }
}
