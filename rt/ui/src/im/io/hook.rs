use {crate::im::prelude::*, core::ffi::c_void};

/// TODO: non-dyn variants
pub trait ContextHookCallback {
    fn call_hook_dyn(
        &mut self,
        ctx: &mut dyn ImUiContext,
        id: usize,
        type_untyped: u32,
        info: &mut dyn ImContextHookInfo,
    );
}

/// TODO: unwind?
pub type ContextHookRaw = unsafe extern "C" fn(*mut c_void, *mut c_void);

pub trait ImContextHookInfo {
    fn id(&self) -> usize;
    fn owner(&self) -> usize;
    fn hook_type(&self) -> usize;
    fn raw_callback(&self) -> Option<ContextHookRaw>;
    #[cfg(todo)]
    fn set_xxx(&mut self);
    fn cancel(&mut self);
}
