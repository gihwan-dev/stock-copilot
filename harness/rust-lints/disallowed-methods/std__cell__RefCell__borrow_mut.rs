pub fn write(cell: &std::cell::RefCell<u8>) {
    *cell.borrow_mut() = 1;
}
