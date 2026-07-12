#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(todo)]
use taimi_ui::im::im192;
use {
    anyhow::Context,
    glamour::{Point2, Size2},
    std::{
        env,
        ffi::{CStr, CString, OsStr},
        mem,
        path::Path,
        ptr,
        sync::Arc,
        thread,
        time::{Duration, Instant},
    },
    taimi_cover::{
        dx::{
            shaders::{ShaderDescription, ShaderDirectory, ShaderPair},
            RenderBackend11,
        },
        input::{KeysDown, KeysDownBroadcast},
        ui::ImRenderer180,
    },
    taimi_d3d::{
        device::swapchain as sc,
        dx,
        dx11::{
            buffer::{ConstantBufferP, ConstantBufferV, Texture2, TextureView2},
            context::DeviceContext0,
            device::Device0,
            prelude::*,
            DepthView,
            RenderTargetView,
            RenderTargetViews,
        },
        shader::ID3DInclude,
    },
    taimi_log as log,
    taimi_space::abi::{self, ImMap2dInstanceData},
    taimi_ui::im::{
        im180::{self, prelude::*, sys as imsys},
        ImPtr,
        ImSpaces,
    },
    windows::{
        core::{Error as WinError, BOOL, PCSTR},
        Win32::{
            Foundation::{self, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM},
            Graphics::Gdi as gdi,
            System::{LibraryLoader as ll, Threading::GetCurrentThreadId},
            UI::{Input::KeyboardAndMouse as vk, WindowsAndMessaging as wm},
        },
    },
};

