/// A poor man's LRU cache
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RecentlyUsed {
    pub generation: u32,
}

impl RecentlyUsed {
    pub const DEFAULT: Self = Self { generation: 0 };

    pub fn mark_used(&mut self) {
        self.generation = 0;
    }
    pub fn mark_unused(&mut self) {
        self.nudge_by(1);
    }
    pub fn mark_if(&mut self, used: bool) {
        match used {
            true => self.mark_used(),
            false => self.mark_unused(),
        }
    }

    pub fn nudge_by(&mut self, amt: i32) {
        self.generation = self.generation.saturating_add_signed(amt);
    }
    pub fn nudge_if(&mut self, used: bool) {
        self.nudge_by(match used {
            true => -1,
            false => 1,
        })
    }

    pub fn mark_for_death(&mut self) {
        self.generation = u32::MAX;
    }

    pub fn is_elderly(&self, threshold: u32) -> bool {
        self.generation > threshold
    }
}
