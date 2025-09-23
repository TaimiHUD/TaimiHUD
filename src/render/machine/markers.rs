use {
    crate::render::machine::RenderMachine,
    taimi_meta::ui::UiMap,
    tokio::sync::Mutex,
};

impl RenderMachine {
    pub fn shared_map_state() -> &'static Mutex<UiMap> {
        static MAP_STATE: Mutex<UiMap> = Mutex::const_new(UiMap::DEFAULT);

        &MAP_STATE
    }

    pub(crate) fn publish_map_state(&self) {
        let Ok(mut shared) = Self::shared_map_state().try_lock() else {
            return
        };

        *shared = self.map.clone();
        drop(shared);
        // TODO: maybe send events if any changes since last write are actionable?
    }
}