#[derive(Debug, Clone)]
pub struct Opts {
    pub dpi_aware: bool,
    pub debug: bool,
    pub ini_path: Option<String>,
    pub size: Size2<i32>,
    pub offset: Point2<i32>,
}
impl Opts {
    pub fn from_env() -> Self {
        Self {
            dpi_aware: opt_var_on(env::var_os("TAIMISIM_DPIAWARE")).unwrap_or(false),
            debug: opt_var_on(env::var_os("TAIMISIM_D3DDEBUG")).unwrap_or(false),
            ini_path: env::var("TAIMISIM_INIPATH").ok(),
            size: Size2::new(1280i32, 720),
            offset: Point2::new(64i32, 64),
        }
    }
}
type WndProc = unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT;
const WNDPROC: WndProc = wnd_proc as WndProc;
fn own_module() -> anyhow::Result<HMODULE> {
    let mut own_handle = HMODULE::default();
    unsafe {
        let flags =
            ll::GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT | ll::GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS;
        ll::GetModuleHandleExA(flags, PCSTR(WNDPROC as *const _), &mut own_handle)
            .context("GetModuleHandleEx")
    }
    .map(move |()| own_handle)
}
#[derive(Debug)]
pub struct Window {
    handle: HWND,
    class_atom: u16,
    instance: HINSTANCE,
}
impl Window {
    pub const MSG_ERR: BOOL = BOOL(-1i32);
    pub const TITLE: &'static CStr = c"TaimiHUD Sim";
    pub const CLASS_NAME: &'static CStr = c"taimisim_surf";
    #[inline]
    pub const fn title_ascii() -> PCSTR {
        PCSTR(Self::TITLE.as_ptr() as *const _)
    }
    #[inline]
    pub const fn class_name_ascii() -> PCSTR {
        PCSTR(Self::CLASS_NAME.as_ptr() as *const _)
    }
    pub fn with_handle(handle: HWND) -> Self {
        Self {
            handle,
            class_atom: 0u16,
            instance: HINSTANCE::default(),
        }
    }
    pub fn new_main(opts: &Opts, module: HINSTANCE) -> anyhow::Result<Self> {
        let style_ex = wm::WINDOW_EX_STYLE::default();
        let style = wm::WS_OVERLAPPEDWINDOW;
        let class = wm::WNDCLASSEXA {
            cbSize: mem::size_of::<wm::WNDCLASSEXA>() as _,
            style: wm::CS_CLASSDC,
            lpszClassName: Self::class_name_ascii(),
            hInstance: module,
            lpfnWndProc: Some(WNDPROC),
            #[cfg(todo)]
            hCursor: default_cursor,
            ..Default::default()
        };
        let class_atom = unsafe { wm::RegisterClassExA(&class) };
        let class_atom = match class_atom {
            0 => Err(WinError::from_win32()),
            a => Ok(a),
        }
        .context("RegisterClassEx")?;
        unsafe {
            wm::CreateWindowExA(
                style_ex,
                class.lpszClassName,
                Self::title_ascii(),
                style,
                opts.offset.x,
                opts.offset.y,
                opts.size.width,
                opts.size.height,
                None,
                None,
                Some(class.hInstance),
                None,
            )
        }
        .context("CreateWindow")
        .map(|handle| Self {
            class_atom,
            handle,
            instance: class.hInstance,
        })
    }
    pub fn show(&self) {
        unsafe {
            let _ = wm::ShowWindow(self.handle, wm::SW_SHOWDEFAULT);
            let _ = gdi::UpdateWindow(self.handle);
        }
    }
    pub fn signal_quit(&self, code: u32, dest: Option<u32>) -> bool {
        let (msg, w, l, towin) = match code {
            0 => (wm::WM_CLOSE, 0, 0, true),
            c => (wm::WM_QUIT, c as _, 0, false),
        };
        let (w, l) = (WPARAM(w), LPARAM(l));
        unsafe {
            match (towin, dest) {
                (true, ..) => {
                    wm::SendMessageA(self.handle, msg, w, l);
                    true
                },
                (false, Some(dest)) => wm::PostThreadMessageA(dest, msg, w, l).is_ok(),
                (false, None) => wm::PostMessageA(Some(self.handle), msg, w, l).is_ok(),
            }
        }
    }
    pub fn destroy(&mut self) {
        if !self.handle.is_invalid() {
            let _ = log::error_ok(unsafe { wm::DestroyWindow(self.handle) }.context("DestroyWindow"));
            self.handle = Default::default();
        }
        if self.class_atom != 0 {
            let _ = log::error_ok(
                unsafe { wm::UnregisterClassA(Self::class_name_ascii(), Some(self.instance)) }
                    .context("UnregisterClass"),
            );
            self.class_atom = 0;
        }
        self.handle = HWND::default();
    }
    pub fn discard(&mut self) {
        self.handle = Default::default();
        self.class_atom = 0;
    }
    pub fn clone_ref(&self) -> Self {
        Self::with_handle(self.handle)
    }
    pub fn register_thread_for_messages() -> bool {
        let mut msg = Default::default();
        let _res = unsafe {
            wm::PeekMessageA(
                &mut msg,
                Some(wm::HWND_TOPMOST),
                wm::WM_USER,
                wm::WM_USER,
                wm::PM_NOREMOVE,
            )
        };
        true
    }
    pub fn get_dpi_scale(&self) -> f32 {
        log::debug!("TODO: monitor dpi");
        1.0f32
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        if self.class_atom != 0 {
            self.destroy();
        }
    }
}
unsafe impl Sync for Window {}
unsafe impl Send for Window {}
fn window_renderer(opts: &Opts, window: &Window) -> anyhow::Result<RenderBackend11> {
    let sc_desc = sc::DXGI_SWAP_CHAIN_DESC0 {
        BufferCount: 1,
        BufferDesc: dxgi::DXGI_MODE_DESC {
            Format: dxgi::DXGI_FORMAT_R8G8B8A8_UNORM,
            RefreshRate: dxgi::DXGI_RATIONAL { Numerator: 60, Denominator: 1 },
            ..Default::default()
        },
        BufferUsage: dx::DXGI_USAGE_RENDER_TARGET_OUTPUT,
        Flags: dx::DXGI_SWAP_CHAIN_FLAG_ALLOW_MODE_SWITCH.0 as _,
        OutputWindow: window.handle,
        SampleDesc: dxgi::DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Windowed: true.into(),
        SwapEffect: dx::DXGI_SWAP_EFFECT_DISCARD,
        ..Default::default()
    };
    let levels = [d3d::D3D_FEATURE_LEVEL_11_0, d3d::D3D_FEATURE_LEVEL_11_1];
    let mut dev = None;
    let mut sc = None;
    let mut context = None;
    let mut selected = d3d::D3D_FEATURE_LEVEL::default();
    let d3d_flags = match opts.debug {
        true => d3d11::D3D11_CREATE_DEVICE_DEBUG,
        _ => Default::default(),
    };
    let () = unsafe {
        d3d11::D3D11CreateDeviceAndSwapChain(
            None,
            #[cfg(todo)]
            d3d::D3D_DRIVER_TYPE_HARDWARE,
            d3d::D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            d3d_flags,
            Some(&levels),
            d3d11::D3D11_SDK_VERSION,
            Some(&sc_desc),
            Some(&mut sc),
            Some(&mut dev),
            Some(&mut selected),
            Some(&mut context),
        )
    }
    .context("D3D11CreateDeviceAndSwapChain")?;
    let sc = sc::SwapChain0::from_d3d(sc.context("d3d11 sc missing")?);
    let dev = Device0::from_d3d(dev.context("d3d11 dev missing")?);
    let context = DeviceContext0::from_d3d(context.context("d3d11 ctx missing")?);

    let size = opts.size.as_();
    let mut render = RenderBackend11::new(sc, dev, size).context("render init")?;
    render.context = Some(context);
    render.feature_level = selected;
    Ok(render)
}
pub struct Renderer {
    pub running: bool,
    pub resizing: Option<Size2<u16>>,
    pub render: ImRenderer180<'static>,
    pub window: Window,
    pub main_thread: u32,
    pub message_streak: u8,
    pub pause: f32,
    pub prev: Instant,
    pub keys: Arc<KeysDownBroadcast>,
    pub mapcompat_v: Option<ConstantBufferV>,
    pub mapcompat_p: Option<ConstantBufferP>,
    pub shader: Option<ShaderPair>,
}
impl Renderer {
    fn new(
        opts: &Opts,
        window: Window,
        main_thread: u32,
        keys: Arc<KeysDownBroadcast>,
    ) -> anyhow::Result<Self> {
        let render = window_renderer(opts, &window)?;

        let dpi_scale = window.get_dpi_scale();
        unsafe {
            let shared_font = ptr::null_mut();
            imsys::igCreateContext(shared_font);

            let io = imsys::igGetIO();
            (&mut *io).ConfigFlags |= (imsys::ImGuiConfigFlags_NavEnableKeyboard
                | imsys::ImGuiConfigFlags_NavEnableGamepad)
                as imsys::ImGuiConfigFlags;

            imsys::igStyleColorsDark(ptr::null_mut());

            let style = imsys::igGetStyle();
            #[cfg(todo)]
            if im192 {
                (&mut *style).FontScaleDpi = dpi_scale;
            }
            imsys::ImGuiStyle_ScaleAllSizes(style, dpi_scale);
        }
        let render = unsafe {
            let io = ptr::NonNull::new(imsys::igGetIO()).context("ImGuiIO null")?;
            ImRenderer180::new180_unchecked(render, io)
        };
        unsafe {
            let io = render.io_mut();
            io.BackendFlags |= (imsys::ImGuiBackendFlags_HasMouseCursors
                | imsys::ImGuiBackendFlags_HasSetMousePos) as imsys::ImGuiBackendFlags;
            io.BackendPlatformName = c"taimisim".as_ptr() as *const _;

            let ini_path = opts
                .ini_path
                .as_ref()
                .and_then(|ini| log::error_ok(CString::new(&ini[..]).context("ini path")));
            if let Some(ini_filename) = ini_path {
                io.IniFilename = ini_filename.into_raw() as *const _;
            }

            io.KeyMap[imsys::ImGuiKey_A as usize] = vk::VK_A.0 as _;
            io.KeyMap[imsys::ImGuiKey_C as usize] = vk::VK_C.0 as _;
            io.KeyMap[imsys::ImGuiKey_V as usize] = vk::VK_V.0 as _;
            io.KeyMap[imsys::ImGuiKey_X as usize] = vk::VK_X.0 as _;
            io.KeyMap[imsys::ImGuiKey_Y as usize] = vk::VK_Y.0 as _;
            io.KeyMap[imsys::ImGuiKey_Z as usize] = vk::VK_Z.0 as _;
            io.KeyMap[imsys::ImGuiKey_End as usize] = vk::VK_END.0 as _;
            io.KeyMap[imsys::ImGuiKey_Tab as usize] = vk::VK_TAB.0 as _;
            io.KeyMap[imsys::ImGuiKey_Home as usize] = vk::VK_HOME.0 as _;
            io.KeyMap[imsys::ImGuiKey_Enter as usize] = vk::VK_RETURN.0 as _;
            io.KeyMap[imsys::ImGuiKey_Space as usize] = vk::VK_SPACE.0 as _;
            io.KeyMap[imsys::ImGuiKey_Backspace as usize] = vk::VK_BACK.0 as _;
            io.KeyMap[imsys::ImGuiKey_Insert as usize] = vk::VK_INSERT.0 as _;
            io.KeyMap[imsys::ImGuiKey_PageUp as usize] = vk::VK_PRIOR.0 as _;
            io.KeyMap[imsys::ImGuiKey_PageDown as usize] = vk::VK_NEXT.0 as _;
            io.KeyMap[imsys::ImGuiKey_UpArrow as usize] = vk::VK_UP.0 as _;
            io.KeyMap[imsys::ImGuiKey_DownArrow as usize] = vk::VK_DOWN.0 as _;
            io.KeyMap[imsys::ImGuiKey_LeftArrow as usize] = vk::VK_LEFT.0 as _;
            io.KeyMap[imsys::ImGuiKey_RightArrow as usize] = vk::VK_RIGHT.0 as _;
            io.KeyMap[imsys::ImGuiKey_Delete as usize] = vk::VK_DELETE.0 as _;
            io.KeyMap[imsys::ImGuiKey_Escape as usize] = vk::VK_ESCAPE.0 as _;
            // TODO? :<
            io.KeyMap[imsys::ImGuiKey_KeyPadEnter as usize] = vk::VK_RETURN.0 as _;
        }
        Ok(Self {
            render,
            window,
            keys,
            running: true,
            resizing: None,
            main_thread,
            message_streak: 0u8,
            pause: Self::PAUSE_STARTUP,
            prev: Instant::now(),
            mapcompat_v: None,
            mapcompat_p: None,
            shader: None,
        })
    }

