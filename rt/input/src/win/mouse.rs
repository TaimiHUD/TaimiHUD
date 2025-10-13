use {
    anyhow::{anyhow, Context},
    crate::win::{
        keyboard::KeyState,
        window_message, window_send_inputs,
    },
    core::{iter, mem::transmute, num::NonZeroI32, ops, slice},
    windows::{
        core::Error as WinError,
        Win32::{
            Foundation::{HWND, LPARAM, POINT, ERROR_SUCCESS, SetLastError},
            Graphics::Gdi,
            UI::{
                HiDpi,
                Input::KeyboardAndMouse,
                WindowsAndMessaging,
            },
        },
    },
};
#[cfg(feature = "arcdps-extras")]
use arcdps::extras::{self, KeybindChange, MouseCode};

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

impl MousePosition {
    //pub const EMPTY: Self = Self::new(i32::MIN, i32::MIN);?
    pub const EMPTY: Self = Self::new(0, 0);

    pub const fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y
        }
    }

    pub const fn is_empty(&self) -> bool {
        matches!(*self, Self::EMPTY)
    }

    pub const fn to_point(self) -> POINT {
        unsafe {
            transmute(self)
        }
    }

    pub const fn as_point(&self) -> &POINT {
        unsafe {
            transmute(self)
        }
    }

    pub fn as_point_mut(&mut self) -> &mut POINT {
        unsafe {
            transmute(self)
        }
    }

    /// Normalize screen coordinate
    pub fn scale_to_primary(self) -> anyhow::Result<MousePosition> {
        let bounds = {
            let (w, h) = primary_screen_bounds()?;
            Self {
                x: w.get(),
                y: h.get(),
            }
        };
        Ok(match self * 0x10000i32 {
            normalized => (normalized + MousePosition { x: bounds.x / 2 - 1, y: bounds.y / 2 - 1 }) / bounds,
            #[cfg(todo)]
            normalized => normalized / bounds,
            #[cfg(todo)]
            normalized => Self {
                x: (normalized.x as f32 / bounds.x as f32).round() as i32,
                y: (normalized.y as f32 / bounds.y as f32).round() as i32,
            },
        })
    }

    pub fn to_window(mut self, wnd: HWND) -> anyhow::Result<MousePosition> {
        let res = unsafe {
            SetLastError(ERROR_SUCCESS);
            // or Gdi::ClientToScreen?
            Gdi::MapWindowPoints(Some(WindowsAndMessaging::HWND_DESKTOP), Some(wnd), slice::from_mut(self.as_point_mut()))
        };
        match res {
            0 => match WinError::from_win32() {
                e if e.code().0 == 0 =>
                    Ok(self),
                e => Err(e),
            },
            _ => Ok(self),
        }.context("MapWindowPoints")
    }

    pub fn to_screen(mut self, wnd: HWND) -> anyhow::Result<MousePosition> {
        let res = unsafe {
            SetLastError(ERROR_SUCCESS);
            Gdi::MapWindowPoints(Some(wnd), Some(WindowsAndMessaging::HWND_DESKTOP), slice::from_mut(self.as_point_mut()))
        };
        match res {
            0 => match WinError::from_win32() {
                e if e.code().0 == 0 =>
                    Ok(self),
                e => Err(e),
            },
            _ => Ok(self),
        }.context("MapWindowPoints")
    }

    pub fn to_input(self, hwnd: HWND) -> anyhow::Result<KeyboardAndMouse::INPUT> {
        MouseInput::from(self).to_input(hwnd)
    }
}

impl<P: Into<MousePosition>> ops::Sub<P> for MousePosition {
    type Output = Self;

    fn sub(self, rhs: P) -> Self {
        let rhs = rhs.into();
        Self {
            x: self.x.saturating_sub(rhs.x),
            y: self.y.saturating_sub(rhs.y),
        }
    }
}
impl<P> ops::SubAssign<P> for MousePosition where
    Self: ops::Sub<P>,
    <MousePosition as ops::Sub<P>>::Output: Into<Self>
{
    fn sub_assign(&mut self, rhs: P) {
        *self = (*self - rhs).into();
    }
}

impl<P: Into<MousePosition>> ops::Add<P> for MousePosition {
    type Output = Self;

