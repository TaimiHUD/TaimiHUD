use crate::{
    dx::IDXGISwapChain,
    dx11::{
        prelude::*,
        impl_d3d_ext11,
        Texture2,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[repr(transparent)]
pub struct SwapChain11 {
    pub chain: IDXGISwapChain,
}

impl SwapChain11 {
    pub fn get_device11(&self) -> anyhow::Result<Dx11Device> {
        unsafe {
            self.chain.GetDevice()
        }.context("IDXGISwapChain::GetDevice")
    }

    pub fn get_framebuffer11(&self) -> anyhow::Result<Texture2> {
        unsafe {
            self.chain.GetBuffer(0)
        }.context("IDXGISwapChain::GetBuffer")
        .map(Texture2::from_d3d)
    }
}

impl_d3d_ext11! {
    unsafe impl D3dInterfacePtr<Interface=IDXGISwapChain,@transparent> for SwapChain11,
        @field(&this => &this.chain);
}
