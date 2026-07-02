use {
    crate::{
        render::machine::{RenderMachine, RenderPosition, RenderPositioning, MumblelinkFrames},
        settings::pathing::CameraSource,
        space::{engine::Engine, DrawSpace},
    },
    core::{num::NonZero, ops::Range},
    glam::{Vec3A, Quat},
    glamour::{Angle, Matrix4, Point3, Transform3, Vector2, Vector3},
    std::sync::LazyLock,
    taimi_meta::{
        spatial::record::{frame_is_lt, FrameRecordOf, FrameRecordEntry},
        coords::{camera_view, MapLocalScale, ScreenSpace},
        map::MapProjectionDepth,
        ui::{gameplay::{GameplayState, GameplayTransition}, MapOpen},
    },
};
#[cfg(feature = "goggles")]
use crate::{
    exports::runtime as rt,
    settings::goggles::GogglesMapDepth,
};
#[cfg(feature = "goggles2-camera")]
use {
    crate::space::goggles::{camera::SnatchMatrix, GogglesShared},
    taimi_meta::ui::UiState,
};
#[cfg(any(feature = "goggles2-camera", feature = "goggles2-project"))]
use crate::settings::goggles::GogglesEnables;

impl RenderMachine {
    pub fn get_camera_pos(&self, source: CameraSource) -> Option<(RenderPosition<DrawSpace>, CameraSource)> {
        let new_frame = self.mumblelink_frames.latest_space_tick();
        let new_frame = match new_frame {
            #[cfg(feature = "goggles2-project")]
            new_frame if self.goggles.is_enabled(GogglesEnables::PROJECT_ENABLE) => new_frame,
            new_frame => new_frame,
        };
        let new = match () {
            #[cfg(feature = "goggles2-camera")]
            _ if self.goggles.camera.debug_interpolate => self.camera.debug_interpolate(new_frame, Some(source), &self.mumblelink_frames, self.goggles.camera.debug_interpolate_off),
            _ => self.camera.get_or_interpolate(new_frame, Some(source), &self.mumblelink_frames),
        };
        new.map(|cam| {
            let up = match () {
                #[cfg(feature = "goggles2-camera")]
                _ if !self.goggles.camera.debug_toggle_up => Self::LOCAL_UP,
                _ => self.camera.up_at(new_frame),
            };
            ((cam.pos(), cam.front(), up), source)
        })
    }

    pub(super) fn is_camera_moving(&self) -> bool {
        self.camera.is_moving(self.mumblelink_frames.latest_render_tick(), None, &self.mumblelink_frames)
    }

    pub(super) fn pre_render_space(&mut self) {
        #[cfg(feature = "goggles")]
        {
            let has_engine = Engine::is_available();
            self.goggles.act_pre_render(has_engine, self.display_size_ref().to_raw());
            if self.goggles.is_classifying() {
                let frame = rt::with_dxgi_swap_chain(|sc| sc.get_last_present_count().ok());
                if let Some(Some(frame)) = frame {
                    #[cfg(todo)]
                    {
                        self.dxgi_frame_count_prev = mem::replace(&mut self.dxgi_frame_count, frame);
                    }
                    self.goggles.class.record_present_count(frame);
                }
            }
        }
    }

    #[cfg(deleteme)]
    pub(super) const CAMERA_SMOOTHING_PER_FRAME: f32 = 0.135;
    #[cfg(deleteme)]
    pub fn get_camera_pos(&self, source: CameraSource) -> Option<(RenderPosition<DrawSpace>, CameraSource)> {
        let mut has_up = false;
        let mut camsrc = CameraSource::MumbleLink;
        let mut cam = match self.get_camera_mumblelink() {
            Some(cam) => {
                has_up = !cam.2.x.is_infinite() && !vec32_eq(cam.2, Vector3::ZERO);
                cam
            },
            None => Self::POSITION_EMPTY,
        };
        if self.is_cutscene() {
            cam = Self::POSITION_EMPTY;
            has_up = false;
        }
        #[cfg(feature = "extension-nexus")]
        if self.rtapi_state.has_camera() {
            if source == CameraSource::RealTimeAPI || cam.1.x.is_infinite() {
                let (pos, dir) = self.rtapi_state.camera;
                cam = (pos, dir, cam.2);
                camsrc = CameraSource::RealTimeAPI;
            }
        }
        #[cfg(feature = "goggles2-camera")]
        {
            let g2_has_cam = self.goggles.camera.has_camera_primary();
            let prefer_g2 = source == CameraSource::Goggles2;
            let need_cam = cam.1.x.is_infinite();
            let snatch = if (prefer_g2 | need_cam | !has_up) && g2_has_cam {
                Some(FerretResource::snatch_camera())
            } else if self.goggles.camera.has_camera_fallback() && (need_cam | !has_up) {
                Some(FerretResource::snatch_camera_smooth())
            } else {
                None
            };
            match snatch {
                Some(snatch) if (g2_has_cam && prefer_g2) | need_cam => {
                    let (pos, dir) = snatch.get_as_look();
                    cam.0 = pos;
                    cam.1 = dir;
                    has_up = false;
                    camsrc = CameraSource::Goggles2;
                },
                _ => (),
            }
            match snatch {
                Some(snatch) if !has_up => {
                    cam.2 = snatch.get_look_up();
                    // TODO? cam.3 = snatch.get_look_side();
                    has_up = true;
                },
                _ => (),
            }
        }
        let has_cam = !cam.1.x.is_infinite();
        #[cfg(todo)]
        if camsrc == CameraSource::MumbleLink && has_cam && self.mumblelink_frame_skip > 0 {
            let mut interp = None;
            #[cfg(feature = "goggles2-camera")]
            if self.goggles.camera.has_camera_primary() {
                interp = Some(FerretResource::snatch_camera().get_as_look())
            } else if self.goggles.camera.has_camera_fallback() {
                interp = Some(FerretResource::snatch_camera_smooth().get_as_look())
            }
            #[cfg(feature = "extension-nexus")]
            if interp.is_none() && self.rtapi_state.has_camera() {
                interp = Some(self.rtapi_state.camera);
            }
            if let Some((pos, front)) = interp {
                let (ml_pos, ml_front, ..) = &self.mumblelink_camera;
                let factor = (Self::CAMERA_SMOOTHING_PER_FRAME * self.mumblelink_frame_skip as f32).min(1.0);
                cam.0 = ml_pos.lerp(pos, factor);
                cam.1 = ml_front.slerp(front, factor);
                //.rotate_towards(front, turn * Self::CAMERA_SMOOTHING_PER_FRAME)?
            }
        }
        match has_up.then(|| cam.2.try_normalize()) {
            Some(Some(up)) => cam.2 = up,
            _ => has_up = false,
        }
        if !has_up {
            cam.2 = Self::LOCAL_UP;
        }
        has_cam.then_some((cam, camsrc))
    }

