use {
    super::RenderMachine,
    crate::render::element::im::UiContextStorage,
    std::sync::{LazyLock, RwLock},
};

impl RenderMachine {
    #[inline(always)]
    pub fn ui_shared_context() -> &'static RwLock<UiContextStorage> {
        static UI_CONTEXT: LazyLock<RwLock<UiContextStorage>> = LazyLock::new(Default::default);
        &UI_CONTEXT
    }
    #[inline]
    pub fn ui_read_context() -> UiContextStorage {
        (*Self::ui_shared_context()
            .read()
            .unwrap_or_else(|e| e.into_inner()))
        .clone()
    }
}