    fn draw_last_frame(&mut self) {
        let draw_data = ptr::NonNull::new(unsafe { imsys::igGetDrawData() })
            .map(ImPtr::with_nn)
            .map(|p| unsafe { &*p.as_ptr() })
            .context("missing draw data");
        let draw_data = draw_data.and_then(|dd| self.render.setup_draw(dd).map(|()| dd));
        let Some(draw_data) = log::warn_ok(draw_data) else {
            #[cfg(todo)]
            sleep();
            return
        };

        if !draw_data.is_empty() {
            self.bind_frame();
        }
        self.render.draw(draw_data);
    }

    fn pause_at_least(&mut self, amt: f32) {
        self.pause = self.pause.max(amt);
    }
    fn unpause(&mut self) -> bool {
        if self.pause.to_bits() == 0.0f32.to_bits() {
            return true
        }
        let delay = match &mut self.pause {
            #[cfg(todo = "unnecessary")]
            pause if pause.to_bits() == 0.0f32.to_bits() => None,
            pause if *pause > Self::PAUSE_MAX_STEP => {
                *pause = (*pause - Self::PAUSE_MAX_STEP).abs();
                match () {
                    #[cfg(todo)]
                    _ if self.resizing.is_some() => Self::PAUSE_MAX_STEP_D_REACTIVE,
                    _ => Self::PAUSE_MAX_STEP_D,
                }
            },
            pause => Duration::from_secs_f32(mem::take(pause)),
        };
        thread::sleep(delay);
        false
    }
    const PAUSE_RESIZE: f32 = 0.4f32;
    const PAUSE_STARTUP: f32 = 0.8f32;
    const PAUSE_MAX_STEP: f32 = 1.0f32;
    const PAUSE_MAX_STEP_D: Duration = taimi_hoard::time::duration_from_secs_f32(Self::PAUSE_MAX_STEP);
    #[cfg(todo)]
    const PAUSE_MAX_STEP_D_REACTIVE: Duration = Duration::from_millis(50);