    pub fn get_camera(&mut self, source: CameraSource) -> RenderPosition<DrawSpace> {
        // TODO: cache direct ptr per frame
        match self.get_camera_pos(source) {
            Some((cam, _src)) => cam,
            None => {
                log::warn!("BUG: missing camera data");
                (Point3::ZERO, Self::LOCAL_FORWARD, Self::LOCAL_UP)
            },
        }
    }

    #[cfg(deleteme)]
    pub const DEPTH_FAR_MULT: f32 = 2114.0f32;
    #[cfg(deleteme)]
    pub const DEPTH_NEAR_MULT: f32 = 1.0f32 / Self::DEPTH_FAR_MULT;
    #[cfg(deleteme)]
    pub(super) const DEPTH_ASPECT_MAX: f32 = 1.5f32;
    pub const DEFAULT_DEPTH_RANGE_IN: Range<f32> = {
        #[cfg(deleteme)]{
        pub const M_TO_UNIT: f32 = 2.0 / MapLocalScale::METRES_PER_FEET;
        let near = M_TO_UNIT / 10.0;
        let far = 10_000.0 / M_TO_UNIT;
        }
        let near = MapProjectionDepth::DEFAULT_FALLBACK.z_near_in_reference();
        let far = MapProjectionDepth::DEFAULT_FALLBACK.z_far_in();
        near..far
    };
    pub const DEFAULT_DEPTH_RANGE: Range<f32> =
        Self::DEFAULT_DEPTH_RANGE_IN.start * MapProjectionDepth::DEFAULT_METRES_PER_INCH
        ..
        Self::DEFAULT_DEPTH_RANGE_IN.end * MapProjectionDepth::DEFAULT_METRES_PER_INCH;
    #[cfg(deleteme)]
    pub const DEFAULT_DEPTH_RANGE: Range<f32> = {
        let near = 0.5 / MapLocalScale::METRES_PER_FEET;
        let far = 700.0 / MapLocalScale::METRES_PER_FEET;
        near..far
    };

    /// inches :<
    pub fn get_depth_range(&self) -> Option<Range<f32>> {
        self.depth_range.clone()
            .or_else(|| match () {
                #[cfg(todo = "unnecessary")]
                _ => self.get_depth_calibration(),
                _ => self.map_depth.as_ref().or(self.map_depth_guess.as_ref()),
            }.map(|z| {
                z.z_near_in_reference()..z.z_far_in()
            }))
    }

    pub fn depth_range(&self) -> Range<f32> {
        #[cfg(feature = "goggles2-camera")]
        if let Some((_, _, near, far)) = self.goggles.camera.perspective_params() {
            let i2m = self.depth_scale_i2m();
            let near = near * i2m;
            let far = far * i2m;
            return near..far
        }
        let Range { start: near, end: far } = self.get_depth_range()
            .unwrap_or(Self::DEFAULT_DEPTH_RANGE_IN);
        let i2m = self.depth_scale_i2m();
        let near = near / self.get_fov().y;
        let near_max = match () {
            #[cfg(todo)]
            _ => MapProjectionDepth::DEFAULT_NEAR_MAX_M,
            #[cfg(todo = "unnecessary")]
            _ => MapProjectionDepth::NEAR_MAX_IN * i2m,
            _ => MapProjectionDepth::NEAR_MAX_IN,
        };
        let near = near.min(near_max) * i2m;
        let far = far * i2m;
        near..far
    }
    #[cfg(feature = "goggles")]
    pub fn set_depth_range_v2(&mut self, z: &MapProjectionDepth) {
        self.depth_range = Some(z.z_near_in_reference()..z.z_far_in())
    }
    #[cfg(feature = "goggles")]
    pub fn set_depth_range(&mut self, entry: &GogglesMapDepth) {
        const DEFAULT_INCHES_PER_METRE: f32 = MapProjectionDepth::DEFAULT_METRES_PER_INCH.recip();
        if let Some(v2) = entry.as_v2_preset() {
            return self.set_depth_range_v2(&v2)
        }

        self.depth_range = match (entry.near_value(), entry.far_value()) {
            (Some(near), None) => {
                const NEAR_FACTOR: f32 = DEFAULT_INCHES_PER_METRE * MapProjectionDepth::NEAR_FACTOR;
                let far = near * NEAR_FACTOR;
                let near = near * DEFAULT_INCHES_PER_METRE;
                Some(near..far)
            },
            (Some(near), Some(far)) => {
                let near = near * DEFAULT_INCHES_PER_METRE;
                let far = far * DEFAULT_INCHES_PER_METRE;
                Some(near..far)
            },
            _ => None,
        };
    }
    pub fn get_depth_calibration(&self) -> Option<MapProjectionDepth> {
        #[cfg(feature = "goggles2-camera")]
        if let Some((_, _, _near, far)) = self.goggles.camera.perspective_params() {
            return Some(MapProjectionDepth::with_far_in(far))
        }
        self.map_depth.clone().or(self.map_depth_guess.clone())
    }

    /// 50 degrees vertical field of view
    pub const DEFAULT_FOV_Y: Angle = Angle::from_radians(50.0f32.to_radians());
    pub fn fov_y(&self) -> Option<Angle> {
        #[cfg(todo)]
        #[cfg(feature = "goggles2-camera")]
        if let Some((h, aspect, ..)) = self.goggles.camera.perspective_params() {
            return h_persp.recip().atan() * 2.0
        }
        #[cfg(feature = "extension-nexus")]
        if let Some(fov_y) = self.rtapi_state.camera_fov_y() {
            return Some(Angle::from_radians(fov_y))
        }
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
        //0.466512
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

    /// m/inches
    pub(crate) fn depth_scale_i2m(&self) -> f32 {
        match () {
            #[cfg(todo)]
            _ => self.map.calibration.local_space().scale_length(),
            _ => MapProjectionDepth::DEFAULT_METRES_PER_INCH,
        }
    }
    pub fn get_space_perspective(&self) -> Transform3<DrawSpace, ScreenSpace> {
        #[cfg(feature = "goggles2-camera")]
        //#[cfg(deleteme)]
        if let Some((h, aspect, near, far)) = self.goggles.camera.perspective_params() {
            use glamour::Vector4;
            let w = h / aspect;
            let r = far / (far - near);
            let near_m = near * self.depth_scale_i2m();
            let persp = Matrix4::from_cols(
                Vector4::ZERO.with_x(w),
                Vector4::ZERO.with_y(h),
                Vector4::W.with_z(r),
                Vector4::ZERO.with_z(-r * near_m),
            );
            return Transform3::from_matrix_unchecked(persp)
        }
        let r = self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO);
        let Range { start: near, end: far } = self.depth_range();
        let fov = self.get_fov();
        let scaling = Self::DEFAULT_FOV_Y.to_radians() / fov.y;
        let scaling = 1.0f32;
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
            #[cfg(todo)]
            Some(..) => !self.get_map_open_state().is_visible(),
            Some(..) => match self.get_map_open_state() {
                MapOpen::Open => false,
                MapOpen::Closed => true,
                state => state.is_anim(),
            },
            None => false,
        }
    }

    pub(super) fn space_map_exit(&mut self, gameplay: GameplayState, trans: GameplayTransition) {
        #[cfg(feature = "goggles")]
        {
            self.goggles.act_map_exit(gameplay, trans);
        }
        self.camera.reset();
    }

    /// If we have suddenly
    /// lost or gained a depth buffer, it typically means something!
    #[cfg(feature = "goggles")]
    pub fn turn_depth_event(&mut self, acquired_or_lost: bool) {
        #[cfg(deleteme)]
        match acquired_or_lost {
            true => (),
            false => self.goggles.lens_lost(),
        }
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

    #[cfg(feature = "goggles")]
    pub fn goggles_new_frame(&mut self, (engine,): super::RenderSlot) {
        let Some(Ok(engine)) = engine else { return };

        if !self.goggles.is_enabled(GogglesEnables::CAMERA_ENABLE) {
            self.camera.record_goggles_unavail();
        }

        // re-read prior to render since a non-trivial amount of time has passed since present/prerender!
        // (and it will still be a while until next one)
        self.lastminute_mumblelink_update();

        engine.prepare_new_frame(self);
    }

    #[cfg(feature = "goggles2-camera")]
    pub(crate) fn goggles_update_camera(&mut self, start: bool) {
        let enabled_dir = self.goggles.is_enabled(GogglesEnables::CAMERA_DIR);
        if enabled_dir && self.goggles.camera.has_camera_primary() {
            self.goggles_update_camera_unchecked();
        } else if start {
            self.camera.goggles_confident = false;
        }
        let enabled_persp = self.goggles.is_enabled(GogglesEnables::CAMERA_PERSPECTIVE);
        if enabled_persp {
            self.goggles.camera.camera_commit_perspective();
        }
        if enabled_dir /*&& enabled_persp*/ && self.goggles.camera.has_camera_fallback() {
            self.goggles_update_camera_smooth_unchecked();
        }
    }
    #[cfg(feature = "goggles2-camera")]
    fn goggles_update_camera_unchecked(&mut self) {
        self.camera.goggles_confident = true;
        let snatch_frame = self.mumblelink_frames.latest_space_tick();
        self.camera.record_goggles(snatch_frame, &GogglesShared::snatch_camera(), true);
    }
    /// the smoothed camera can still be useful if we pretend it's historic data, though beware it may overshoot?
    #[cfg(feature = "goggles2-camera")]
    fn goggles_update_camera_smooth_unchecked(&mut self) {
        let snatch_frame = self.mumblelink_frames.latest_space_tick();
        let prev_frame = match self.goggles.camera.debug_smooth {
            true => Some(snatch_frame),
            false => {
                let prev_frame = snatch_frame.wrapping_sub(1);
                self.camera.goggles.get_at(prev_frame).is_none().then_some(prev_frame)
            },
        };
        if let Some(prev_frame) = prev_frame {
            let update_up = self.goggles.camera.debug_smooth || self.camera.up.get_at(prev_frame).is_none();
            #[cfg(feature = "goggles2-project")]
            let update_up = !self.goggles.is_enabled(GogglesEnables::PROJECT_ENABLE) || update_up;
            self.camera.record_goggles(prev_frame, &GogglesShared::snatch_camera_smooth(), update_up);
        }
    }
    #[cfg(feature = "goggles")]
    pub(super) fn goggles_post_render(&mut self) {
        #[cfg(feature = "goggles2-camera")]
        {
            self.goggles_camera_post_render();
        }
        crate::render::goggles::blah_pre_render(self);
        self.goggles.act_render_post();
    }
    #[cfg(feature = "goggles")]
    pub(super) fn goggles_post_render_late(&mut self, _render_slot: super::RenderSlot) {
        self.goggles.act_render_post_late();
    }
    #[cfg(feature = "goggles2-camera")]
    pub(super) fn goggles_camera_post_render(&mut self) {
        if !self.goggles.is_enabled(GogglesEnables::CAMERA_ENABLE) {
            return
        }
        let cam = match self.get_camera_mumblelink_verbatim() {
            #[cfg(feature = "extension-nexus")]
            _ if self.rtapi_state.is_cutscene() => Err(true),
            #[cfg(feature = "extension-nexus")]
            #[cfg(todo = "unnecessary")]
            _ if self.rtapi_state.is_intermission() => Err(false),
            _ if self.gameplay.gameplay_map().is_none() => Err(false),
            cam => Ok(cam),
        };
        match cam {
            Ok(cam) => {
                let persp = (
                    self.get_fov().y,
                    self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO),
                );
                let ml_frame = self.mumblelink_frame.wrapping_add(self.mumblelink_frame_skip);
                let update_mod: u32 = match self.mumblelink_state {
                    s if !s.contains(UiState::WINDOW_FOCUS) =>
                        0x80,
                    s if s.contains(UiState::MAP_OPEN) => match self.get_map_open_state() {
                        MapOpen::Closed | MapOpen::Closing { .. } =>
                            0x08,
                        _ => 0x80,
                    },
                    _ => 0x02,
                };
                let update_mask = update_mod - 1;
                let update = (ml_frame & update_mask) as u8;
                self.goggles.camera.camera_setup(cam, persp, update);
            },
            Err(intermission) =>
                self.goggles.camera.camera_pause(intermission),
        }
    }

    fn logo_material() -> tobj::MTLLoadResult {
        tobj::load_mtl_buf(&mut &include_bytes!("../../../data/assets/taimihud.mtl")[..])
    }
    /// TODO: preprocess into vertex data in a useful format
    pub fn logo_object() -> Option<(&'static [tobj::Model], &'static [tobj::Material])> {
        static MAT: LazyLock<Option<(Vec<tobj::Model>, Vec<tobj::Material>)>> = LazyLock::new(|| {
            let lo = tobj::LoadOptions {
                reorder_data: false,
                single_index: true,
                triangulate: true,
                ignore_points: true,
                ignore_lines: true,
                // requires unstable but we don't use it...
                #[cfg(todo = "unnecessary")]
                merge_identical_points: false,
                .. Default::default()
            };
            let res = tobj::load_obj_buf(
                &mut &include_bytes!("../../../data/assets/taimihud.obj")[..],
                &lo,
                |_| RenderMachine::logo_material(),
            ).and_then(|(o, m)| m.map(|m|
                (o, m)
            ));
            match res {
                #[cfg(taimi_debug)]
                res => rt::log::debug_ok(
                    anyhow::Context::context(res, "taimihud.obj")),
                #[cfg(not(taimi_debug))]
                res => res.ok(),
            }
        });
        MAT.as_ref().map(|(o, m)| (&o[..], &m[..]))
    }
}

