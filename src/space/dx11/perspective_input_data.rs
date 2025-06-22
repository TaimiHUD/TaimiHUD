use {
    arc_atomic::AtomicArc,
    glam::Vec3,
    std::sync::{Arc, OnceLock},
};

pub static PERSPECTIVEINPUTDATA: OnceLock<Arc<AtomicArc<PerspectiveInputData>>> = OnceLock::new();

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PerspectiveInputData {
    pub front: Vec3,
    pub pos: Vec3,
    pub fov: f32,
    pub playpos: Vec3,
    pub map_open: bool,
    pub is_gameplay: bool,
}

impl PerspectiveInputData {
    pub fn create() {
        let aarc = Arc::new(AtomicArc::new(Arc::new(Self::default())));
        let _ = PERSPECTIVEINPUTDATA.set(aarc);
    }

    pub fn read() -> Option<Arc<Self>> {
        Some(PERSPECTIVEINPUTDATA.get()?.load())
    }
    
    pub fn swap_is_gameplay(is_gameplay: bool) {
        if let Some(data) = PERSPECTIVEINPUTDATA.get() {
            let pdata = data.load();
            data.store(Arc::new(PerspectiveInputData {
                is_gameplay,
                ..*pdata
            }))
        }
    }

    pub fn swap_map_open(map_open: bool) {
        if let Some(data) = PERSPECTIVEINPUTDATA.get() {
            let pdata = data.load();
            data.store(Arc::new(PerspectiveInputData {
                map_open,
                ..*pdata
            }))
        }
    }

    pub fn swap_camera(front: Vec3, pos: Vec3, playpos: Vec3) {
        if let Some(data) = PERSPECTIVEINPUTDATA.get() {
            let pdata = data.load();
            data.store(Arc::new(PerspectiveInputData {
                playpos,
                front,
                pos,
                ..*pdata
            }))
        }
    }

    pub fn swap_fov(fov: f32) {
        if let Some(data) = PERSPECTIVEINPUTDATA.get() {
            let pdata = data.load();
            data.store(Arc::new(PerspectiveInputData {
                fov,
                ..*pdata
            }))
        }
    }
}
