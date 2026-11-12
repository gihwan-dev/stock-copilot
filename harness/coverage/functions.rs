fn plain() {
    run();
}

struct Holder;

impl Holder {
    fn in_impl(&self) -> u32 {
        1
    }
}

mod inner {
    pub fn nested_module(value: u32)
        -> u32 {
        value
    }
}

fn repeated() {
    first();
}

mod again {
    fn repeated() {
        second();
    }
}

trait Shape {
    fn declared(&self);
}