    fn add(self, rhs: P) -> Self {
        let rhs = rhs.into();
        Self {
            x: self.x.saturating_add(rhs.x),
            y: self.y.saturating_add(rhs.y),
        }
    }
}
impl<P> ops::AddAssign<P> for MousePosition where
    Self: ops::Add<P>,
    <MousePosition as ops::Add<P>>::Output: Into<Self>
{
    fn add_assign(&mut self, rhs: P) {
        *self = (*self + rhs).into();
    }
}

impl<P: Into<MousePosition>> ops::Div<P> for MousePosition {
    type Output = Self;

    fn div(self, rhs: P) -> Self {
        let rhs = rhs.into();
        Self {
            x: self.x.checked_div(rhs.x).unwrap_or(0),
            y: self.y.checked_div(rhs.y).unwrap_or(0),
        }
    }
}
impl<P> ops::DivAssign<P> for MousePosition where
    Self: ops::Div<P>,
    <MousePosition as ops::Div<P>>::Output: Into<Self>
{
    fn div_assign(&mut self, rhs: P) {
        *self = (*self / rhs).into();
    }
}

impl<P: Into<MousePosition>> ops::Mul<P> for MousePosition {
    type Output = Self;

    fn mul(self, rhs: P) -> Self {
        let rhs = rhs.into();
        Self {
            x: self.x.saturating_mul(rhs.x),
            y: self.y.saturating_mul(rhs.y),
        }
    }
}
impl<P> ops::MulAssign<P> for MousePosition where
    Self: ops::Mul<P>,
    <MousePosition as ops::Mul<P>>::Output: Into<Self>
{
    fn mul_assign(&mut self, rhs: P) {
        *self = (*self * rhs).into();
    }
}

impl From<POINT> for MousePosition {
    fn from(POINT { x, y }: POINT) -> Self {
        Self {
            x,
            y,
        }
    }
}

impl From<i32> for MousePosition {
    fn from(mag: i32) -> Self {
        Self {
            x: mag,
            y: mag,
        }
    }
}

impl From<MousePosition> for POINT {
    fn from(MousePosition { x, y }: MousePosition) -> Self {
        POINT {
            x,
            y,
        }
    }
}

impl From<LPARAM> for MousePosition {
    fn from(l: LPARAM) -> Self {
        Self {
            y: l.0 as i16 as i32,
            x: ((l.0 as usize & 0xffff0000) >> 16) as i16 as i32
        }
    }
}

impl From<MousePosition> for LPARAM {
    fn from(pos: MousePosition) -> Self {
        let x = (pos.x << 16) as u32;
        let y = pos.y as u16;
        LPARAM(x as isize | y as isize)
    }
}

impl From<MousePosition> for isize {
    fn from(pos: MousePosition) -> Self {
        LPARAM::from(pos).0
    }
}

impl From<isize> for MousePosition {
    fn from(l: isize) -> Self {
        Self::from(LPARAM(l))
    }
}

#[cfg(todo)]
impl From<Point2<ScreenPoint>> for MousePosition {
    fn from(point: glam::Vec2) -> Self {
        let mut display_size = None;
        #[cfg(feature = "markers")]
        if let Some(mid) = MarkerInputData::read() {
            display_size.get_or_insert(mid.display_size.to_array());
        }
        if let Some(sz) = crate::RENDER_STATE.try_lock().ok().and_then(|state| state.as_ref().and_then(|state| state.last_display_size)) {
            display_size.get_or_insert(sz);
        }
        let [w, h] = match display_size {
            Some([0.0f32, 0.0f32]) => panic!("screen size missing"),
            sz => sz.expect("screen size unknown"),
        };
        Self {
            x: (point.x * w).round() as i32,
            y: (point.y * h).round() as i32,
        }
    }
}

#[cfg(todo)]
impl From<ScreenPoint> for MousePosition {
    fn from(point: ScreenPoint) -> Self {
        Self {
            x: point.x.round() as i32,
            y: point.y.round() as i32,
        }
    }
}

pub fn primary_screen_bounds() -> anyhow::Result<(NonZeroI32, NonZeroI32)> {
    let x = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) };
    let y = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) };
    match (NonZeroI32::new(x), NonZeroI32::new(y)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => anyhow::bail!("GetSystemMetrics(SM_CYSCREEN) produced nothing"),
    }
}

