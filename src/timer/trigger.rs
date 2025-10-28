use {
    super::TimerKeybinds,
    crate::timer::{BlishPosition, Polytope, Position},
    serde::{Deserialize, Serialize},
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TimerTrigger {
    #[serde(rename = "type", default)]
    pub kind: TimerTriggerType,
    #[serde(alias = "keyBinds")]
    pub key_bind: Option<String>,
    pub position: Option<BlishPosition>,
    pub antipode: Option<BlishPosition>,
    pub radius: Option<f32>,
    #[serde(default)]
    pub require_combat: bool,
    #[serde(default)]
    pub require_out_of_combat: bool,
    #[serde(default)]
    pub require_entry: bool,
    #[serde(default)]
    pub require_departure: bool,
}

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum CombatState {
    Outside,
    Entered,
    Exited,
}

impl TimerTrigger {
    #[allow(dead_code)]
    pub fn position(&self) -> Option<Position> {
        self.position.map(Into::into)
    }

    #[allow(dead_code)]
    pub fn antipode(&self) -> Option<Position> {
        self.antipode.map(Into::into)
    }

    pub fn polytope(&self) -> Option<Polytope> {
        match *self {
            Self {
                radius: Some(radius),
                position: Some(center),
                ..
            } => Some(Polytope::NSphere {
                radius,
                center: center.into(),
            }),
            Self {
                antipode: Some(antipode),
                position: Some(pode),
                ..
            } => Some(Polytope::NCuboid {
                antipode: antipode.into(),
                pode: pode.into(),
            }),
            _ => None,
        }
    }
    pub fn check(&self, pos: Position, cb: CombatState, key_pressed: &mut TimerKeybinds) -> bool {
        let shape = match self.polytope() {
            Some(s) => s,
            None => return false,
        };
        use TimerTriggerType::*;
        let key_check = match self.kind {
            Location => true,
            Key => {
                if let Some(key_bind) = &self.key_bind {
                    let idx = key_bind.parse::<usize>().unwrap();
                    let flag = 1u8 << idx;
                    let flaggy = TimerKeybinds::from_bits_retain(flag);
                    let result = key_pressed.contains(flaggy.clone());
                    key_pressed.remove(flaggy);
                    result
                } else {
                    unreachable!("keybind not specified for a key type phase trigger");
                }
            },
        };
        let position_check = shape.point_is_within(pos);
        let combat_entered_check = !self.require_combat || cb == CombatState::Entered;
        let combat_exited_check = !self.require_out_of_combat || cb == CombatState::Exited;
        let combat_check = combat_entered_check && combat_exited_check;
        let entry_check = !self.require_entry || position_check;
        let departure_check = !self.require_departure || !position_check;
        let complete = entry_check && departure_check && combat_check && key_check;
        complete
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum TimerTriggerType {
    Location,
    Key,
}

impl Default for TimerTriggerType {
    fn default() -> Self {
        Self::Location
    }
}
