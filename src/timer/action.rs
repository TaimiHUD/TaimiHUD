use {
    crate::timer::TimerTrigger,
    serde::{Deserialize, Serialize},
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
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub enum TimerActionType {
    SkipTime,
}

impl Default for TimerActionType {
    fn default() -> Self {
        Self::SkipTime
    }
}
