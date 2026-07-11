pub use self::{
    batch::{ImBlitBatch, ImBlitBatchMut, ImDrawTarget, ImDrawTargetExt, ImSurfaceTarget},
    buffer::{ImBufferBlob, ImBufferBlobExt, ImBufferBlobGrow, ImBufferBlobInfo, ImBufferBlobMut},
};
use super::prelude::*;

mod batch;
mod buffer;

pub trait ImDrawTargetStack<'ui> {
    fn push_clip_rect(&mut self, bounds: Box2<ImSpace>, intersect_current: bool) -> UiTokenDyn<'ui>;
    fn push_clip_rect_fullscreen(&mut self) -> UiTokenDyn<'ui>;
    #[cfg(todo)]
    fn push_texture(&mut self) -> UiTokenDyn<'ui>;
}
