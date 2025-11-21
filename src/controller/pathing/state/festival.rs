use taimi_pack::attributes::{Festivals, Festival};

#[derive(Debug, Default, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FestivalState {
    pub active: Festivals,
    pub on: Festivals,
    pub off: Festivals,
}

impl FestivalState {
    pub const DEFAULT: Self = Self {
        active: Festivals::empty(),
        on: Festivals::empty(),
        off: Festivals::empty(),
    };

    pub fn update_preferences(&mut self, (on, off): (Festivals, Festivals)) {
        self.on = on;
        self.off = off;
    }
    pub fn set_preference(&mut self, festival: Festival, pref: Option<FestivalPreference>) {
        let festival = Festivals::from(festival);
        self.on.remove(festival);
        self.off.remove(festival);
        match pref {
            Some(true) =>
                self.on.insert(festival),
            Some(false) =>
                self.off.insert(festival),
            None => (),
        }
    }

    pub fn get_preference(&self, festival: Festival) -> Option<FestivalPreference> {
        if self.off.get(festival) {
            Some(false)
        } else if self.on.get(festival) {
            Some(true)
        } else {
            None
        }
    }

    pub fn get(&self) -> Festivals {
        (self.active | self.on) & !self.off
    }
}
