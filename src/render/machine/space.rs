use {
    crate::{
        render::machine::{RenderMachine, RenderPosition, RenderPositioning},
        settings::pathing::CameraSource,
        space::DrawSpace,
    },
    core::ops::Range,
    glamour::{Angle, Matrix4, Point3, Transform3, Vector2, Vector3},
    taimi_meta::{
        coords::{camera_view, MapLocalScale, ScreenSpace},
        ui::MapOpen,
    },
};
#[cfg(feature = "goggles")]
use crate::space::goggles;
#[cfg(feature = "goggles2-camera")]
use goggles::{camera::CameraSearch, CameraFerret, PerspectiveFerret, FerretResource};

impl RenderMachine {
    const CAMERA_SMOOTHING_PER_FRAME: f32 = 0.135;
    pub fn get_camera_pos(&self, source: CameraSource) -> Option<RenderPositioning<DrawSpace>> {
        match source {
            #[cfg(feature = "extension-nexus")]
            CameraSource::RealTimeAPI if self.rtapi_state.has_camera() =>
                return Some(self.rtapi_state.camera),
            #[cfg(feature = "extension-nexus")]
            CameraSource::MumbleLink if self.mumblelink_frame_skip > 0 && self.rtapi_state.has_camera() => return Some({
                let factor = Self::CAMERA_SMOOTHING_PER_FRAME * self.mumblelink_frame_skip as f32 /*.min(1.0)*/;
                let (ml_pos, ml_front) = &self.mumblelink_camera;
                let (pos, front) = self.rtapi_state.camera;
                (
                    ml_pos.lerp(pos, factor),
                    ml_front.slerp(front, factor),
                    //.rotate_towards(front, turn * Self::CAMERA_SMOOTHING_PER_FRAME)?
                )
            }),
            #[cfg(feature = "goggles2-camera")]
            CameraSource::Goggles2 => {
                let cam = FerretResource::snatch_camera();
                if !cam.is_empty() {
                    return Some(cam.get_as_look())
                }
            },
            _ => (),
        }
        match self.get_camera_mumblelink() {
            _ if self.is_cutscene() => (),
            cam @ Some(..) => return cam,
            None => (),
        }
        #[cfg(feature = "goggles2-camera")]
        if self.goggles.has_camera_fallback() {
            // TODO: cam_mumblelink should be marked stale in this case probably?
            let cam = FerretResource::snatch_camera_smooth();
            if !cam.is_empty() {
                return Some(cam.get_as_look())
            }
        }

        None
    }

    pub fn get_camera(&mut self, source: CameraSource) -> RenderPosition<DrawSpace> {
        // TODO: cache direct ptr per frame
        let (pos, front) = self
            .get_camera_pos(source)
            .unwrap_or((Point3::ZERO, Vector3::ZERO));
        (pos, front.normalize_or(Self::LOCAL_FORWARD), Self::LOCAL_UP)
    }

    pub const DEFAULT_DEPTH_RANGE: Range<f32> = {
        let near = 0.5 / MapLocalScale::METRES_PER_FEET;
        let far = 700.0 / MapLocalScale::METRES_PER_FEET;
        near..far
    };
    #[cfg(feature = "goggles")]
    pub const GOGGLES_DEPTH_RANGE: Range<f32> = {
        pub const M_TO_UNIT: f32 = 2.0 / MapLocalScale::METRES_PER_FEET;
        let near = M_TO_UNIT / 10.0;
        let far = 10_000.0 / M_TO_UNIT;
        near..far
    };

    pub fn get_depth_range(&self) -> Option<Range<f32>> {
        self.depth_range.clone()
    }

    pub fn depth_range(&self) -> Range<f32> {
        self.get_depth_range().unwrap_or_else(|| match () {
            #[cfg(feature = "goggles")]
            _ if goggles::is_enabled() => Self::GOGGLES_DEPTH_RANGE,
            _ => Self::DEFAULT_DEPTH_RANGE,
        })
    }

    /// 50 degrees vertical field of view
    ///
    /// beware that mumblelink values are rounded and inaccurate...
    pub const DEFAULT_FOV_Y: Angle = Angle::from_radians(match () {
        #[cfg(todo)]
        _ => 0.8730f32,
        _ => 50.0f32.to_radians(),
    });
    pub fn fov_y(&self) -> Option<Angle> {
        match self.fov.y.to_bits() {
            0 => None,
            _ => Some(Angle::new(self.fov.y)),
        }
    }

