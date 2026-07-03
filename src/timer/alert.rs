use {
    crate::timer::{BlishColour, SharedTimeOffsets, TimerFilePhase},
    relative_path::RelativePathBuf,
    serde::{Deserialize, Serialize},
    std::ops::Deref,
    strum::Display,
    tokio::time::{Duration, Instant},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlishAlert {
    #[serde(default)]
    pub warning_duration: Option<f32>,
    #[serde(default)]
    pub alert_duration: Option<f32>,
    #[serde(default)]
    pub warning: Option<String>,
    #[serde(default)]
    pub warning_color: Option<BlishColour>,
    #[serde(default)]
    pub alert: Option<String>,
    #[serde(default)]
    pub alert_color: Option<BlishColour>,
    #[serde(default)]
    pub icon: Option<RelativePathBuf>,
    #[serde(default)]
    pub fill_color: Option<BlishColour>,
    #[serde(default)]
    pub timestamps: Vec<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uid: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Display, Copy)]
pub enum TimerAlertType {
    Alert,
    Warning,
}

impl BlishAlert {
    #[inline]
    pub fn offset_set(&self) -> &str {
        self.set
            .as_ref()
            .map(|s| &s[..])
            .unwrap_or(super::TimerAction::DEFAULT_SET)
    }
}

#[derive(Debug, Clone)]
pub struct TimerAlert {
    pub file_alert: TimerFileAlert,
    pub kind: TimerAlertType,
    pub timestamp: f32,
}

impl TimerAlert {
    pub fn fill_colour(&self) -> &Option<BlishColour> {
        &self.fill_color
    }
    pub fn text(&self) -> &str {
        match self.kind {
            TimerAlertType::Warning => self.warning.as_ref(),
            TimerAlertType::Alert => self.alert.as_ref(),
        }
        .map(|text| &text[..])
        .unwrap_or("")
    }
    pub fn colour(&self) -> &Option<BlishColour> {
        match self.kind {
            TimerAlertType::Warning => &self.warning_color,
            TimerAlertType::Alert => &self.alert_color,
        }
    }
    pub fn raw_duration(&self) -> f32 {
        match self.kind {
            TimerAlertType::Warning => self.warning_duration,
            TimerAlertType::Alert => self.alert_duration,
        }
        .unwrap_or(0.0f32)
    }
    pub fn alert_duration(&self) -> Duration {
        Duration::from_secs_f32(self.raw_duration())
    }
    pub fn end_timestamp(&self) -> Duration {
        Duration::from_secs_f32(self.timestamp)
    }
    pub fn start_timestamp(&self) -> Duration {
        self.end_timestamp().saturating_sub(self.alert_duration())
    }
    pub(super) fn end(&self, phase_start: Instant) -> Instant {
        phase_start + self.end_timestamp()
    }
    pub(super) fn start(&self, phase_start: Instant) -> Instant {
        phase_start + self.start_timestamp()
    }
    pub fn percentage(&self, offsets: &SharedTimeOffsets, start: Instant) -> Option<f32> {
        let start = offsets.adjust_start_for(self.offset_set(), start);
        let elapsed = Instant::now()
            .checked_duration_since(self.start(start))?
            .as_secs_f32();
        let duration = self.raw_duration();
        if elapsed > duration {
            None
        } else {
            Some(elapsed / duration)
        }
    }
    pub fn remaining(&self, start: Instant) -> Duration {
        self.end(start).saturating_duration_since(Instant::now())
    }
    pub fn progress_bar_text(&self, offsets: &SharedTimeOffsets, start: Instant) -> String {
        let start = offsets.adjust_start_for(self.offset_set(), start);
        format!("{} - in {:.1}s", self.text(), self.remaining(start).as_secs_f32())
    }
}
impl Deref for TimerAlert {
    type Target = TimerFileAlert;
    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.file_alert
    }
}

#[derive(Debug, Clone)]
pub struct TimerFileAlert {
    pub(super) phase: TimerFilePhase,
    pub(super) alert_idx: usize,
}
impl TimerFileAlert {
    #[inline]
    pub fn new(phase: TimerFilePhase, alert_idx: usize) -> Option<Self> {
        (phase.alerts.len() > alert_idx).then(|| Self { phase, alert_idx })
    }
    #[inline]
    pub fn as_alert(&self) -> &BlishAlert {
        unsafe { self.phase.alerts.get_unchecked(self.alert_idx) }
    }

    pub fn to_alerts(self, timestamp: f32) -> [Option<TimerAlert>; 2] {
        let is_warning = self.warning.is_some() && self.warning_duration.is_some();
        let is_alert = self.alert.is_some() && self.alert_duration.is_some();
        let warning = is_warning.then(|| TimerAlert {
            kind: TimerAlertType::Warning,
            file_alert: self.clone(),
            timestamp,
        });
        let alert = is_alert.then(|| TimerAlert {
            kind: TimerAlertType::Alert,
            file_alert: self.clone(),
            timestamp,
        });
        [warning, alert]
    }
    pub fn get_alerts(&self) -> impl Iterator<Item = TimerAlert> + '_ {
        self.as_alert()
            .timestamps
            .iter()
            .flat_map(move |&timestamp| self.clone().to_alerts(timestamp))
            .flatten()
    }
    pub fn fan_out(self) -> impl Iterator<Item = TimerAlert> + 'static {
        (0..self.timestamps.len())
            .flat_map(move |ts_idx| {
                self.clone()
                    .to_alerts(unsafe { *self.timestamps.get_unchecked(ts_idx) })
            })
            .flatten()
    }
}
impl Deref for TimerFileAlert {
    type Target = BlishAlert;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_alert()
    }
}
#[cfg(todo)]
impl Iterator for TimerFileAlert {
    type Item = Self;
    fn next(&mut self) -> Option<Self> {
        match self.len() {
            0..=1 => {
                self.reset();
                None
            },
            _ => {
                let alert = self.clone();
                self.alert_idx += 1;
                Some(alert)
            },
        }
    }
}
#[cfg(todo)]
impl ExactSizeIterator for TimerFileAlert {
    fn len(&self) -> usize {
        self.phase.alerts.len() - self.alert_idx
    }
}
