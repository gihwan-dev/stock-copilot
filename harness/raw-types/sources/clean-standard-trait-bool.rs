impl PartialEq for Price {
    fn eq(&self, other: &Self) -> bool {
        self.compare(other).is_eq()
    }

    fn ne(&self, other: &Self) -> bool {
        self.compare(other).is_ne()
    }
}

impl std::cmp::PartialOrd for Price {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.compare(other))
    }

    fn lt(&self, other: &Self) -> bool {
        self.compare(other).is_lt()
    }

    fn le(&self, other: &Self) -> bool {
        self.compare(other).is_le()
    }

    fn gt(&self, other: &Self) -> bool {
        self.compare(other).is_gt()
    }

    fn ge(&self, other: &Self) -> bool {
        self.compare(other).is_ge()
    }
}

impl core::cmp::Ord for Price {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.compare(other)
    }
}
