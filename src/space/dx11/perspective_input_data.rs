use {
    arc_atomic::AtomicArc,
    crate::space::DrawSpace,
    glam::Vec3,
    glamour::{Point3, Vector3},
    nexus::data_link::mumble::UiState,
    std::sync::{Arc, LazyLock},
};

pub static PERSPECTIVEINPUTDATA: LazyLock<AtomicArc<PerspectiveInputData>> = LazyLock::new(|| AtomicArc::new(Arc::new(PerspectiveInputData::default())));

#[derive(Debug, PartialEq, Clone)]
pub struct PerspectiveInputData {
    #[cfg(feature = "extension-nexus")]
    pub has_rtapi: bool,
    pub front: Vec3,
    pub pos: Vec3,
    pub fov: f32,
    pub playpos: Vec3,
    pub ui_state: UiState,
    pub is_gameplay: Option<bool>,
}

impl PerspectiveInputData {
    #[deprecated = "no longer fallible; use PerspectiveInputData::get()"]
    pub fn read() -> Option<Arc<Self>> {
        Some(Self::get())
    }

    pub fn get() -> Arc<Self> {
        PERSPECTIVEINPUTDATA.load()
    }

    pub fn cloned() -> Self {
        (*PERSPECTIVEINPUTDATA.load()).clone()
    }

    pub fn commit(self) {
        PERSPECTIVEINPUTDATA.store(Arc::new(self))
    }

    pub fn world_visible(&self) -> bool {
        self.is_gameplay.unwrap_or(false) && !self.ui_state.contains(UiState::IS_MAP_OPEN)
    }

    pub fn player_pos(&self) -> Point3<DrawSpace> {
        Point3::from_raw(self.playpos)
    }

    pub fn camera_pos(&self) -> Point3<DrawSpace> {
        Point3::from_raw(self.pos)
    }

    pub fn camera_front(&self) -> Vector3<DrawSpace> {
        Vector3::from_raw(self.front)
    }

    #[inline]
    pub fn camera_up(&self) -> Vector3<DrawSpace> {
        Vector3::Y
    }
}

impl Default for PerspectiveInputData {
    fn default() -> Self {
        Self {
            front: Vec3::new(0.0, 1.0, 0.0),
            pos: Vec3::ZERO,
            fov: 75.0f32.to_radians(),
            playpos: Vec3::ZERO,
            ui_state: UiState::empty(),
            is_gameplay: Default::default(),
            #[cfg(feature = "extension-nexus")]
            has_rtapi: false,
        }
    }
}
