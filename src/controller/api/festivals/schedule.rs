use {
    std::time::{Duration, SystemTime},
    taimi_pack::attributes::{Festival, Festivals},
};

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
            true => Some(Self { start_timestamp, end_timestamp }),
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
        let Some(start) = self.start() else { return false };

        #[cfg(todo = "unnecessary")]
        if now < start {
            return false
        } else {
            start
                .checked_add(self.duration())
                .map(|end| end > now)
                .unwrap_or(true)
        }

        match now.duration_since(start) {
            Ok(d) => d <= self.duration(),
            _ => false,
        }
    }

    /// [self.active_festivals](SystemTime::now)
    pub fn current_festivals() -> Festivals {
        Self::active_festivals(SystemTime::now())
    }
    /// For use as an initial setting, not to be considered reliable
    pub fn active_festivals(now: SystemTime) -> Festivals {
        Self::FESTIVAL_WINDOWS
            .iter()
            .filter(move |(_f, window)| window.is_active(now))
            .map(|&(festival, ..)| Festivals::from(festival))
            .collect()
    }

    /// Generally Tuesdays from $(date +%s -d '20??-??-??T09:00:00-07:00')
    /// to $(date +%s -d '20??-??-??T12:00:00-07:00')
    pub const FESTIVAL_WINDOWS: &'static [(Festival, FestivalWindow)] = &[
        // Shadow of the Mad King 2025: 2025-10-07 — 2025-11-04
        (
            Festival::Halloween,
            FestivalWindow::with_timestamp(1759852800, 1762282800),
        ),
        // Wintersday 2025: 2025-12-09 — 2026-01-02
        (
            Festival::Wintersday,
            FestivalWindow::with_timestamp(1765296000, 1767376800),
        ),
    ];
}
