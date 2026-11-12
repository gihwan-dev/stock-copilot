pub fn read(cell: &std::cell::RefCell<u8>) -> u8 {
    *cell.borrow()
}
