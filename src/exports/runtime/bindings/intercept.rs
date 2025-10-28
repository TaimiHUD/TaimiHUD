use {
    std::sync::atomic::{AtomicU64, Ordering},
    taimi_input::win::keyboard::{KeyInput, KeyState},
    windows::Win32::UI::Input::KeyboardAndMouse,
};

pub enum KeyIntercept {
    Pending,
    Intercepted { key: KeyInput },
}

static KEY_INTERCEPT: AtomicU64 = AtomicU64::new(KeyIntercept::NONE);
impl KeyIntercept {
    const NONE: u64 = 0;
    const PENDING: u64 = u64::MAX;
    const DOWN: u64 = 0x1_00000000_0000;

    pub fn raw(&self) -> u64 {
        match self {
            Self::Pending => Self::PENDING,
            Self::Intercepted { key } => {
                let vk = key.vk.0 as u64;
                let mods = (key.mods.bits() as u64) << 16;
                let down = match key.down {
                    true => Self::DOWN,
                    false => 0,
                };
                vk as u64 | mods | down
            },
        }
    }

    pub fn from_raw(raw: u64) -> Option<Self> {
        Some(match raw {
            0 => return None,
            Self::PENDING => Self::Pending,
            raw => Self::Intercepted {
                key: KeyInput {
                    vk: KeyboardAndMouse::VIRTUAL_KEY(raw as u16),
                    mods: KeyState::from_bits_retain((raw >> 16) as u32),
                    down: raw & Self::DOWN != 0,
                },
            },
        })
    }

    pub fn intercept_restart() {
        KEY_INTERCEPT.store(Self::PENDING, Ordering::SeqCst);
    }

    pub fn intercept_take() -> Option<Self> {
        let mut raw = KEY_INTERCEPT.load(Ordering::SeqCst);
        loop {
            let int = match Self::from_raw(raw) {
                res @ (None | Some(Self::Pending)) => return res,
                int => int,
            };
            match KEY_INTERCEPT.compare_exchange_weak(
                raw,
                Self::NONE,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(..) => break int,
                Err(current) => {
                    raw = current;
                },
            }
        }
    }

    pub fn intercept_ready() -> bool {
        KEY_INTERCEPT.load(Ordering::Relaxed) == Self::PENDING
    }

    #[cfg(todo)]
    pub fn intercept_read() -> Option<Self> {
        Self::from_raw(KEY_INTERCEPT.load(Ordering::Relaxed))
    }

    pub fn intercept_report(key: KeyInput) {
        let int = Self::Intercepted { key };
        KEY_INTERCEPT.store(int.raw(), Ordering::SeqCst);
    }

    pub fn intercept_try_report(key: KeyInput) -> bool {
        let int = Self::Intercepted { key };
        KEY_INTERCEPT
            .compare_exchange(
                Self::PENDING,
                int.raw(),
                Ordering::SeqCst,
                Ordering::Relaxed,
            )
            .is_ok()
    }
}
