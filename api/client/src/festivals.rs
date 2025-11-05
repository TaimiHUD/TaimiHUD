use gw2lib_model::achievements::categories::AchievementCategoryId;

pub struct FestivalCategory(pub AchievementCategoryId);
impl FestivalCategory {
    pub const HALLOWEEN: Self = Self(79);
    pub const WINTERSDAY: Self = Self(98);
    pub const SUPER_ADVENTURE_BOX: Self = Self(162);
    pub const LUNAR_NEW_YEAR: Self = Self(201);
    pub const FOUR_WINDS: Self = Self(213);
    pub const DRAGON_BASH: Self = Self(233);
}
