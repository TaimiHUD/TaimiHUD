use windows::core::Interface;

pub use self::swapchain::{SwapChain0, SwapChain1, SwapChain2, SwapChain3, SwapChain4};

pub mod swapchain;

pub trait D3dContext: Interface {
    type IDevice: D3dDevice;
}
pub trait D3dDevice: Interface {
    type IBuffer: Interface;
}

#[cfg(todo)]
impl<D3DC: D3dContext> D3dContext for &'_ D3DC {
    type IDevice = D3DC::IDevice;
}
#[cfg(todo)]
impl<D3DD: D3dDevice> D3dDevice for &'_ D3DD {
    type IBuffer = D3DD::IBuffer;
}

pub trait D3dContextBindable<D3DC> {
    fn set(&self, device_context: &D3DC);
}

pub trait D3dContextBindableSlot<D3DC> {
    fn set(&self, device_context: &D3DC, slot: u32);
}

impl<T: ?Sized, D3DC: D3dContext> D3dContextBindable<D3DC> for &'_ T
where
    T: D3dContextBindable<D3DC>,
{
    fn set(&self, device_context: &D3DC) {
        D3dContextBindable::set(*self, device_context)
    }
}
impl<T: ?Sized, D3DC: D3dContext> D3dContextBindableSlot<D3DC> for &'_ T
where
    T: D3dContextBindableSlot<D3DC>,
{
    fn set(&self, device_context: &D3DC, slot: u32) {
        D3dContextBindableSlot::set(*self, device_context, slot)
    }
}
