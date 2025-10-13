use {
    core::ops::Range,
    crate::{
        render::machine::{RenderMachine, RenderPosition, RenderPositioning},
        settings::pathing::CameraSource,
        space::{
            pack::FestivalFixup,
            DrawSpace,
        },
    },
    glamour::{
        Angle,
        Matrix4,
        Point3,
        Transform3,
        Vector2, Vector3,
    },
    std::{
        collections::BTreeSet,
        time::SystemTime,
    },
    taimi_meta::{
        coords::{
            camera_view,
            ScreenSpace,
        },
        ui::MapOpen,
    },
    taimi_pack::attributes::Festival,
};

impl RenderMachine {
    const CAMERA_SMOOTHING_PER_FRAME: f32 = 0.135;
    pub fn get_camera_pos(&self, source: CameraSource) -> Option<RenderPositioning<DrawSpace>> {
        #[cfg(feature = "extension-nexus")]
        let has_rtapi = !self.rtapi_state.camera.0.x.is_infinite();
        match source {
            #[cfg(feature = "extension-nexus")]
            CameraSource::RealTimeAPI if has_rtapi =>
                Some(self.rtapi_state.camera),
            #[cfg(feature = "extension-nexus")]
            CameraSource::MumbleLink if self.mumblelink_frame_skip > 0 && has_rtapi => Some({
                let factor = Self::CAMERA_SMOOTHING_PER_FRAME * self.mumblelink_frame_skip as f32 /*.min(1.0)*/;
                let (ml_pos, ml_front) = &self.mumblelink_camera;
                let (pos, front) = self.rtapi_state.camera;
                (
                    ml_pos.lerp(pos, factor),
                    ml_front.slerp(front, factor),
                    //.rotate_towards(front, turn * Self::CAMERA_SMOOTHING_PER_FRAME)?
                )
            }),
            _ => self.get_camera_mumblelink(),
        }
    }

    pub fn get_camera(&mut self, source: CameraSource) -> RenderPosition<DrawSpace> {
        // TODO: cache direct ptr per frame
        let (pos, front) = self.get_camera_pos(source)
            .unwrap_or((Point3::ZERO, Vector3::ZERO));
        (pos, front.normalize_or(Self::LOCAL_FORWARD), Self::LOCAL_UP)
    }

    pub const DEFAULT_DEPTH_RANGE: Range<f32> = {
        use taimi_meta::coords::MapLocalScale;

        let near = 2.0 / MapLocalScale::METRES_PER_FEET;
        let far = 2000.0 / MapLocalScale::METRES_PER_FEET;
        near..far
    };

    pub fn get_depth_range(&self) -> Option<Range<f32>> {
        self.depth_range.clone()
    }

    /// 50 degrees vertical field of view
    pub const DEFAULT_FOV_Y: Angle = Angle::from_radians(0.8730f32);
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
        0.466512
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
        let r = self.aspect_ratio().unwrap_or(Self::DEFAULT_ASPECT_RATIO);
        let Range { start: near, end: far } = self.get_depth_range().unwrap_or(Self::DEFAULT_DEPTH_RANGE);
        Transform3::from_matrix_unchecked(match self.get_fov() {
            fov => {
                Matrix4::perspective_lh(Angle::new(fov.y), r, near, far)
            },
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
            camera_view(camera_pos, camera_front, camera_up)
            //glam::Mat4::look_to_lh(camera_pos.into(), camera_front.into(), camera_up.into()).into()
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
                (true, MapOpen::Open) =>
                    self.set_map_open(MapOpen::Closing { elapsed: 0.0 }),
                (false, MapOpen::Opening { .. }) =>
                    self.set_map_open(MapOpen::Open),
                _ => false,
            };
            if changed {
                self.act_map_open();
            }
        }
    }

    pub fn initial_festivals() -> impl Iterator<Item = Festival> {
        let now = SystemTime::now();
        FestivalFixup::FESTIVAL_WINDOWS.iter()
            .filter(move |(_f, window)| window.is_active(now))
            .map(|&(festival, ..)| festival)
    }

    #[inline]
    pub fn festival_active(&self, festival: Festival) -> bool {
        self.active_festivals.contains(&festival)
    }
}