    pub fn get_fov(&self) -> Vector2<Angle> {
        let mut fov = self.fov;
        if fov.y.to_bits() == 0 {
            fov.y = Self::DEFAULT_FOV_Y.radians;
        }
        fov
    }

    pub(crate) const DEFAULT_FOV2_TAN: Angle = Angle::from_radians({
        #[cfg(todo)]
        let fov2 = Self::DEFAULT_FOV_Y.to_radians() * 0.5;
        // fov2.tan() // not const...
        0.466307
    });
    pub fn set_fov(&mut self, fov: Vector2<Angle>) {
        self.fov = fov;
        if self.fov.x.to_bits() == 0 {
            let fov_y = self.fov_y().unwrap_or(Self::DEFAULT_FOV_Y);
            self.fov2_tan = Angle::new((fov_y * 0.5f32).tan());
            let r = self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO);
            self.fov.x = 2.0 * (self.fov2_tan.radians * r).atan();
        }
    }

    pub fn get_space_perspective(&self) -> Transform3<DrawSpace, ScreenSpace> {
        #[cfg(feature = "goggles2-camera")]
        if let Some((h, aspect, near, far)) = self.goggles.perspective_params() {
            use glamour::Vector4;
            let w = h / aspect;
            let r = far / (far - near);
            let near_m = near * MapLocalScale::METRES_PER_INCH;
            let persp = Matrix4::from_cols(
                Vector4::ZERO.with_x(w),
                Vector4::ZERO.with_y(h),
                Vector4::W.with_z(r),
                Vector4::ZERO.with_z(-r * near_m),
            );
            return Transform3::from_matrix_unchecked(persp)
        }
        let r = self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO);
        let Range { start: near, end: far } = self.get_depth_range().unwrap_or(Self::DEFAULT_DEPTH_RANGE);
        let fov = self.get_fov();
        let scaling = Self::DEFAULT_FOV_Y.to_radians() / fov.y;
        Transform3::from_matrix_unchecked(match fov {
            fov => Matrix4::perspective_lh(Angle::new(fov.y), r, near * scaling, far * scaling),
            #[cfg(todo)]
            fov => {
                use glamour::Vector4;
                let h = 1.0 / self.fov2_tan;
                let w = h / r;
                let d = far / (far - near);
                Matrix4::from_cols(
                    Vector4::ZERO.with_x(w),
                    Vector4::ZERO.with_y(h),
                    Vector4::ZERO.with_z(d).with_w(1.0),
                    Vector4::ZERO.with_z(-d * near),
                )
            },
        })
    }

    pub fn space_view(camera: RenderPosition) -> Transform3<DrawSpace, DrawSpace> {
        let (camera_pos, camera_front, camera_up) = camera;
        Transform3::from_matrix_unchecked(
            camera_view(camera_pos, camera_front, camera_up), //glam::Mat4::look_to_lh(camera_pos.into(), camera_front.into(), camera_up.into()).into()
        )
    }

    #[cfg(todo = "unnecessary")]
    pub fn get_space_view(&self, source: CameraSource) -> Transform3<DrawSpace, DrawSpace> {
        Self::space_view(self.get_camera(source))
    }

    pub fn is_space_visible(&self) -> bool {
        match self.is_ingame() {
            Some(..) => !self.get_map_open_state().is_visible(),
            None => false,
        }
    }

    /// If we have suddenly
    /// lost or gained a depth buffer, it typically means something!
    #[cfg(feature = "goggles")]
    pub fn turn_depth_event(&mut self, acquired_or_lost: bool) {
        if !self.map_users.is_empty() {
            let changed = match (acquired_or_lost, self.get_map_open_state()) {
                (true, MapOpen::Open) => self.set_map_open(MapOpen::Closing { elapsed: 0.0 }),
                (false, MapOpen::Opening { .. }) => self.set_map_open(MapOpen::Open),
                _ => false,
            };
            if changed {
                self.act_map_open();
            }
        }
    }
}