pub fn virtual_screen_bounds() -> anyhow::Result<(NonZeroI32, NonZeroI32)> {
    let x = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXVIRTUALSCREEN) };
    let y = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYVIRTUALSCREEN) };
    match (NonZeroI32::new(x), NonZeroI32::new(y)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => anyhow::bail!("GetSystemMetrics(SM_CYVIRTUALSCREEN) produced nothing"),
    }
}

// TODO: GetDpiAwarenessContextForProcess, GetDpiForSystem, etc?
pub fn window_dpi(hwnd: HWND) -> anyhow::Result<u32> {
    let dpi = unsafe {
        HiDpi::GetDpiForWindow(hwnd)
    };
    match dpi {
        0 => Err(anyhow!("GetDpiForWindow")),
        dpi => Ok(dpi),
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MouseInput {
    pub position: MousePosition,
    pub button: KeyState,
    pub down: Option<bool>,
}

impl MouseInput {
    pub const fn with_position(position: MousePosition) -> Self {
        Self {
            position,
            button: KeyState::EMPTY,
            down: None,
        }
    }

    pub const fn with_button(button: KeyState) -> Self {
        Self {
            position: MousePosition::EMPTY,
            button,
            down: None,
        }
    }

    pub const fn new(position: MousePosition, button: KeyState, down: Option<bool>) -> Self {
        Self {
            position,
            button,
            down,
        }
    }

    pub const fn to_movement(self) -> Self {
        Self::new(self.position, self.button, None)
    }

    pub const fn is_movement(&self) -> bool {
        self.down.is_none() || !self.button.intersects(KeyState::BUTTON)
    }

    pub const fn buttons(&self) -> KeyState {
        KeyState::from_bits_retain(self.button.bits() & KeyState::BUTTON.bits())
    }

    pub const fn mods(&self) -> KeyState {
        KeyState::from_bits_retain(self.button.bits() & !KeyState::BUTTON.bits())
    }

    pub const fn button_after(&self) -> KeyState {
        match self.down {
            Some(false) => self.mods(),
            _ => self.button,
        }
    }

    pub const fn button_before(&self) -> KeyState {
        match self.down {
            Some(true) => self.mods(),
            _ => self.button,
        }
    }

    pub fn input_buttons(self) -> impl Iterator<Item = Self> + Clone + Send + Sync + 'static {
        let Self { position, button, down } = self;
        let buttons = button & KeyState::BUTTON;
        let mods = button & !KeyState::BUTTON;
        buttons.iter_keys()
            .map(move |b| Self::new(position, b | mods, down))
    }

    pub fn to_input(self, hwnd: HWND) -> anyhow::Result<KeyboardAndMouse::INPUT> {
        let flag_move = KeyboardAndMouse::MOUSEEVENTF_MOVE | KeyboardAndMouse::MOUSEEVENTF_MOVE_NOCOALESCE;
        let flag_button = self.down.and_then(|down| self.button.mouse_flag(down)).unwrap_or_default();
        let xdata = match flag_button {
            flag if (flag & (KeyboardAndMouse::MOUSEEVENTF_XDOWN | KeyboardAndMouse::MOUSEEVENTF_XUP)).0 != 0 =>
                self.button.button_x(),
            _ => 0,
        };
        let relative_to = ();
        let (flag_abs, MousePosition { x: dx, y: dy }) = match relative_to {
            // XXX: relative applies thresholds and mouse speed multipliers, do not want
            #[cfg(todo)]
            Some(rel) => (0, self - rel),
            _ => {
                let position = self.position.to_screen(hwnd)
                    .and_then(|pos| pos.scale_to_primary())?;
                (
                    KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE,
                    position
                )
            },
        };
        Ok(KeyboardAndMouse::INPUT {
            r#type: KeyboardAndMouse::INPUT_MOUSE,
            Anonymous: KeyboardAndMouse::INPUT_0 {
                mi: KeyboardAndMouse::MOUSEINPUT {
                    dx,
                    dy,
                    mouseData: xdata,
                    time: 0,
                    dwFlags: flag_button | flag_abs | flag_move,
                    dwExtraInfo: 0,
                },
            },
        })
    }

    pub const EVENT_MODS: KeyState = KeyState::from_bits_retain(KeyState::CTRL.bits() | KeyState::SHIFT.bits());
    pub fn to_event(self) -> Option<(u32, usize, isize)> {
        let button = self.buttons();

        let msg = match self.down {
            Some(down) if !self.is_movement() =>
                button.event_msg(down),
            _ => WindowsAndMessaging::WM_MOUSEMOVE,
        };

        let w = button.event_w(msg) | self.button_after().to_modifierkeys().0 as usize;

        let l = LPARAM::from(self.position);

        Some((msg, w, l.0))
    }

    pub fn to_events(self, prior: Option<Self>) -> impl Iterator<Item = (u32, usize, isize)> {
        let movement = match (self.is_movement(), &prior) {
            (_, Some(prior)) if self.position == prior.position => None,
            (false, None) => None,
            //(false, Some(prior)) => None,
            /*(true_, _)*/ _ => self.to_event(),
        };
        let mut before = prior.as_ref().map(|p| p.button_after()).unwrap_or_else(|| self.button_before());
        let after = self.button_after();
        let changes = after ^ before;
        let events = changes.iter_keys()
            .filter_map(move |button| {
                if !button.intersects(KeyState::BUTTON) {
                    return None
                }
                let input = Self::new(self.position, button, self.down);
                let (msg, _w, l) = input.to_event()?;
                if let Some(down) = self.down {
                    before.set(button, down);
                }
                let w = button.event_w(msg) | before.to_modifierkeys().0 as usize;
                Some((msg, w, l))
            });
        movement.into_iter()
            .chain(events)
    }
}

impl From<MousePosition> for MouseInput {
    fn from(position: MousePosition) -> Self {
        Self::with_position(position)
    }
}

impl From<KeyState> for MouseInput {
    fn from(button: KeyState) -> Self {
        Self::with_button(button)
    }
}

#[cfg(feature = "arcdps-extras")]
impl TryFrom<KeybindChange> for MouseInput {
    type Error = anyhow::Error;

    fn try_from(key: KeybindChange) -> Result<Self, Self::Error> {
        let mods = KeyState::from(&key);
        let button = match key.key {
            extras::Key::Mouse(code) => KeyState::try_from(code),
            key => Err(anyhow::anyhow!("not a mouse binding: {key:?}")),
        }?;
        Ok(Self::with_button(button | mods))
    }
}

#[cfg(feature = "arcdps-extras")]
impl TryFrom<MouseCode> for MouseInput {
    type Error = anyhow::Error;

    fn try_from(code: MouseCode) -> Result<Self, Self::Error> {
        KeyState::try_from(code).map(Self::with_button)
    }
}


#[cfg(todo)]
impl From<MouseInput> for KeyboardAndMouse::INPUT {
    fn from(input: MouseInput) -> Self {
        input.to_input(hwnd)
            .expect("failed to determine screen coordinates")
    }
}

#[cfg(todo)]
impl From<MousePosition> for KeyboardAndMouse::INPUT {
    fn from(position: MousePosition) -> Self {
        MouseInput::from(position).to_input(hwnd)
            .expect("failed to determine screen coordinates")
    }
}

pub fn screen_position() -> anyhow::Result<MousePosition> {
    let mut out = MousePosition::default();
    unsafe {
        WindowsAndMessaging::GetCursorPos(out.as_point_mut())
    }.context("GetCursorPos")
    .map(move |()| out)
}

pub fn send_mouse(hwnd: HWND, input: MouseInput, prior: Option<MouseInput>) -> anyhow::Result<()> {
    let mut sent = false;
    let mut error = None;
    for (msg, w, l) in input.to_events(prior) {
        sent = true;
        let res = unsafe {
            window_message(hwnd, msg, w, l)
        };
        if let Err(e) = res {
            let _ = error.insert(e);
        }
    }
    error.map(Err).unwrap_or(match sent {
        true => Ok(()),
        false => Err(anyhow!("empty or unsupported mouse input")),
    })
}

pub fn send_input<I: Into<MouseInput>>(hwnd: HWND, input: I) -> anyhow::Result<()> {
    let input = input.into();
    let input = input.to_input(hwnd)?;
    window_send_inputs(hwnd, iter::once_with(move || input))
}
