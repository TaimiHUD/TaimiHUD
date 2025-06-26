use {
    crate::exports::runtime::{self as rt, keyboard::KeyMods, RuntimeResult},
    std::{iter, mem::transmute, ops, slice},
    windows::Win32::{
        Foundation::{LPARAM, POINT, ERROR_SUCCESS, SetLastError, GetLastError},
        Graphics::Gdi,
        System::SystemServices,
        UI::{
            WindowsAndMessaging,
            Input::KeyboardAndMouse,
        },
    },
};
#[cfg(feature = "markers")]
use crate::marker::atomic::ScreenPoint;

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(C)]
pub struct MousePosition {
    pub x: i32,
    pub y: i32,
}

impl MousePosition {
    pub const fn new(x: i32, y: i32) -> Self {
        Self {
            x,
            y
        }
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
    pub button: Option<u32>,
    pub down: bool,
}

impl MouseInput {
    pub const fn with_position(position: MousePosition) -> Self {
        Self {
            position,
            button: None,
            down: false,
        }
    }

    pub const fn new(position: MousePosition, button: u32, down: bool) -> Self {
        Self {
            position,
            button: Some(button),
            down,
        }
    }

    pub fn to_input(self) -> KeyboardAndMouse::INPUT {
        let flag_move = KeyboardAndMouse::MOUSEEVENTF_MOVE | KeyboardAndMouse::MOUSEEVENTF_MOVE_NOCOALESCE;
        let flag_button = self.button.map(|b| match (self.down, b) {
            (true, 0) => KeyboardAndMouse::MOUSEEVENTF_LEFTDOWN,
            (false, 0) => KeyboardAndMouse::MOUSEEVENTF_LEFTUP,
            (true, 1) => KeyboardAndMouse::MOUSEEVENTF_RIGHTDOWN,
            (false, 1) => KeyboardAndMouse::MOUSEEVENTF_RIGHTUP,
            (true, 2) => KeyboardAndMouse::MOUSEEVENTF_MIDDLEDOWN,
            (false, 2) => KeyboardAndMouse::MOUSEEVENTF_MIDDLEUP,
            (true, ..) => KeyboardAndMouse::MOUSEEVENTF_XDOWN,
            (false, ..) => KeyboardAndMouse::MOUSEEVENTF_XUP,
        }).unwrap_or_default();
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
                    mouseData: self.button.map(|b| match b {
                        0..=2 => 0,
                        x => x,
                    }).unwrap_or(0),
                    time: 0,
                    dwFlags: flag_button | flag_abs | flag_move,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    pub fn to_event(self, mods: KeyMods, only_move: bool) -> Option<(u32, usize, isize)> {
        if mods.alt {
            log::error!("mouse events with alt modifier unimplemented");
            return None
        }
        let msg = match (self.down, self.button) {
            _ if only_move => WindowsAndMessaging::WM_MOUSEMOVE,
            (_, None) => WindowsAndMessaging::WM_MOUSEMOVE,
            (true, Some(0)) => WindowsAndMessaging::WM_LBUTTONDOWN,
            (false, Some(0)) => WindowsAndMessaging::WM_LBUTTONUP,
            (true, Some(1)) => WindowsAndMessaging::WM_RBUTTONDOWN,
            (false, Some(1)) => WindowsAndMessaging::WM_RBUTTONUP,
            (true, Some(2)) => WindowsAndMessaging::WM_MBUTTONDOWN,
            (false, Some(2)) => WindowsAndMessaging::WM_MBUTTONUP,
            (true, Some(_)) => WindowsAndMessaging::WM_XBUTTONDOWN,
            (false, Some(_)) => WindowsAndMessaging::WM_XBUTTONUP,
        };

        let w_mods = mods.ctrl.then_some(SystemServices::MK_CONTROL.0 as usize).into_iter()
            .chain(mods.shift.then_some(SystemServices::MK_SHIFT.0 as usize))
            .sum::<usize>();
        let w_button = match self.button {
            Some(button @ 0..=1) => SystemServices::MK_LBUTTON.0 << button,
            Some(button @ 2..=4) => SystemServices::MK_MBUTTON.0 << (button - 2),
            Some(..) => return None,
            None => 0,
        } as usize;

        let l = LPARAM::from(self.position);

        Some((msg, w_button | w_mods, l.0))
    }
}

impl From<MousePosition> for MouseInput {
    fn from(position: MousePosition) -> Self {
        Self::with_position(position)
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

pub fn send_mouse(input: MouseInput, mods: KeyMods, only_move: bool) -> RuntimeResult<()> {
    let (msg, w, l) = input.to_event(mods, only_move)
        .ok_or("unsupported mouse input")?;
    unsafe {
        rt::window_message(msg, w, l)
    }
}

pub fn send_input<I: Into<MouseInput>>(input: I) -> RuntimeResult<()> {
    rt::window_send_inputs(iter::once_with(move || input.into().to_input()))
}
