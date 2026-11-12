pub struct Gate;

#[cfg(test)]
mod gate_tests {
    impl super::Gate {
        pub(crate) fn allows(&self) -> bool {
            true
        }
    }
}
