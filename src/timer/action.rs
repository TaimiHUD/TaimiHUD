use {
    crate::timer::{TimerFilePhase, TimerTrigger},
    serde::{Deserialize, Serialize},
    std::{ops::Deref, time::Duration},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimerAction {
    pub name: String,
    #[serde(rename = "type", default)]
    pub kind: TimerActionType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sets: Option<Vec<String>>,
    pub trigger: TimerTrigger,
    #[serde(rename = "type", default)]
    pub time: Option<f32>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum TimerActionType {
    SkipTime,
}

impl Default for TimerActionType {
    fn default() -> Self {
        Self::SkipTime
    }
}

impl TimerAction {
    pub const DEFAULT_SET: &'static str = "default";

    /// TODO: wrap this or change to enum with `tag="type"`
    pub fn as_skip_time(&self) -> Option<(Duration, &[String])> {
        match self.kind {
            TimerActionType::SkipTime => Some((
                Duration::from_secs_f32(self.time?),
                self.sets.as_ref().map(|sets| &sets[..]).unwrap_or(&[]),
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TimerFileAction {
    phase: TimerFilePhase,
    action_idx: usize,
}
impl TimerFileAction {
    #[inline]
    pub fn new(phase: TimerFilePhase, action_idx: usize) -> Option<Self> {
        (phase.actions.len() > action_idx).then(|| Self { phase, action_idx })
    }

    #[inline]
    pub fn as_action(&self) -> &TimerAction {
        unsafe { self.phase.actions.get_unchecked(self.action_idx) }
    }
}
impl Deref for TimerFileAction {
    type Target = TimerAction;
    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_action()
    }
}