    fn process_msg(&mut self, msg: u32, w: WPARAM, l: LPARAM) -> anyhow::Result<bool> {
        self.message_streak = self.message_streak.saturating_add(1);
        let mut process_again = matches!(self.message_streak, 0..=Self::STREAK_THRESHOLD);
        match msg {
            wm::WM_QUIT | wm::WM_CLOSE => {
                self.running = false;
                process_again = true;
            },
            wm::WM_SIZE if w.0 == wm::SIZE_MINIMIZED as usize => {
                self.process_resize(Size2::ZERO);
            },
            wm::WM_SIZE => {
                let sz = Size2::new(l.0 as u16, (l.0 >> 16) as u16);
                self.process_resize(sz);
                process_again = true;
            },
            wm::WM_CHAR if matches!(w.0, 1..=0xffff) => unsafe {
                imsys::ImGuiIO_AddInputCharacterUTF16(self.render.io_cell().get(), w.0 as u16);
                if self.message_streak != u8::MAX {
                    process_again = true
                }
            },
            wm::WM_MOUSEWHEEL | wm::WM_MOUSEHWHEEL => unsafe {
                const WHEEL_DELTA: f32 = (wm::WHEEL_DELTA as f32).recip();
                let io = self.render.io_cell().get();
                let dest = match msg {
                    wm::WM_MOUSEHWHEEL => &raw mut (*io).MouseWheelH,
                    wm::WM_MOUSEWHEEL | _ => &raw mut (*io).MouseWheel,
                };
                let delta = (w.0 >> 16) as u16 as i16 as f32 * WHEEL_DELTA;
                (*&mut *dest) += delta;
            },
            _ => (),
        }
        Ok(!process_again)
    }
    const STREAK_THRESHOLD: u8 = 0x80;

    fn process_resize(&mut self, new_size: Size2<u16>) {
        self.resizing = Some(new_size);
        self.pause_at_least(Self::PAUSE_RESIZE);
    }

