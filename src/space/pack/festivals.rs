use {
    std::{
        collections::BTreeMap,
        time::{Duration, SystemTime},
    },
    taimi_pack::attributes::Festival,
};

pub struct FestivalFixup;

impl FestivalFixup {
    pub fn festival_categories() -> BTreeMap<&'static str, Festival> {
        Self::FESTIVAL_CATEGORIES.iter().copied().collect()
    }

    pub const FESTIVAL_CATEGORIES: &'static [(&'static str, Festival)] = &[
        ("tw_guides.tw_festivals.tw_festival_wintersday", Festival::Wintersday),
        ("tw_guides.tw_festivals.tw_sab", Festival::SuperAdventureBox),
        ("tw_guides.tw_festivals.tw_festival_halloween", Festival::Halloween),
        ("tw_guides.tw_festivals.tw_festival_dragonbash", Festival::DragonBash),
        ("tw_guides.tw_festivals.tw_festival_fourwinds", Festival::FourWinds),
        ("tw_guides.tw_festivals.tw_festival_lunarnewyear", Festival::LunarNewYear),
        ("reactif.festivals.super_adventure_box", Festival::SuperAdventureBox),
        ("reactif.festivals.tribulation", Festival::SuperAdventureBox),
        ("reactif.festivals.interaction", Festival::SuperAdventureBox),
        ("reactif.festivals.hub", Festival::SuperAdventureBox),
        ("reactif.festivals.normal", Festival::SuperAdventureBox),
        ("reactif.festivals.halloween", Festival::Halloween),
        ("reactif.festivals.pumpkin_carving", Festival::Halloween),
        ("reactif.festivals.new_afterlife_for_quaggan", Festival::Halloween),
        ("reactif.festivals.mad_maleficence", Festival::Halloween),
        ("reactif.festivals.mad_mementos", Festival::Halloween),
        ("reactif.festivals.mad_mysteries", Festival::Halloween),
        ("reactif.festivals.mad_memorises", Festival::Halloween),
        ("reactif.festivals.mad_memorial", Festival::Halloween),
        ("reactif.festivals.mad_king_s_herald_in_training", Festival::Halloween),
        ("reactif.festivals.halloween_time_trial", Festival::Halloween),
        ("reactif.festivals.champion_of_the_labyrinth", Festival::Halloween),
        ("reactif.festivals.courtly_service", Festival::Halloween),
        ("reactif.festivals.firecracker_finder", Festival::LunarNewYear),
        ("reactif.festivals.lunar_new_year", Festival::LunarNewYear),
        ("reactif.festivals.lunar_new_year_adventure_1", Festival::LunarNewYear),
        ("reactif.festivals.lunar_new_year_adventure_2", Festival::LunarNewYear),
        ("reactif.festivals.lunar_new_year_race", Festival::LunarNewYear),
        ("reactif.festivals.wintersdaytimetrial", Festival::Wintersday),
        ("reactif.festivals.winter_wonderland_mastery", Festival::Wintersday),
        ("reactif.festivals.dauntless_donator", Festival::Wintersday),
        ("reactif.festivals.finders_keepers", Festival::Wintersday),
        ("reactif.festivals.golden_generosity", Festival::Wintersday),
        ("reactif.festivals.polar_bones", Festival::Wintersday),
        ("reactif.festivals.ogre_obliterator", Festival::Wintersday),
        ("reactif.festivals.pavilion_pursuer", Festival::Wintersday),
        ("reactif.festivals.ulla_s_rival", Festival::Wintersday),
        ("reactif.festivals.wintersday", Festival::Wintersday),
        ("reactif.festivals.crown_pavilion", Festival::Wintersday),
        ("reactif.festivals.burn_them_all", Festival::DragonBash),
        ("reactif.festivals.color_the_sky", Festival::DragonBash),
        ("reactif.festivals.dragonbash_race", Festival::DragonBash),
        ("reactif.festivals.paper_dragon", Festival::DragonBash),
        ("reactif.festivals.drooburt", Festival::DragonBash),
        ("reactif.festivals.adventure", Festival::DragonBash),
        ("reactif.festivals.adventure.pinata_bashing", Festival::DragonBash),
        ("reactif.festivals.adventure.time_trial", Festival::DragonBash),
        ("reactif.festivals.dragon_bash", Festival::DragonBash),
        ("reactif.festivals.aspect_gatherer", Festival::FourWinds),
        ("reactif.festivals.dolyak_flyer", Festival::FourWinds),
        ("reactif.festivals.man_overboard", Festival::FourWinds),
        ("reactif.festivals.master_aspect_gatherer", Festival::FourWinds),
        ("reactif.festivals.master_of_ceremony", Festival::FourWinds),
        ("reactif.festivals.no_one_likes_a_wet_charr", Festival::FourWinds),
        ("reactif.festivals.rise_and_fall_of_kookoochoo", Festival::FourWinds),
        ("reactif.festivals.slalom_skimmer", Festival::FourWinds),
        ("reactif.festivals.four_winds", Festival::FourWinds),
    ];
    pub const FESTIVAL_PREFIXES: &'static [&'static str] = &[
        "tw_guides.tw_festivals",
        "reactif.festivals.hidden",
        "reactif.festivals",
    ];

    /// Generally Tuesdays from $(date +%s -d '20??-??-??T09:00:00-07:00')
    /// to $(date +%s -d '20??-??-??T12:00:00-07:00')
    pub const FESTIVAL_WINDOWS: &'static [(Festival, FestivalWindow)] = &[
        // Shadow of the Mad King 2025: 2025-10-07 — 2025-11-04
        (Festival::Halloween, FestivalWindow::with_timestamp(
            1759852800,
            1762282800,
        )),
    ];
}

#[derive(Debug, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct FestivalWindow {
    pub start_timestamp: u64,
    pub end_timestamp: u64,
}

impl FestivalWindow {
    pub const fn with_timestamp(start_timestamp: u64, end_timestamp: u64) -> Self {
        match Self::try_with_timestamp(start_timestamp, end_timestamp) {
            Some(window) => window,
            None => panic!("festival duration cannot be negative"),
        }
    }

    pub const fn try_with_timestamp(start_timestamp: u64, end_timestamp: u64) -> Option<Self> {
        match end_timestamp > start_timestamp {
            true => Some(Self {
                start_timestamp,
                end_timestamp,
            }),
            false => None,
        }
    }

    pub fn duration(&self) -> Duration {
        Duration::from_secs(self.end_timestamp - self.start_timestamp)
    }

    pub fn start(&self) -> Option<SystemTime> {
        let start = Duration::from_secs(self.start_timestamp);
        SystemTime::UNIX_EPOCH.checked_add(start)
    }

        #[cfg(todo = "unused")]
    pub fn end(&self) -> Option<SystemTime> {
        let end = Duration::from_secs(self.end_timestamp);
        SystemTime::UNIX_EPOCH.checked_add(end)
    }

    pub fn is_active(&self, now: SystemTime) -> bool {
        let Some(start) = self.start() else {
            return false
        };

        #[cfg(todo = "unnecessary")]
        if now < start {
            return false
        } else {
            start.checked_add(self.duration())
                .map(|end| end > now)
                .unwrap_or(true)
        }

        match now.duration_since(start) {
            Ok(d) => d <= self.duration(),
            _ => false,
        }
    }
}
