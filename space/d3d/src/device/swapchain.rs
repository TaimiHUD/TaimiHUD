use {
    crate::{
        prelude::*,
        device::D3dDevice,
        dx::IDXGIOutput,
    },
    windows::{
        core::Interface,
        Win32::Foundation::HWND,
    },
};

pub use crate::{
    dx::{
        IDXGISwapChain as IDXGISwapChain0,
        IDXGISwapChain1, IDXGISwapChain2, IDXGISwapChain3, IDXGISwapChain4,
        DXGI_FRAME_STATISTICS,
        DXGI_PRESENT, DXGI_PRESENT_PARAMETERS,
        DXGI_SWAP_CHAIN_DESC as DXGI_SWAP_CHAIN_DESC0,
        DXGI_SWAP_CHAIN_DESC1,
        DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
    },
    dxgi::DXGI_MODE_ROTATION,
};

// TODO: DXGI_PRESENT bitflags

impl_d3d! {
    unsafe impl D3dInterfacePtr for IDXGISwapChain;

    @[transparent(D3dInterfacePtr <= IDXGISwapChain)]
    pub struct SwapChain0.chain;
}

impl SwapChain0 {
    pub fn get_device<D: D3dDevice>(&self) -> anyhow::Result<D> {
        unsafe {
            self.chain.GetDevice()
        }.context("IDXGISwapChain::GetDevice")
    }

    pub fn get_buffer<B: Interface>(&self, index: u32) -> anyhow::Result<B> {
        unsafe {
            self.chain.GetBuffer(index)
        }.context("IDXGISwapChain::GetBuffer")
    }

    pub fn get_framebuffer<D: D3dDevice>(&self, index: u32) -> anyhow::Result<D::IBuffer> {
        self.get_buffer(index)
    }

    pub fn get_desc0(&self) -> anyhow::Result<DXGI_SWAP_CHAIN_DESC0> {
        unsafe {
            self.chain.GetDesc()
        }.context("IDXGISwapChain::GetDesc")
    }

    pub fn get_fullscreen_state(&self) -> anyhow::Result<Option<IDXGIOutput>> {
        let mut fullscreen = Default::default();
        let mut output = Default::default();
        let res = unsafe {
            self.chain.GetFullscreenState(Some(&mut fullscreen), Some(&mut output))
        }.map(move |()| output);
        match (res, fullscreen.0) {
            (Ok(None), 0) => Ok(None),
            (Ok(None), _) => Err(anyhow!("inconsistent fullscreen state due to missing output")),
            (Ok(Some(..)), 0) => Err(anyhow!("inconsistent fullscreen state due to unexpected output")),
            (Ok(Some(output)), _) => Ok(Some(output)),
            (Err(e), ..) => Err(e.into()),
        }.context("IDXGISwapChain::GetFullscreenState")
    }

    pub fn get_last_present_count(&self) -> anyhow::Result<u32> {
        unsafe {
            self.chain.GetLastPresentCount()
        }.context("IDXGISwapChain::GetLastPresentCount")
    }

    pub fn get_frame_statistics(&self) -> anyhow::Result<DXGI_FRAME_STATISTICS> {
        let mut out = Default::default();
        unsafe {
            self.chain.GetFrameStatistics(&mut out)
        }.context("IDXGISwapChain::GetFrameStatistics")
        .map(move |()| out)
    }

    pub fn get_containing_output(&self) -> anyhow::Result<IDXGIOutput> {
        unsafe {
            self.chain.GetContainingOutput()
        }.context("IDXGISwapChain::GetContainingOutput")
    }

    pub fn present(&self, sync_interval: u32, flags: DXGI_PRESENT) -> anyhow::Result<()> {
        unsafe {
            self.chain.Present(sync_interval, flags)
        }.ok().context("IDXGISwapChain::Present")
    }

    // TODO: resizebuffers, resizetarget
}

#[cfg(feature = "dx11")]
impl SwapChain0 {
    pub fn get_device11(&self) -> anyhow::Result<crate::dx11::device::Device0> {
        self.get_device()
            .map(crate::dx11::device::Device0::from_d3d)
    }

    pub fn get_framebuffer11(&self, index: u32) -> anyhow::Result<crate::dx11::Texture2> {
        self.get_buffer(index)
            .map(crate::dx11::Texture2::from_d3d)
    }
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= IDXGISwapChain1)]
    pub struct SwapChain1 {
        pub chain0: SwapChain0,
    }
    @deref(SwapChain0);
}

impl SwapChain1 {
    pub fn get_desc1(&self) -> anyhow::Result<DXGI_SWAP_CHAIN_DESC1> {
        unsafe {
            self.as_d3d().GetDesc1()
        }.context("IDXGISwapChain1::GetDesc1")
    }

    pub fn get_fullscreen_desc(&self) -> anyhow::Result<DXGI_SWAP_CHAIN_FULLSCREEN_DESC> {
        unsafe {
            self.as_d3d().GetFullscreenDesc()
        }.context("IDXGISwapChain1::GetFullscreenDesc")
    }

    pub fn get_hwnd(&self) -> anyhow::Result<HWND> {
        unsafe {
            self.as_d3d().GetHwnd()
        }.context("IDXGISwapChain1::GetHwnd")
    }

    pub fn get_rotation(&self) -> anyhow::Result<DXGI_MODE_ROTATION> {
        unsafe {
            self.as_d3d().GetRotation()
        }.context("IDXGISwapChain1::GetRotation")
    }
    pub fn set_rotation(&self, mode: DXGI_MODE_ROTATION) -> anyhow::Result<()> {
        unsafe {
            self.as_d3d().SetRotation(mode)
        }.context("IDXGISwapChain1::SetRotation")
    }

    pub fn present1(&self, sync_interval: u32, flags: DXGI_PRESENT, parameters: &DXGI_PRESENT_PARAMETERS) -> anyhow::Result<()> {
        unsafe {
            self.as_d3d().Present1(sync_interval, flags, parameters)
        }.ok().context("IDXGISwapChain1::Present1")
    }
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= IDXGISwapChain2)]
    pub struct SwapChain2 {
        pub chain1: SwapChain1,
    }
    @deref(SwapChain1);
}

impl SwapChain2 {
    pub fn get_maximum_frame_latency(&self) -> anyhow::Result<u32> {
        unsafe {
            self.as_d3d().GetMaximumFrameLatency()
        }.context("IDXGISwapChain2::GetMaximumFrameLatency")
    }

    /// The resulting rect is always aligned at (0,0)
    pub fn get_source_size(&self) -> anyhow::Result<Size2<u32>> {
        let mut out = Size2::ZERO;
        unsafe {
            self.as_d3d().GetSourceSize(&mut out.width, &mut out.height)
        }.context("IDXGISwapChain2::GetSourceSize")
        .map(move |()| out)
    }
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= IDXGISwapChain3)]
    pub struct SwapChain3 {
        pub chain2: SwapChain2,
    }
    @deref(SwapChain2);
}

impl SwapChain3 {
    pub fn get_current_back_buffer_index(&self) -> u32 {
        unsafe {
            self.as_d3d().GetCurrentBackBufferIndex()
        }
    }
}

impl_d3d! {
    @[transparent(D3dInterfacePtr <= IDXGISwapChain4)]
    pub struct SwapChain4 {
        pub chain3: SwapChain3,
    }
    @deref(SwapChain3);
}