    /// TODO: GetClientRect ig
    fn update_im_display(&mut self) {
        let io = unsafe { self.render.io_mut() };
        io.DisplaySize = ImSpaces(self.render.backend.viewport.size2()).into();
    }
    fn update_im_timers(&mut self) {
        let prev = mem::replace(&mut self.prev, Instant::now());
        let elapsed = self.prev.duration_since(prev);
        let io = unsafe { self.render.io_mut() };
        io.DeltaTime = elapsed.as_secs_f32();
    }
    fn update_im_mouse(&mut self) {
        let io = unsafe { self.render.io_mut() };
        if io.WantSetMousePos {
            let mut pos = Foundation::POINT {
                x: io.MousePos.x as i32,
                y: io.MousePos.y as i32,
            };
            let res = unsafe {
                let res = gdi::ClientToScreen(self.window.handle, &mut pos);
                match res {
                    Foundation::FALSE => Err(WinError::from_win32()),
                    _ => Ok(pos),
                }
            }
            .context("ClientToScreen");
            let res = res.and_then(|pos| unsafe { wm::SetCursorPos(pos.x, pos.y).context("SetCursorPos") });
            let _ = log::warn_ok(res);
        }
        let pos = unsafe {
            let mut pos = Default::default();
            let handle = self.window.handle;
            wm::GetCursorPos(&mut pos)
                .context("GetCursorPos")
                .and_then(move |()| {
                    match gdi::ScreenToClient(handle, &mut pos) {
                        Foundation::FALSE => Err(WinError::from_win32()),
                        _ => Ok(pos),
                    }
                    .context("ScreenToClient")
                })
        };
        if let Some(pos) = log::warn_ok(pos) {
            // TODO: apply framebuffer scale here or no?
            io.MousePos = ImSpaces(Point2::<f32>::new(pos.x as f32, pos.y as f32)).into();
        }
    }
    fn update_im_inputs(&mut self) {
        let io = unsafe { self.render.io_mut() };
        let keys = self.keys.keys_racy();
        io.KeyAlt = keys.is_key_down(vk::VK_MENU.0 as _)
            | keys.is_key_down(vk::VK_LMENU.0 as _)
            | keys.is_key_down(vk::VK_RMENU.0 as _);
        io.KeyCtrl = keys.is_key_down(vk::VK_CONTROL.0 as _)
            | keys.is_key_down(vk::VK_LCONTROL.0 as _)
            | keys.is_key_down(vk::VK_RCONTROL.0 as _);
        io.KeyShift = keys.is_key_down(vk::VK_SHIFT.0 as _)
            | keys.is_key_down(vk::VK_LSHIFT.0 as _)
            | keys.is_key_down(vk::VK_RSHIFT.0 as _);
        io.KeySuper = keys.is_key_down(vk::VK_LWIN.0 as _) | keys.is_key_down(vk::VK_RWIN.0 as _);
        for (dest, s) in io.KeysDown.iter_mut().zip(keys.iter_keys()) {
            *dest = s;
        }
        let buttons = [
            keys.is_key_down(vk::VK_LBUTTON.0 as _),
            keys.is_key_down(vk::VK_RBUTTON.0 as _),
            keys.is_key_down(vk::VK_MBUTTON.0 as _),
            keys.is_key_down(vk::VK_XBUTTON1.0 as _),
            keys.is_key_down(vk::VK_XBUTTON2.0 as _),
        ];
        for (dest, s) in io.MouseDown.iter_mut().zip(buttons) {
            *dest = s;
        }
    }