#[derive(Debug, Clone, Default)]
pub struct CameraState {
    pub mumblelink: FrameRecordOf<CameraPosition>,
    /// apparently more accurate when RTAPI patches are present
    pub mumblelink_confident: bool,
    mumblelink_movement: f32,
    #[cfg(feature = "goggles2-camera")]
    pub goggles: FrameRecordOf<CameraPosition>,
    /// whether sourced from the "smooth" camera or not
    #[cfg(feature = "goggles2-camera")]
    pub goggles_confident: bool,
    #[cfg(feature = "goggles2-camera")]
    goggles_movement: f32,
    movement_frame: u32,
    pub up: FrameRecordOf<Vector3<DrawSpace>>,
    render_offset: u32,
}
impl CameraState {
    pub(super) fn resync_with_frames(&mut self, frames: &MumblelinkFrames) {
        self.render_offset = frames.render_offset;
    }
    /// TODO
    pub fn is_moving(&self, frame: u32, prefer: Option<CameraSource>, frames: &MumblelinkFrames) -> bool {
        let Some((prior, next)) = self.frames_near_relaxed(frame) else {
            return false
        };
        let fallback = || frame.checked_sub(next).map(|behind| behind <= Self::MOVING_GRACE_FRAMES).unwrap_or(true);
        let prior = match prior {
            Some(prior) => prior,
            // allow for a tiny bit of frameskip?
            None => return fallback(),
        };
        match (self.get_at(prior, prefer), self.get_at(next, prefer)) {
            (Some(prior), Some(next)) => {
                let ui_tick = frames.render_to_uitick(frame);
                let frame_time = frames.duration_at(Some(ui_tick)).unwrap_or(Self::MOVING_THRESHOLD_TIME);
                let thresh = Self::MOVING_THRESHOLD * Self::MOVING_THRESHOLD_FPS * frame_time;
                !prior.pos.abs_diff_eq(next.pos, thresh)
            },
            #[cfg(debug_assertions)]
            _ => unreachable!(),
            #[cfg(todo)]
            _ => unreachable_unchecked(),
            _ => fallback(),
        }
    }
    /// amount of camera eye movement across a time period of [Self::MOVING_THRESHOLD_TIME]
    const MOVING_THRESHOLD: f32 = 1.4 * MapLocalScale::METRES_PER_INCH;
    /// 50 FPS
    const MOVING_THRESHOLD_TIME: f32 = 50.0f32.recip();
    const MOVING_THRESHOLD_FPS: f32 = Self::MOVING_THRESHOLD_TIME.recip();
    /// allow for a tiny bit of frameskip?
    const MOVING_GRACE_FRAMES: u32 = 1;
    pub fn get_at(&self, render_frame: u32, prefer: Option<CameraSource>) -> Option<&CameraPosition> {
        let ui_frame = render_frame.wrapping_add(self.render_offset);
        let ml = self.mumblelink.get_at(ui_frame);
        #[cfg(feature = "goggles2-camera")]
        let goggles = self.goggles.get_at(render_frame);
        match ml {
            #[cfg(feature = "goggles2-camera")]
            _ if goggles.is_some() && (self.goggles_confident || prefer == Some(CameraSource::Goggles2)) =>
                goggles,
            #[cfg(feature = "goggles2-camera")]
            ml if (!self.mumblelink_confident && prefer != Some(CameraSource::MumbleLink)) || ml.is_none() =>
                goggles.or(ml),
            ml => ml,
        }
    }
    fn populated_frames(&self) -> impl Iterator<Item = u32> + '_ {
        let render_offset = self.render_offset;
        let populated_frames = self.mumblelink.iter_populated().map(move |(pos, ..)| pos.wrapping_sub(render_offset));
        #[cfg(feature = "goggles2-camera")]
        let populated_frames = populated_frames.chain(self.goggles.iter_populated().map(|(pos, ..)| pos));
        populated_frames
    }
    pub fn get_or_interpolate(&self, frame: u32, prefer: Option<CameraSource>, _frames: &MumblelinkFrames) -> Option<CameraPosition> {
        if let Some(cam) = self.get_at(frame, prefer) {
            return Some(*cam)
        }
        #[cfg(feature = "goggles2-camera")]
        if _frames.is_ui_tick_overdue() {
            match self.goggles.iter_populated().next() {
                Some((tick, cam)) if frame.wrapping_sub(tick) <= Self::SKIP_TICK_THRESHOLD || frame_is_lt(frame, tick) =>
                    return Some(*cam),
                _ => (),
            }
        }
        let (prior, next) = self.frames_near_strict(frame)?;

        Some(self.interpolate_between(
            frame, prior, next,
            Self::INTERP_PARAMS_DEFAULT,
            prefer,
        ))
    }
    pub fn debug_interpolate(&self, frame: u32, prefer: Option<CameraSource>, _frames: &MumblelinkFrames, force_off: bool) -> Option<CameraPosition> {
        use taimi_meta::spatial::record::frame_is_gt;
        if force_off {
            if let Some(cam) = self.get_at(frame, prefer) {
                return Some(*cam)
            }
        }

        let frame = match force_off {
            #[cfg(deleteme)]
            false => if let Some((i, ..)) = _frames.latest_tick() {
                let latest_render = _frames.uitick_to_render(i);
                if !frame_is_lt(frame, latest_render) {
                    frame.wrapping_sub(1)
                } else {
                    frame
                }
            } else { frame },
            _ => frame,
        };
        let (prior, next) = match self.frames_near_strict(frame) {
            Some(frames) => frames,
            #[cfg(todo)]
            None => (Some(frame.wrapping_sub(2)), frame.wrapping_sub(1)),
            None => return None,
        };

        match force_off {
            true => {
                let choose_next = !frame_is_gt(next, frame);
                let cam_next = self.get_at(next, prefer);
                let cam_prior = || prior.and_then(|prior|
                    self.get_at(prior, prefer)
                );
                (!choose_next).then(cam_prior).flatten().or(cam_next).copied()
            },
            false => Some(self.interpolate_between(
                frame, prior, next,
                (0.05f32, true),
                prefer,
            )),
        }
    }
    fn interpolate_between(
        &self,
        frame: u32,
        prior: Option<u32>,
        next: u32,
        (param_adj, dir_extrapolate): (f32, bool),
        prefer: Option<CameraSource>,
    ) -> CameraPosition {
        let cam_next = self.get_at(next, prefer);
        debug_assert!(cam_next.is_some());
        let Some(cam_next) = cam_next else {
            if !crate::built_info::IS_TAGGED_VERSION {
                log::debug!("interp missing cam={next}");
            }
            return RenderMachine::POSITIONING_EMPTY.into()
        };

        let cam_prior = prior.and_then(|prior|
            self.get_at(prior, prefer)
        );
        debug_assert!(cam_prior.is_some());
        let (Some(prior), Some(cam_prior)) = (prior, cam_prior) else {
            return *cam_next
        };
        #[cfg(todo)]
        let interp_time = match (frames.timestamp_at(prior), frames.timestamp_at(next)) {
            // XXX: use frametimes instead if avail?
            (Some(ts_prior), Some(ts_next)) => ts_next.saturating_duration_since(*ts_prior),
            _ => 0.0f32,
        };
        let interp_frame = {
            let interp_len = next.wrapping_sub(prior) as f32;
            let interp_pos = match frame.wrapping_sub(prior) {
                pos if frame_is_lt(next, frame) =>
                    pos as f32 + param_adj,
                pos => pos as f32,
            };
            interp_pos / interp_len
        };
        let interp = interp_frame;
        let pos = cam_prior.pos.lerp(cam_next.pos, interp);
        let front = match dir_extrapolate {
            dir_extrapolate => {
                let interp = match dir_extrapolate {
                    false => interp.min(1.0),
                    true => interp,
                };
                let (up_prior, up_next) = match () {
                    #[cfg(todo)]
                    _ => {
                        // messy and unnecessary
                        let up_prior = self.up_at_with(prior, 1);
                    },
                    _ => (RenderMachine::LOCAL_UP.to_raw(), RenderMachine::LOCAL_UP.to_raw()),
                };
                let rot_prior = Quat::look_to_lh(cam_prior.front.into(), up_prior);
                let rot_next  = Quat::look_to_lh(cam_next.front.into(), up_next);
                let front = rot_prior.lerp(rot_next, interp);
                match front {
                    #[cfg(todo = "unnecessary")]
                    front => decompose_look32::<f32>(Mat4::from_quat(front)).1,
                    front => front.conjugate()
                        .mul_vec3a(RenderMachine::LOCAL_FORWARD.to_vec3a())
                        .try_normalize().map(From::from)
                        .unwrap_or(cam_prior.front),
                }
            },
            #[cfg(todo = "unnecessary")]
            _ => cam_prior.front.slerp(cam_next.front, interp).normalize_or(cam_next.front),
        };
        CameraPosition {
            pos,
            front,
        }
    }
    const INTERP_CONSERVATIVE_EXTRAPOLATION: f32 = -0.1f32;
    const INTERP_PARAMS_DEFAULT: (f32, bool) = {
        let extrapolate = true;
        (Self::INTERP_CONSERVATIVE_EXTRAPOLATION, extrapolate)
    };
    pub fn frames_near_strict(&self, frame: u32) -> Option<(Option<u32>, u32)> {
        self.frames_near(frame, true)
    }
    fn frames_near_relaxed(&self, frame: u32) -> Option<(Option<u32>, u32)> {
        self.frames_near(frame, false)
    }
    fn frames_near(&self, frame: u32, strict: bool) -> Option<(Option<u32>, u32)> {
        let sort_key = |&pos: &u32| {
            let mut delta = pos.wrapping_sub(frame);
            let behind = match delta {
                0 if strict => return u32::MAX,
                #[cfg(todo = "unnecessary")]
                _ => frame_is_lt(pos, frame),
                delta => delta & 0x80000000u32 != 0,
            };
            if behind {
                // pseudo-abs the difference, using the sign to prefer future (positive) frames
                delta ^= 0x7fffffffu32;
            }
            delta
        };
        let next = self.populated_frames().min_by_key(sort_key);
        let past = self.populated_frames().filter(|&pos| frame_is_lt(pos, frame) && Some(pos) != next);
        let (prior, next) = match (past.min_by_key(sort_key), next) {
            (_, None) => return None,
            (prior, Some(next)) => (prior, next),
            #[cfg(todo = "unnecessary")]
            (Some(prior), None) => (self.populated_frames().filter(|&pos| frame_is_lt(pos, prior)).min_by_key(sort_key), prior),
        };
        Some((prior, next))
    }
    pub fn up_at(&self, frame: u32) -> Vector3<DrawSpace> {
        self.up_at_with(frame, Self::UP_TICK_THRESHOLD)
            .unwrap_or(RenderMachine::LOCAL_UP)
    }
    fn up_at_with(&self, frame: u32, thresh: u32) -> Option<Vector3<DrawSpace>> {
        self.up.get_at(frame).copied()
            .or_else(|| self.up.iter_populated().next().and_then(|(up_tick, up)| {
                let recent_cutoff = frame.wrapping_sub(thresh);
                let up_recent = !frame_is_lt(up_tick, recent_cutoff);
                up_recent.then_some(*up)
            }))
    }
    const UP_TICK_THRESHOLD: u32 = 1;
    const SKIP_TICK_THRESHOLD: u32 = 3;
    pub(super) fn record_mumblelink(&mut self, frame: u32, (pos, front, up): RenderPosition) {
        let entry = (pos, front).into();
        if let Some(prev) = self.mumblelink.front().get_opt() {
            let movement = prev.pos.distance_squared(pos.to_vec3a());
            self.mumblelink_movement = self.mumblelink_movement * Self::MOVEMENT0 + movement * Self::MOVEMENT1;
        }
        if self.mumblelink_movement > Self::MOVEMENT_THRESHOLD {
            self.movement_frame = frame;
        }
        self.mumblelink.set_at(frame, entry);
        #[cfg(todo)]
        if !vec32_eq(up, Vector3::ZERO) {
            let render_frame = frame.wrapping_sub(self.render_offset);
            self.up.set_at(render_frame, up);
        }
    }
    const MOVEMENT0: f32 = 0.4;
    const MOVEMENT1: f32 = 1.0 - Self::MOVEMENT0;
    const MOVEMENT_THRESHOLD: f32 = 2e-4f32;
    #[cfg(feature = "goggles2-camera")]
    pub(super) fn record_goggles(&mut self, frame: u32, cam: &SnatchMatrix, include_up: bool) {
        let pos = CameraPosition::from(cam.get_as_look());
        if let Some(prev) = self.goggles.front().get_opt() {
            let movement = prev.pos.distance_squared(pos.pos);
            self.goggles_movement = self.goggles_movement * Self::MOVEMENT0 + movement * Self::MOVEMENT1;
            if self.goggles_movement > Self::MOVEMENT_THRESHOLD {
                self.movement_frame = frame;
            }
        }
        self.goggles.set_at(frame, pos);
        if include_up /*&& self.up.get_at(frame).is_none()*/ {
            self.up.set_at(frame, cam.get_look_up());
        }
    }
    #[cfg(feature = "goggles2-camera")]
    pub(super) fn record_goggles_unavail(&mut self) {
        if !self.goggles.front().is_empty() | !self.goggles.back().is_empty() {
            for (_, pos) in self.goggles.iter_all_mut() {
                pos.clobber();
            }
        }
    }

    pub fn position(&self) -> u32 {
        let pos = self.mumblelink.position;
        #[cfg(feature = "goggles2-camera")]
        let pos = pos.max(self.goggles.position);
        pos
    }
    /// TODO: ?
    pub fn id(&self) -> Option<NonZero<usize>> {
        let id = self.movement_frame as usize;
        NonZero::new(id)
    }
    pub fn reset(&mut self) {
        self.movement_frame = 0;
        self.mumblelink_movement = 0.0;
        self.mumblelink.advance_to(self.mumblelink.position.wrapping_add(4));
        #[cfg(feature = "goggles2-camera")]
        {
            self.goggles_movement = 0.0;
            self.goggles.advance_to(self.goggles.position.wrapping_add(4));
        }
    }

    pub fn movement(&self) -> f32 {
        let movement = self.mumblelink_movement;
        #[cfg(feature = "goggles2-camera")]
        let movement = movement.max(self.goggles_movement);
        movement
    }
}
#[derive(Debug, Copy, Clone, Default)]
pub struct CameraPosition {
    pub pos: Vec3A,
    pub front: Vec3A,
    #[cfg(todo)]
    pub interpolated: bool,
}
impl From<RenderPositioning> for CameraPosition {
    fn from((pos, front, ..): RenderPositioning) -> Self {
        Self {
            pos: pos.into(),
            front: front.into(),
        }
    }
}
impl From<RenderPosition> for CameraPosition {
    fn from((pos, front, ..): RenderPosition) -> Self {
        Self {
            pos: pos.into(),
            front: front.into(),
        }
    }
}
impl CameraPosition {
    pub fn pos(&self) -> Point3<DrawSpace> {
        self.pos.into()
    }
    pub fn front(&self) -> Vector3<DrawSpace> {
        self.front.into()
    }
}
impl FrameRecordEntry for CameraPosition {
    const EMPTY: Self = Self {
        pos: FrameRecordEntry::EMPTY,
        front: FrameRecordEntry::EMPTY,
    };
    fn is_empty(&self) -> bool { self.pos.is_empty() }
    fn clobber(&mut self) { self.pos.clobber(); }
}