#[cfg(feature = "goggles")]
#[derive(Debug, Clone, Default)]
pub struct GogglesState {
    pub enabled: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_enabled: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_paused: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_lost: u16,
    #[cfg(feature = "goggles2-camera")]
    pub perspective_lost: u16,
    #[cfg(feature = "goggles2-camera")]
    pub perspective_params: (f32, f32, f32, f32),
}
#[cfg(feature = "goggles")]
impl GogglesState {
    pub(crate) fn act_map_enter(&mut self) {
        #[cfg(feature = "goggles2-camera")]
        {
            self.camera_paused = false;
            if self.camera_enabled {
                self.camera_clear();
            }
        }
    }
    /// TODO: awkwardly called by engine, hacky...
    pub(super) fn act_render_post(&mut self) {
        #[cfg(todo)]
        if !self.enabled { return }

        #[cfg(feature = "goggles2-camera")]
        if self.camera_enabled && CameraSearch::with_mut_unchecked(|s| s.seen_frame()) {
            if let Some((_b, _o, _is_m43)) = FerretResource::found_camera() {
                let lost_cam = FerretResource::wants_snatch_camera();
                if lost_cam {
                    if self.camera_lost == 0 {
                        log::warn!("lost cambuf {:p}@{_o:#x}", _b as *mut ());
                    }
                    self.camera_lost = self.camera_lost.max(1);
                    FerretResource::clear_camera_found();
                } else {
                    self.camera_lost = Default::default();
                }
            }

            if let Some((_b, _o)) = FerretResource::found_perspective() {
                let lost_persp = FerretResource::wants_snatch_perspective();
                if lost_persp {
                    if self.perspective_lost == 0 {
                        log::warn!("lost perspbuf {:p}@{_o:#x}", _b as *mut ());
                    }
                    self.perspective_lost = self.perspective_lost.max(1);
                    FerretResource::clear_perspective_found();
                } else {
                    self.perspective_lost = Default::default();
                }
            }
        }
        #[cfg(feature = "goggles2-camera")]
        if self.camera_enabled {
            CameraSearch::with_mut_unchecked(|s| s.clear_active());
            if !self.camera_paused {
                FerretResource::trip_snatch_camera();
                FerretResource::trip_snatch_camera_smooth();
                FerretResource::trip_snatch_perspective();
            }
        }
    }