    fn turn(&mut self) -> anyhow::Result<()> {
        let msg = unsafe {
            let mut msg = Default::default();
            let peek = wm::PeekMessageA(&mut msg, Some(wm::HWND_TOPMOST), 0, 0, wm::PM_REMOVE);
            match peek {
                Foundation::FALSE => None,
                _ => Some((msg.message, msg.wParam, msg.lParam)),
            }
        };
        if let Some((msg, w, l)) = msg {
            let keep_going = self.process_msg(msg, w, l)?;
            if !keep_going {
                #[cfg(todo)]
                log::trace!("skipping render for message...");
                return Ok(())
            }
        }
        // reset streak after rendering a frame
        self.message_streak = 0;

        if self.resizing.is_some()
            && self.render.backend.viewport.viewport.Width.to_bits() == 0.0f32.to_bits()
        {
            self.pause_at_least(Self::PAUSE_RESIZE);
        }

        if !self.unpause() {
            // still waiting or just finished and should check queue
            return Ok(())
        }

        if let Some(size) = self.resizing.take() {
            let mut vp = self.render.backend.viewport_rect();
            vp.size = size.as_();
            self.render.resize(vp);
            let size = size.as_::<u32>();
            log::debug!("resizing window to {size:?}");
            unsafe {
                self.render.backend.swap_chain.chain.ResizeBuffers(
                    0,
                    size.width,
                    size.height,
                    dxgi::DXGI_FORMAT_UNKNOWN,
                    dx::DXGI_SWAP_CHAIN_FLAG::default(),
                )?;
            }
        }
        self.setup_frame()?;

        self.update_im_display();
        self.update_im_timers();
        self.update_im_inputs();
        self.update_im_mouse();

        unsafe { imsys::igNewFrame() }

        // TODO: imgui draw dyn callback or something lol
        unsafe {
            let vis = imsys::igBegin(c"heya".as_ptr() as *const _, ptr::null_mut(), 0);
            if vis {
                imsys::igText(c"%s".as_ptr() as *const _, c"wheeee".as_ptr());
            }
            imsys::igEnd();
        }

        unsafe {
            imsys::igRender();
        }

        if let (Some(rt), Some(context)) = (&self.render.rt, &self.render.backend.context) {
            rt.clear_rgba(context, glam::Vec3::splat(0.6f32).extend(1.0f32));
        }
        self.draw_last_frame();
        if let (Some(..), Some(context)) = (&self.render.rt, &self.render.backend.context) {
            // unbind only really matters when resizing, but why not clean up after each frame anyway...
            RenderTargetViews::with_views(None::<&RenderTargetView>, None::<&DepthView>).set(context);
        }

        let vsync_on = 1;
        self.render
            .backend
            .swap_chain
            .present(vsync_on, Default::default())?;

        Ok(())
    }
    fn setup_im(&mut self) -> anyhow::Result<()> {
        unsafe {
            self.render.register()?;
        }
        Ok(())
    }
    /// TODO: shader or sampler needs to be aware...
    #[cfg(todo)]
    const FONT_RGBA: bool = false;
    const FONT_RGBA: bool = true;
    fn setup_font_texture(&mut self) -> anyhow::Result<()> {
        let fonts = unsafe { &mut *self.render.io_mut().Fonts };
        let prev_tex = ptr::NonNull::new(mem::take(&mut fonts.TexID))
            .map(|p| unsafe { TextureView2::from_d3d(ID3D11ShaderResourceView::from_raw(p.as_ptr())) });
        if let Some(_tex) = prev_tex {
            #[cfg(debug_assertions)]
            log::debug!("destroying stale font texture");
        }
        let mut size = Size2::<i32>::ZERO;
        let mut bypp = 1i32;
        let mut data = ptr::null_mut();
        match Self::FONT_RGBA {
            true => unsafe {
                imsys::ImFontAtlas_GetTexDataAsRGBA32(
                    fonts,
                    &mut data,
                    &mut size.width,
                    &mut size.height,
                    &mut bypp,
                )
            },
            false => unsafe {
                imsys::ImFontAtlas_GetTexDataAsAlpha8(
                    fonts,
                    &mut data,
                    &mut size.width,
                    &mut size.height,
                    &mut bypp,
                )
            },
        }
        if size.width == 0 {
            log::warn!("font texture empty?");
        }
        let mut desc2d = match Self::FONT_RGBA {
            true => ImRenderer180::FONT_DESC_RGBA8,
            false => ImRenderer180::FONT_DESC_A8,
        };
        desc2d.Width = size.width as _;
        desc2d.Height = size.height as _;
        let data = d3d11::D3D11_SUBRESOURCE_DATA {
            pSysMem: data as *const u8 as *const _,
            SysMemPitch: size.width as u32 * bypp as u32,
            ..Default::default()
        };
        let desc_srv = d3d11::D3D11_TEX2D_SRV {
            MipLevels: desc2d.MipLevels,
            ..TextureView2::DESC_DEFAULT
        };
        let srv =
            unsafe { Texture2::new_with_desc_unchecked(&self.render.backend.device, &desc2d, Some(&data)) }
                .and_then(|tex| {
                    TextureView2::new_with_texture2(&self.render.backend.device, &tex, Some(desc_srv))
                })?;
        // TODO: store this or prev somewhere to be deallocated after a frame or two in case of stale drawdata??
        fonts.TexID = srv.into_d3d().into_raw();
        Ok(())
    }
    fn setup_shaders(&mut self) -> anyhow::Result<()> {
        // TODO: render setup: font
        let shaders = &mut self.render.backend.shaders;
        shaders.register_layout11_sys("Map2dIm", &ImMap2dInstanceData::INPUT_LAYOUT_ORTHO_IB);
        let root = match () {
            #[cfg(debug_assertions)]
            _ => Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/shaders")),
            #[cfg(not(debug_assertions))]
            _ => Path::new("shaders"),
        };
        let embedded = match () {
            #[cfg(debug_assertions)]
            _ => None,
            #[cfg(not(debug_assertions))]
            _ => Some(include_dir::include_dir!(
                "$CARGO_MANIFEST_DIR/../../data/shaders"
            )),
        };
        let dir = ShaderDirectory::new(root.to_owned(), embedded);
        let descs = IntoIterator::into_iter([dir
            .get_file_contents("2d.shaderdesc")
            .and_then(|f| ShaderDescription::load_from_bytes(f).context("2d.shaderdesc"))])
        .flat_map(|d| log::error_ok(d).into_iter().flatten());
        shaders.load_with_descs(&dir, descs, &self.render.backend.device)?;
        let includes = ID3DInclude::new(&dir);
        let bytecode_v = {
            let desc = shaders.partial.get("imgui-v").context("imgui-v")?;
            let f = dir.get_file_contents(&desc.path)?;
            let mut desc = desc.clone();
            desc.defs.extend([(c"SHADER_MAP", c"3")]);
            desc.defs.terminate();
            desc.compile(&f[..], Some(&*includes))
        }?;
        shaders.load_partial(&self.render.backend.device, "imgui", &bytecode_v, "imgui-v")?;
        let bytecode_p = {
            let desc = shaders.partial.get("map2d-p").context("map2d-p")?;
            let f = dir.get_file_contents(&desc.path)?;
            let mut desc = desc.clone();
            desc.defs.extend([(c"SHADER_MAP", c"3")]);
            desc.defs.terminate();
            desc.compile(&f[..], Some(&*includes))
        }?;
        shaders.load_partial(&self.render.backend.device, "imgui", &bytecode_p, "map2d-p")?;
        self.shader = Some(shaders.pair_named("imgui")?);
        Ok(())
    }
    fn setup_d3d(&mut self) -> anyhow::Result<()> {
        self.setup_shaders()?;

        let mut cb_v = abi::Map2dConstantDataV::IDENTITY;
        // undo our silly inverted UVs...
        cb_v.trail.tex_scale = -1.0f32;
        cb_v.trail.tex_offset = -1.0f32;
        let cb_v = ConstantBufferV::new_with_data(&self.render.backend.device, &cb_v)?;
        let cb_p = ConstantBufferP::new_with_data(
            &self.render.backend.device,
            &abi::Map2dConstantDataP::IDENTITY,
        )?;
        self.mapcompat_v = Some(cb_v);
        self.mapcompat_p = Some(cb_p);

        self.render.setup_depth_state()?;
        Ok(())
    }
    fn setup_frame(&mut self) -> anyhow::Result<()> {
        self.render.setup_frame()?;
        Ok(())
    }
    fn bind_frame(&mut self) {
        let Some(context) = &self.render.backend.context else { return };
        self.render.backend.viewport.set(context);
        self.render.backend.blend_state.set(context);
        self.render.backend.sampler_state.set(context, 0);
        ImRenderer180::PRIMITIVE.set(context);
        if let Some(cb_v) = &self.mapcompat_v {
            cb_v.set(context, 0);
        }
        if let Some(cb_p) = &self.mapcompat_p {
            cb_p.set(context, 0);
        }
        if let Some(shader) = &self.shader {
            shader.set(context);
        }
        if let Some(rt) = &self.render.rt {
            RenderTargetViews::with_views(rt, None::<&DepthView>).set(context);
        }
    }
    fn pre_main(&mut self) -> anyhow::Result<()> {
        self.setup_d3d()?;
        self.setup_im()?;
        self.setup_font_texture().context("font setup")?;

        Window::register_thread_for_messages();
        let w = unsafe { GetCurrentThreadId() };
        unsafe { wm::PostThreadMessageA(self.main_thread, wm::WM_USER, WPARAM(w as _), LPARAM::default()) }
            .context("PostThreadMessage")?;

        Ok(())
    }
    fn main(&mut self) -> anyhow::Result<()> {
        while self.running {
            let () = self.turn()?;
        }
        Ok(())
    }
    fn spawn(
        opts: Opts,
        window: Window,
        main_thread: u32,
        keys: Arc<KeysDownBroadcast>,
    ) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            let render = Renderer::new(&opts, window.clone_ref(), main_thread, keys);
            let render = match render {
                Ok(mut render) => render.pre_main().map(move |()| render),
                res @ Err(..) => res,
            };
            let Some(mut render) = log::error_ok(render) else {
                window.signal_quit(EXIT_RENDER_ERR, Some(main_thread));
                return
            };