#[cfg(deleteme)]
#[cfg(feature = "goggles")]
#[derive(Debug, Clone, Default)]
pub struct GogglesState {
    pub enabled: bool,
    /// requested features
    pub enabled_config: GogglesEnables,
    pub active: GogglesEnables,
    #[cfg(feature = "goggles2")]
    pub is_drawing: bool,
    #[cfg(feature = "goggles2")]
    pub inherit_render: bool,
    #[cfg(feature = "goggles2")]
    pub project_depth_fill: bool,
    #[cfg(feature = "goggles2")]
    pub project_viewport_force: bool,
    #[cfg(feature = "goggles2")]
    pub project_shadow: bool,
    #[cfg(feature = "goggles2")]
    pub project_blend_force: bool,
    #[cfg(feature = "goggles2-project")]
    pub project_enabled: bool,
    #[cfg(feature = "goggles2-project")]
    pub project_projecting: bool,
    #[cfg(feature = "goggles2-project")]
    pub project_flush: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_enabled: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_paused: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_lost: u16,
    #[cfg(feature = "goggles2-camera")]
    pub camera_debug_toggle_up: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_debug_smooth: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_debug_interpolate: bool,
    #[cfg(feature = "goggles2-camera")]
    pub camera_debug_interpolate_off: bool,
    #[cfg(feature = "goggles2-camera")]
    pub perspective_lost: u16,
    #[cfg(feature = "goggles2-camera")]
    pub perspective_params: (f32, f32, f32, f32),
}
#[cfg(deleteme)]
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
        #[cfg(feature = "goggles2-project")]
        if self.project_enabled {
            FerretResource::project_reset_frame();
        }
    }

    pub(crate) fn act_pre_render(&mut self, _visible: bool) {
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
        #[cfg(feature = "goggles2-project")]
        if self.project_enabled {
            FerretResource::project_report_frame();
        }
        #[cfg(feature = "goggles2-camera")]
        if _visible {
            self.camera_commit_perspective();
        }
    }
    pub(crate) fn prepare_frame(&mut self) {
        #[cfg(feature = "goggles2-project")]
        if self.project_enabled {
            #[cfg(feature = "goggles2-camera")]
            {
                self.camera_commit_perspective();
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
    pub(crate) fn settings_enables(&self, settings: &SpaceSettings) -> GogglesEnables {
        let cam = settings.camera_source == Some(CameraSource::Goggles2);
        let project = settings.goggles.project_enabled();
        let enabled = settings.goggles.enabled();
        let project_shadow = settings.goggles.project_shadowboxing();
        (enabled, enabled && cam, (enabled && project, project_shadow))
    }
    pub(crate) fn act_enable(&mut self, (en, en_cam, (en_proj, en_proj_shadowbox)): GogglesEnables) {
        if en | en_cam | en_proj {
            #[cfg(feature = "goggles")]
            goggles::enable();
        } else {
            #[cfg(feature = "goggles")]
            goggles::disable();
            return
        }
        #[cfg(feature = "goggles")]
        if en {
            #[cfg(todo)]
            goggles::clear_lens();
        }
        #[cfg(feature = "goggles2-camera")]
        if en_cam || en {
            self.camera_enable();
        } else if self.camera_enabled {
            self.camera_disable();
        }
        #[cfg(feature = "goggles2-project")]
        if en_proj || en {
            self.project_enable();
            match en_proj_shadowbox {
                true => FerretResource::project_set_shadowbox_request(Some(ProjectRequest::default_shadowbox())),
                false => FerretResource::project_set_shadowbox_request(None),
            }
        } else if self.project_enabled {
            self.project_disable();
        }
    }
    #[cfg(feature = "space")]
    pub(crate) fn setup_engine(&mut self, engine: &Engine, enables: GogglesEnables) {
        #[cfg(feature = "goggles")]
        goggles::lens::classify_space_lens(engine);
        self.act_enable(enables);
    }
}
#[cfg(deleteme)]
type GogglesEnables = (bool, bool, (bool, bool));
#[cfg(deleteme)]
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
    pub(super) fn camera_setup(&mut self, cam: Option<RenderPosition<DrawSpace>>, (fov_y, aspect): (f32, f32), update: u8) {
        self.camera_paused = false;
        let cam = match cam {
            Some(..) if Self::camera_lost_defer(&mut self.camera_lost, update) => None,
            cam => cam,
        };
        if let Some((pos, dir, up)) = cam {
            let up = up.normalize_or(RenderMachine::LOCAL_UP);
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

    pub(super) fn has_camera_primary(&self) -> bool {
        FerretResource::has_found_camera() && !FerretResource::wants_snatch_camera()
    }
    /// TODO
    pub(super) fn has_camera(&self) -> bool {
        if !self.camera_enabled { return false }
        match () {
            _ if self.has_camera_primary() =>
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
    fn camera_commit_perspective(&mut self) {
        if self.camera_enabled && !self.camera_paused && !FerretResource::wants_snatch_perspective() {
            let persp = FerretResource::snatch_perspective();
            if self.perspective_params().is_none() {
                let (h, range) = persp.get_as_perspective();
                let (_near, far) = (range.start, range.end);
                let map_id = crate::exports::runtime::mumble_link_ptr()
                    .map(|ml| ml.read_map_id()).unwrap_or(0);
                log::error!("ON.mapid={map_id:04} FOUND NEW PERSP.far = {far:?} ({_near}..{far}) h={h:?}");
            }
            self.perspective_params = Self::perspective_params_for(&persp);
        }
    }
}
#[cfg(deleteme)]
#[cfg(feature = "goggles2-project")]
impl GogglesState {
    pub(crate) fn project_enable(&mut self) {
        self.project_enabled = true;
        if !FerretResource::project_enabled() {
            FerretResource::project_set_target_request(Some(ProjectRequest::default_target()));
        }
    }
    pub(crate) fn project_disable(&mut self) {
        self.project_enabled = false;
        FerretResource::project_set_shadowbox_request(None);
        FerretResource::project_set_target_request(None);
    }
    pub(crate) fn project_draw_start(&mut self, frames: &mut MumblelinkFrames) -> bool {
        if !self.project_enabled { return false }
        self.project_projecting = true;
        frames.render_offset_space = 1;
        true
    }
    pub(crate) fn project_draw_end(&mut self, frames: &mut MumblelinkFrames) {
        self.project_projecting = false;
        frames.render_offset_space = 0;
    }
    pub(crate) fn project_is_projecting(&self) -> bool {
        self.project_projecting
    }
}
