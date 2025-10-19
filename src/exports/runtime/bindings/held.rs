use {
    crate::exports::runtime::bindings::{
        ControlSlot,
        GameControl, GameControls,
        KeyPresses,
        TaimiControls,
    },
    std::{
        collections::BTreeMap,
        fmt,
        mem,
        sync::RwLock,
    },
    tokio::sync::watch,
    windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY,
};


#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WatcherSlot {
    Control {
        control: GameControl,
        index: u8,
    },
    Taimi {
        index: u8,
    },
}

impl WatcherSlot {
    pub fn control(&self) -> Option<GameControl> {
        match self {
            &Self::Control { control, .. } => Some(control),
            _ => None,
        }
    }

    /// Either a single bit, or [TaimiControls::empty()]
    pub fn taimi(&self) -> TaimiControls {
        match self {
            &Self::Taimi { index } => TaimiControls::from_index(index),
            _ => TaimiControls::empty(),
        }
    }
}

impl From<ControlSlot> for WatcherSlot {
    fn from((control, index): ControlSlot) -> Self {
        Self::Control {
            control,
            index,
        }
    }
}
impl From<TaimiControls> for WatcherSlot {
    fn from(controls: TaimiControls) -> Self {
        Self::Taimi {
            index: controls.index(),
        }
    }
}

/// TODO: BTreeSet would be fineish
pub type HeldControlsState = BTreeMap<WatcherSlot, u16>;
pub struct HeldControls {
    pub controls: watch::Sender<HeldControlsState>,
    pub interesting_keys: RwLock<KeyPresses>,
    pub interesting_controls: GameControls,
}

impl HeldControls {
    pub fn new(interesting_controls: GameControls) -> Self {
        Self {
            controls: watch::Sender::new(BTreeMap::new()),
            interesting_keys: RwLock::new(Default::default()),
            interesting_controls,
        }
    }

    pub fn is_interested_in_control(&self, control: GameControl) -> bool {
        self.interesting_controls.contains(control)
    }

    pub fn is_interested_in_key(&self, vk: VIRTUAL_KEY) -> bool {
        self.interesting_keys.read().ok()
            .and_then(|interesting| interesting.get(vk.0 as usize)
                .map(|b| *b)
            ).unwrap_or(false)
    }

    pub fn notify_release(&self, vk: VIRTUAL_KEY) {
        self.controls.send_if_modified(|controls| {
            let prev_len = controls.len();
            controls.retain(|_, &mut heldvk| heldvk != vk.0);
            prev_len != controls.len()
        });
    }

    pub fn notify_press<C: Into<WatcherSlot>>(&self, vk: VIRTUAL_KEY, control: C) {
        self.controls.send_modify(|controls| {
            // consider storing controls twice with slot index 0xff to track/ignore overlap better..?
            let control = control.into();
            let _prev = controls.insert(control, vk.0);
            if _prev.is_some() {
                log::debug!("held control {control:?} double-pressed with {vk:?}");
            }
        });
    }

    pub fn held_controls(controls: &HeldControlsState) -> GameControls {
        Self::collect_controls(controls.keys().filter_map(|&slot| slot.control()))
    }

    pub fn taimi_controls(controls: &HeldControlsState) -> TaimiControls {
        controls.keys().map(|&slot| slot.taimi()).collect()
    }

    pub fn collect_interesting_keys<B>(&self, binds: B) -> KeyPresses where
        B: IntoIterator<Item = (ControlSlot, VIRTUAL_KEY)>,
    {
        let mut interesting_binds = KeyPresses::default();
        for ((control, _index), vk) in binds {
            if !self.is_interested_in_control(control) { continue }
            if vk.0 == 0 || vk.0 >= 0xff { continue }
            unsafe {
                interesting_binds.set_unchecked(vk.0 as usize, true);
            }
        }
        interesting_binds
    }

    pub fn collect_controls<C>(controls: C) -> GameControls where
        C: IntoIterator<Item = GameControl>,
    {
        let mut interesting_controls = GameControls::default();
        for control in controls {
            interesting_controls.set(control, true);
        }
        interesting_controls
    }

    pub fn set_interesting_keys(&self, interesting_keys: KeyPresses) {
        if let Ok(mut out) = self.interesting_keys.write() {
            *out = interesting_keys;
        }
    }

    pub fn subscribe_controls(&self) -> ControlsReceiver {
        ControlsReceiver::new(self.controls.subscribe())
    }

    pub fn subscribe_taimi(&self) -> TaimiReceiver {
        TaimiReceiver::new(self.controls.subscribe())
    }
}

#[derive(Clone)]
pub struct ControlsReceiver {
    pub prev: GameControls,
    pub receiver: watch::Receiver<HeldControlsState>,
}

impl ControlsReceiver {
    pub fn new(receiver: watch::Receiver<HeldControlsState>) -> Self {
        Self {
            prev: Default::default(),
            receiver,
        }
    }

    pub fn mark_unchanged(&mut self) {
    }

    pub fn current(&self) -> &GameControls {
        &self.prev
    }

    #[cfg(todo = "unused")]
    pub fn latest(&self) -> GameControls {
        HeldControls::held_controls(&self.receiver.borrow())
    }

    pub async fn wait<'a>(&'a mut self) -> Result<(&'a GameControls, GameControls), watch::error::RecvError> {
        let mut latest = Default::default();
        let prev = &mut self.prev;
        self.receiver.wait_for(|held| {
            latest = HeldControls::held_controls(held);
            let prev = mem::replace(prev, latest);
            latest ^= prev;
            !latest.is_empty()
        }).await?;
        Ok((&*prev, latest))
    }

    pub fn update<'a>(&'a mut self) -> Option<(&'a GameControls, GameControls)> {
        let mut latest = HeldControls::held_controls(&*self.receiver.borrow_and_update());
        let prev = mem::replace(&mut self.prev, latest);
        latest ^= prev;
        (!latest.is_empty()).then_some((&self.prev, latest))
    }
}

impl fmt::Debug for ControlsReceiver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("ControlsReceiver")
            .field("prev", &self.prev)
            .finish()
    }
}

#[derive(Clone)]
pub struct TaimiReceiver {
    pub prev: TaimiControls,
    pub receiver: watch::Receiver<HeldControlsState>,
}

impl TaimiReceiver {
    pub fn new(receiver: watch::Receiver<HeldControlsState>) -> Self {
        Self {
            prev: Default::default(),
            receiver,
        }
    }

    pub fn mark_unchanged(&mut self) {
    }

    pub fn current(&self) -> &TaimiControls {
        &self.prev
    }

    #[cfg(todo = "unused")]
    pub fn latest(&self) -> TaimiControls {
        HeldControls::taimi_controls(&self.receiver.borrow())
    }

    pub async fn wait(&mut self) -> Result<(TaimiControls, TaimiControls), watch::error::RecvError> {
        let mut latest = Default::default();
        let prev = &mut self.prev;
        self.receiver.wait_for(|held| {
            latest = HeldControls::taimi_controls(held);
            let prev = mem::replace(prev, latest);
            latest ^= prev;
            !latest.is_empty()
        }).await?;
        Ok((*prev, latest))
    }

    pub fn update(&mut self) -> Option<(TaimiControls, TaimiControls)> {
        let mut latest = HeldControls::taimi_controls(&*self.receiver.borrow_and_update());
        let prev = mem::replace(&mut self.prev, latest);
        latest ^= prev;
        (!latest.is_empty()).then_some((self.prev, latest))
    }
}

impl fmt::Debug for TaimiReceiver {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("TaimiReceiver")
            .field("prev", &self.prev)
            .finish()
    }
}
