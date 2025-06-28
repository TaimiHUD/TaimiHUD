use {
    crate::exports::runtime::{self as rt, keyboard::KeyState, RuntimeResult},
    std::{iter, mem::transmute, ops, slice},
    windows::Win32::{
        Foundation::{LPARAM, POINT, ERROR_SUCCESS, SetLastError, GetLastError},
        Graphics::Gdi,
        UI::{
            WindowsAndMessaging,
            Input::KeyboardAndMouse,
        },
    },
};
#[cfg(feature = "markers")]
use crate::marker::atomic::ScreenPoint;
#[cfg(feature = "extension-arcdps")]
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

    #[cfg(todo)]
    pub fn scale_to_primary(mut self) -> RuntimeResult<MousePosition> {
        let bounds = {
            let (w, h) = primary_screen_bounds()?;
            Self {
                x: w.get(),
                y: h.get(),
            }
        };
        Ok(self / bounds)
    }

    pub fn to_screen(mut self) -> RuntimeResult<MousePosition> {
        let wnd = rt::window_handle()?;
        let res = unsafe {
            SetLastError(ERROR_SUCCESS);
            // or Gdi::ClientToScreen?
            Gdi::MapWindowPoints(Some(WindowsAndMessaging::HWND_DESKTOP), Some(wnd), slice::from_mut(self.as_point_mut()))
        };
        match res {
            0 => unsafe { GetLastError() }.ok()
                .map_err(|_| "MapWindowPoints failed")
                .map(|()| self),
            _ => Ok(self),
        }
    }

    pub fn to_window(mut self) -> RuntimeResult<MousePosition> {
        let wnd = rt::window_handle()?;
        let res = unsafe {
            SetLastError(ERROR_SUCCESS);
            Gdi::MapWindowPoints(Some(wnd), Some(WindowsAndMessaging::HWND_DESKTOP), slice::from_mut(self.as_point_mut()))
        };
        match res {
            0 => unsafe { GetLastError() }.ok()
                .map_err(|_| "MapWindowPoints failed")
                .map(|()| self),
            _ => Ok(self),
        }
    }

    pub fn to_input(self) -> KeyboardAndMouse::INPUT {
        MouseInput::from(self).to_input()
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

impl From<POINT> for MousePosition {
    fn from(POINT { x, y }: POINT) -> Self {
        Self {
            x,
            y,
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

impl From<glam::Vec2> for MousePosition {
    fn from(point: glam::Vec2) -> Self {
        Self {
            x: (0x10000 as f32 * point.x) as i32,
            y: (0x10000 as f32 * point.y) as i32,
        }
    }
}

#[cfg(feature = "markers")]
impl From<ScreenPoint> for MousePosition {
    fn from(point: ScreenPoint) -> Self {
        Self {
            x: (0x10000 as f32 * point.x) as i32,
            y: (0x10000 as f32 * point.y) as i32,
        }
    }
}

#[cfg(todo)]
pub fn primary_screen_bounds() -> RuntimeResult<(NonZeroI32, NonZeroI32)> {
    let x = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXSCREEN) };
    let y = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYSCREEN) };
    match (NonZeroI32::new(x), NonZeroI32::new(y)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(rt::RT_UNAVAILABLE),
    }
}

#[cfg(todo)]
pub fn virtual_screen_bounds() -> RuntimeResult<(NonZeroI32, NonZeroI32)> {
    let x = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CXVIRTUALSCREEN) };
    let y = unsafe { WindowsAndMessaging::GetSystemMetrics(WindowsAndMessaging::SM_CYVIRTUALSCREEN) };
    match (NonZeroI32::new(x), NonZeroI32::new(y)) {
        (Some(x), Some(y)) => Ok((x, y)),
        _ => Err(rt::RT_UNAVAILABLE),
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

    pub fn to_input(self) -> KeyboardAndMouse::INPUT {
        let flag_move = KeyboardAndMouse::MOUSEEVENTF_MOVE | KeyboardAndMouse::MOUSEEVENTF_MOVE_NOCOALESCE;
        let flag_button = self.down.and_then(|down| self.button.mouse_flag(down)).unwrap_or_default();
        let xdata = match flag_button {
            flag if (flag & (KeyboardAndMouse::MOUSEEVENTF_XDOWN | KeyboardAndMouse::MOUSEEVENTF_XUP)).0 != 0 =>
                self.button.button_x(),
            _ => 0,
        };
        #[cfg(todo)]
        let (flag_abs, Self { x: dx, y: dy }) = match relative_to {
            // XXX: relative applies thresholds and mouse speed multipliers, do not want
            Some(rel) => (0, self - rel),
            None => (KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE, self),
        };
        let flag_abs = KeyboardAndMouse::MOUSEEVENTF_ABSOLUTE;
        KeyboardAndMouse::INPUT {
            r#type: KeyboardAndMouse::INPUT_MOUSE,
            Anonymous: KeyboardAndMouse::INPUT_0 {
                mi: KeyboardAndMouse::MOUSEINPUT {
                    dx: self.position.x,
                    dy: self.position.y,
                    mouseData: xdata,
                    time: 0,
                    dwFlags: flag_button | flag_abs | flag_move,
                    dwExtraInfo: 0,
                },
            },
        }
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

#[cfg(feature = "extension-arcdps")]
impl TryFrom<KeybindChange> for MouseInput {
    type Error = rt::RuntimeError;

    fn try_from(key: KeybindChange) -> Result<Self, Self::Error> {
        let mods = KeyState::from(&key);
        let button = match key.key {
            extras::Key::Mouse(code) => KeyState::try_from(code),
            _ => Err("not a mouse binding"),
        }?;
        Ok(Self::with_button(button | mods))
    }
}

#[cfg(feature = "extension-arcdps")]
impl TryFrom<MouseCode> for MouseInput {
    type Error = rt::RuntimeError;

    fn try_from(code: MouseCode) -> Result<Self, Self::Error> {
        KeyState::try_from(code).map(Self::with_button)
    }
}


impl From<MouseInput> for KeyboardAndMouse::INPUT {
    fn from(input: MouseInput) -> Self {
        input.to_input()
    }
}

impl From<MousePosition> for KeyboardAndMouse::INPUT {
    fn from(position: MousePosition) -> Self {
        MouseInput::from(position).to_input()
    }
}

pub fn screen_position() -> RuntimeResult<MousePosition> {
    let mut out = MousePosition::default();
    let res = unsafe {
        WindowsAndMessaging::GetCursorPos(out.as_point_mut())
    };
    match res {
        Err(e) => {
            log::warn!("GetCursorPos failed: {e}");
            Err("GetCursorPos failed")
        },
        Ok(()) => Ok(out),
    }
}

pub fn send_mouse(input: MouseInput, prior: Option<MouseInput>) -> RuntimeResult<()> {
    let mut sent = false;
    let mut error = None;
    for (msg, w, l) in input.to_events(prior) {
        sent = true;
        let res = unsafe {
            rt::window_message(msg, w, l)
        };
        if let Err(e) = res {
            let _ = error.insert(e);
        }
    }
    error.map(Err).unwrap_or(match sent {
        true => Ok(()),
        false => Err("empty or unsupported mouse input"),
    })
}

pub fn send_input<I: Into<MouseInput>>(input: I) -> RuntimeResult<()> {
    rt::window_send_inputs(iter::once_with(move || input.into().to_input()))
}
