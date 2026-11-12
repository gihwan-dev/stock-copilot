pub fn drain(queue: &mut Queue) {
    #[expect(clippy::redundant_clone, reason = "블록 전체")]
    while queue.next() {
        queue.pop();
    }
}
