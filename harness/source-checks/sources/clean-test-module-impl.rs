pub trait Port {}

pub struct Gate;

#[cfg(test)]
mod tests {
    struct Local;

    trait Probe {}

    impl Local {
        fn probe(&self) {}
    }

    impl super::Port for Local {}

    impl Probe for super::Gate {}
}