            let res = log::error_ok(render.main());
            window.signal_quit(
                match res.is_some() {
                    true => 0,
                    false => EXIT_RENDER_ERR,
                },
                Some(main_thread),
            );
        })
    }
}
const EXIT_RENDER_ERR: u32 = 1;
const EXIT_RENDER_TIMEOUT: u32 = 2;
fn try_main(opts: &Opts) -> anyhow::Result<u32> {
    let module = own_module()?;

    let mut window = Window::new_main(opts, HINSTANCE(module.0))?;

    Window::register_thread_for_messages();
    let keys = Arc::new(KeysDownBroadcast::EMPTY);
    let thread_id = unsafe { GetCurrentThreadId() };
    let render = Renderer::spawn(opts.clone(), window.clone_ref(), thread_id, keys.clone());
    #[cfg(all(todo, windows))]
    let render_queue = render.as_raw_handle();
    let mut render_id = None::<u32>;

    let _ = thread::spawn({
        let window = window.clone_ref();
        move || {
            let res = log::error_ok(
                render
                    .join()
                    .map_err(|e| anyhow::anyhow!("{e:?}"))
                    .context("renderer panicked"),
            );
            if res.is_none() {
                window.signal_quit(EXIT_RENDER_ERR, Some(thread_id));
            }
        }
    });
    let _ = thread::spawn(move || unsafe {
        thread::sleep(Duration::from_secs(5));
        // if renderer fails to start up in time, tell main thread to give up
        let _ = wm::PostThreadMessageA(thread_id, wm::WM_USER, WPARAM::default(), LPARAM::default());
    });

    loop {
        let (msg, w, l) = unsafe {
            let mut msg = Default::default();
            let receiver = match () {
                #[cfg(todo)]
                _ => Some(wm::HWND_TOPMOST),
                #[cfg(todo)]
                _ => Some(window.handle),
                _ => None,
            };
            let res = match wm::GetMessageA(&mut msg, receiver, 0, 0) {
                Foundation::FALSE => {
                    debug_assert_eq!(msg.message, wm::WM_QUIT);
                    log::debug!("WM_QUIT received by main thread");
                    if let Some(dest) = render_id {
                        let _res = wm::PostThreadMessageA(dest, msg.message, msg.wParam, msg.lParam);
                        #[cfg(debug_assertions)]
                        let _ = log::debug_ok(_res.context("PostThreadMessage").context("render relay WM_QUIT"));
                    }
                    break Ok(msg.wParam.0 as _)
                },
                Window::MSG_ERR => Err(WinError::from_win32()),
                _ => {
                    debug_assert_ne!(msg.message, wm::WM_QUIT);
                    Ok(())
                },
            }
            .context("GetMessage");
            let () = res?;
            #[cfg(debug_assertions)]
            match msg.message {
                // omit very spammy messages...
                wm::WM_SIZE | wm::WM_PAINT => (),
                _ => log::trace!("rx(T): {}({})", msg.message, msg.wParam.0),
            }
            let _ = wm::TranslateMessage(&msg);
            wm::DispatchMessageA(&msg);
            (msg.message, msg.wParam, msg.lParam)
        };
        let tell_render = matches!(
            msg,
            wm::WM_CLOSE | wm::WM_SIZE | wm::WM_CHAR | wm::WM_MOUSEWHEEL | wm::WM_MOUSEHWHEEL
        );
        match msg {
            wm::WM_USER if w.0 == 0 =>
                if render_id.is_none() {
                    log::error!("render setup timeout");
                    break Ok(EXIT_RENDER_TIMEOUT)
                },
            wm::WM_USER => {
                log::debug!("render thread notified main of start");
                window.show();
                render_id = Some(w.0 as u32);
                unsafe {
                    // TODO: remove hack here, set when imgui requests a change
                    let _ = wm::SetCursor(wm::LoadCursorW(None, wm::IDC_ARROW).ok());
                }
            },
            wm::WM_CLOSE => {
                window.destroy();
            },
            wm::WM_DESTROY => {
                window.discard();
            },
            KeysDown::WM_KEY_MIN..=KeysDown::WM_KEY_MAX => {
                let _res = keys.process_key_event_unchecked(msg, w.0, l.0);
                #[cfg(debug_assertions)]
                if let Some(e) = _res {
                    log::debug!("input(k) event: {e:?}");
                }
            },
            KeysDown::WM_BUTTON_MIN..=KeysDown::WM_BUTTON_MAX => {
                let _res = keys.process_button_event_unchecked(msg, w.0, l.0);
                #[cfg(debug_assertions)]
                if let Some(e) = _res {
                    log::debug!("input(b) event: {e:?}");
                }
            },
            _ => (),
        }
        if let (Some(dest), true) = (render_id, tell_render) {
            let _res = unsafe { wm::PostThreadMessageA(dest, msg, w, l) };
            #[cfg(debug_assertions)]
            let _ = log::debug_ok(_res.context("PostThreadMessage").context("render relay"));
        }
    }
}

