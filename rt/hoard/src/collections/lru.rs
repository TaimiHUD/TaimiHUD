/// A poor man's LRU cache
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecentlyUsed {
    pub generation: u32,
}

impl RecentlyUsed {
    pub const DEFAULT: Self = Self {
        generation: 0,
    };

    pub fn mark_used(&mut self) {
        self.generation = 0;
    }

    pub fn mark_unused(&mut self) {
        self.generation = self.generation.saturating_add(1);
    }

    pub fn is_elderly(&self, threshold: u32) -> bool {
        self.generation > threshold
    }
}