    pub(crate) fn act_pre_render(&mut self, _visible: bool) {
        #[cfg(feature = "goggles")]
        if /*self.enabled*/ true {
            goggles::lens::reset_frame();
        }
        #[cfg(feature = "goggles2-camera")]
        if self.camera_enabled && !self.camera_paused && !FerretResource::wants_snatch_perspective() {
            let persp = FerretResource::snatch_perspective();
            self.perspective_params = Self::perspective_params_for(&persp);
        }
        #[cfg(feature = "goggles2-camera")]
        if self.camera_enabled && FerretResource::wants_camera() {
            let mut alternatives = 0;
            let found = CameraSearch::with_mut_unchecked(|s| {
                alternatives = s.matches.len();
                s.distill()
            });
            if let Some((buf_dest, found)) = found {
                log::info!("found new cam at {buf_dest:p}@{:#x} out of {alternatives} choices", found.offset);
                goggles::FerretResource::set_camera_found(buf_dest, found.offset as _, found.is_m43);
            }
        }
    }
    pub(crate) fn reset_search(&mut self, _force: bool) {
        #[cfg(feature = "goggles2-camera")]
        {
            self.perspective_lost = self.perspective_lost.min(1);
            self.camera_lost = self.camera_lost.min(1);
        }
    }
}
#[cfg(feature = "goggles2-camera")]
impl GogglesState {
    pub(crate) fn camera_enable(&mut self) {
        self.camera_enabled = true;
        self.camera_clear();
        #[cfg(todo)]
        let (min, max) = (0x160, 0x1b0+1);
        #[cfg(todo)]
        let (min, max) = (60, 0x390);
        let (min, max) = (60, 0x5c0);
        FerretResource::set_size_range(min..max + 1);
        FerretResource::set_granularity(4);
        //FerretResource::set_granularity(8);
    }
    pub(crate) fn camera_disable(&mut self) {
        self.camera_enabled = false;
        self.camera_clear();
        FerretResource::set_size_range(8..8);
    }
    fn camera_lost_defer(lost: &mut u16, update: u8) -> bool {
        let mut deferred = update != 0;
        let retry = match *lost {
            0 => return false,
            1 => {
                deferred = false;
                true
            },
            2..=3 => true,
            lost @ 4..=0x100 => lost & 0x0f == 0,
            lost => lost & 0xff == 0,
        };
        if !deferred {
            let next = lost.wrapping_add(1);
            *lost = match next {
                0 => 0x100,
                lost => lost,
            };
        }
        !retry
    }
    pub(super) fn camera_setup(&mut self, cam: Option<RenderPositioning<DrawSpace>>, (fov_y, aspect): (f32, f32), update: u8) {
        self.camera_paused = false;
        let cam = match cam {
            Some(..) if Self::camera_lost_defer(&mut self.camera_lost, update) => None,
            cam => cam,
        };
        if let Some((pos, dir)) = cam {
            let up = RenderMachine::LOCAL_UP;
            let cam = CameraFerret::new(pos, dir, up);
            FerretResource::set_camera(cam);
        } else {
            FerretResource::set_camera(CameraFerret::EMPTY);
        }
        let update_persp = || match cam.is_some() {
            //#[cfg(todo)]
            true => {
                // desynchronize updates to reduce per-frame impact of the search
                update.wrapping_sub(1)
            },
            _ => update,
        };
        if !Self::camera_lost_defer(&mut self.perspective_lost, update_persp()) {
            let persp = PerspectiveFerret::new(fov_y, aspect);
            FerretResource::set_perspective(persp);
        } else {
            FerretResource::set_perspective(PerspectiveFerret::EMPTY);
        }
    }
    pub(super) fn camera_pause(&mut self, intermission: bool) {
        self.camera_paused = !intermission;
        if intermission {
            let needs_persp = match () {
                #[cfg(todo)]
                _ => !FerretResource::wants_perspective(),
                _ => true,
            };
            if needs_persp {
                let persp = FerretResource::snatch_perspective();
                if !persp.is_empty() {
                    FerretResource::set_perspective(persp.get_ferret_perspective());
                }
            }
            let cam = FerretResource::snatch_camera();
            if !cam.is_empty() {
                FerretResource::set_camera(cam.get_ferret_look());
            }
            // TODO: else if !fallback_cam.is_empty()?
        } else {
            FerretResource::set_perspective(PerspectiveFerret::EMPTY);
            FerretResource::set_camera(CameraFerret::EMPTY);
        }
    }
    pub(super) fn camera_clear(&mut self) {
        FerretResource::clear_camera_found();
        FerretResource::clear_perspective_found();
        FerretResource::set_camera(CameraFerret::EMPTY);
        FerretResource::set_perspective(PerspectiveFerret::EMPTY);
        CameraSearch::with_mut_unchecked(|s| s.clear());
        self.perspective_params = Default::default();
        self.perspective_lost = Default::default();
        self.camera_lost = Default::default();
    }
    pub(super) fn perspective_params(&self) -> Option<(f32, f32, f32, f32)> {
        if self.perspective_params.0.to_bits() != 0.0f32.to_bits() {
            Some(self.perspective_params)
        } else {
            None
        }
    }
    fn perspective_params_for(m: &goggles::camera::SnatchMatrix) -> (f32, f32, f32, f32) {
        let (h, range) = m.get_as_perspective();
        let aspect = m.perspective_aspect_ratio();
        (h, aspect, range.start, range.end)
    }

    /// TODO
    pub(super) fn is_camera_moving(&self) -> bool {
        false
    }
    /// TODO
    pub(super) fn has_camera(&self) -> bool {
        if !self.camera_enabled { return false }
        match () {
            _ if FerretResource::has_found_camera() && !FerretResource::wants_snatch_camera() =>
                true,
            _ if self.has_camera_fallback() => true,
            _ => false,
        }
    }
    pub(super) fn has_camera_fallback(&self) -> bool {
        match () {
            _ if !FerretResource::has_found_perspective() => false,
            _ => !FerretResource::wants_snatch_camera_smooth(),
        }
    }
}