unsafe extern "system" fn wnd_proc(h: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    #[cfg(debug_assertions)]
    match msg {
        // omit very spammy messages...
        wm::WM_SIZE
        | wm::WM_MOVE
        | wm::WM_MOUSEMOVE
        | wm::WM_WINDOWPOSCHANGED
        | wm::WM_WINDOWPOSCHANGING
        | wm::WM_GETMINMAXINFO
        | wm::WM_NCCALCSIZE
        | wm::WM_NCPAINT
        | wm::WM_ERASEBKGND
        | wm::WM_PAINT => (),
        _ => log::trace!("rx(W): {msg}({})", w.0),
    }
    match msg {
        wm::WM_SIZE | wm::WM_CLOSE => {
            let res = wm::PostMessageA(None, msg, w, l);
            if log::warn_ok(res).is_some() {
                return Default::default()
            }
        },
        wm::WM_SETCURSOR if l.0 as u32 & 0xffff == wm::HTCLIENT => unsafe {
            let _ = wm::SetCursor(wm::LoadCursorW(None, wm::IDC_ARROW).ok());
            return LRESULT(Foundation::TRUE.0 as _)
        },
        wm::WM_SYSCOMMAND if w.0 as u32 & 0xfff0 == wm::SC_KEYMENU =>
        // prevent alt from triggering window menu
            return Default::default(),
        wm::WM_DESTROY => {
            wm::PostQuitMessage(0);
            return Default::default()
        },
        _ => (),
    }
    let res = wm::DefWindowProcA(h, msg, w, l);
    match msg {
        #[cfg(todo)]
        wm::WM_NCHITTEST => {
            log::debug!("WM_NCHITTEST returned {res:?}");
            res
        },
        _ => res,
    }
}

fn pre_main() {
    env_logger::init_from_env(env_logger::Env::new().default_filter_or("warn"));
}

pub fn main() {
    pre_main();
    let opts = Opts::from_env();
    match try_main(&opts) {
        Ok(0) => (),
        Ok(exit) => std::process::exit(exit as _),
        Err(e) if log::log_enabled!(log::Level::Error) => {
            log::error!("{e:#}");
        },
        Err(e) => {
            eprintln!("{e:#}");
        },
    }
}
fn opt_var_on<V: AsRef<OsStr>>(v: Option<V>) -> Option<bool> {
    let v = v.as_ref().map(AsRef::as_ref)?;
    if v.is_empty() {
        return None
    }
    Some(!v.eq_ignore_ascii_case("0"))
}
