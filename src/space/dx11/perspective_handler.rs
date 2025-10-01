use {
    super::PerspectiveInputData,
    crate::{
        settings::pathing::SpaceSettings,
        space::{max_depth, min_depth, MapTarget, ScreenSpace},
    },
    glam::{Mat4, Vec2, Vec3, Vec4, Quat},
    glamour::Size2,
    taimi_d3d::{
        dx11::{
            prelude::*,
            buffer::{ConstantBufferP, ConstantBufferV},
        },
        D3dContextBindableSlot,
    },
};

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct PerspectiveData {
    pub view: Mat4,
    pub projection: Mat4,
    /// Billboard transform for current camera.
    pub billboard: Mat4,
    pub player: Vec4,
    pub expand: Vec4,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct PixelData {
    distance_param: Vec4,
}

pub struct PerspectiveHandler {
    constant_buffer: ConstantBufferV,
    pub constant_buffer_data: PerspectiveData,
    constant_buffer_pixel: ConstantBufferP,
    pub constant_buffer_pixel_data: PixelData,
    constant_buffer_mapv: ConstantBufferV,
    pub constant_buffer_mapv_data: MapDataV,
    constant_buffer_mapp: ConstantBufferP,
    pub constant_buffer_mapp_data: MapDataP,
    aspect_ratio: f32,
    pub alpha: f32,
    up: Vec3,
    near: f32,
    far: f32,
    display_size: Size2<ScreenSpace>,
}

impl PerspectiveHandler {
    pub fn setup(device: &Dx11Device, display_size: Size2<ScreenSpace>) -> anyhow::Result<Self> {
        let aspect_ratio = display_size.width / display_size.height;
        let constant_buffer_data = PerspectiveData::INITIAL;
        let constant_buffer = ConstantBufferV::new_with_data(device, &constant_buffer_data)?;
        let constant_buffer_pixel_data = PixelData::INITIAL;
        let constant_buffer_pixel = ConstantBufferP::new_with_data(device, &constant_buffer_pixel_data)?;
        let constant_buffer_mapv_data = MapDataV::INITIAL;
        let constant_buffer_mapv = ConstantBufferV::new_with_data(device, &constant_buffer_mapv_data)?;
        let constant_buffer_mapp_data = MapDataP::INITIAL;
        let constant_buffer_mapp = ConstantBufferP::new_with_data(device, &constant_buffer_mapp_data)?;
        Ok(Self {
            up: Vec3::ZERO.with_y(1.0),
            aspect_ratio,
            alpha: 1.0,
            display_size,
            constant_buffer,
            constant_buffer_data,
            constant_buffer_pixel,
            constant_buffer_pixel_data,
            constant_buffer_mapv,
            constant_buffer_mapv_data,
            constant_buffer_mapp,
            constant_buffer_mapp_data,
            near: min_depth(),
            far: max_depth(),
        })
    }

    pub fn prepare(&mut self, display_size: Size2<ScreenSpace>) {
        if display_size != self.display_size {
            self.aspect_ratio = display_size.width / display_size.height;
            self.display_size = display_size;
        }
    }

    pub fn update_perspective(&mut self, poi_scale: Vec3) {
        let data = PerspectiveInputData::get();

            self.constant_buffer_data.view = Mat4::look_to_lh(data.pos, data.front, self.up);
            self.near = min_depth();
            self.far = max_depth();
            self.constant_buffer_data.projection =
                Mat4::perspective_lh(data.fov(), self.aspect_ratio, self.near, self.far);
        self.constant_buffer_data.player =
            data.player_pos().extend(self.alpha).to_raw();

        self.constant_buffer_data.billboard = {
            let cam_front = data.front.normalize();
            let cam_right = cam_front.cross(self.up).normalize();
            let cam_up = cam_right.cross(cam_front).normalize();
            Mat4::from_cols(
                cam_right.extend(0.0),
                cam_up.extend(0.0),
                -cam_front.extend(0.0),
                Vec4::ZERO.with_w(1.0),
            )
        } * Mat4::from_scale(poi_scale);
    }

    pub fn aspect_ratio(&self) -> f32 {
        self.aspect_ratio
    }

    pub fn near(&self) -> f32 {
        self.near
    }

    pub fn far(&self) -> f32 {
        self.far
    }

    pub fn alpha(&self) -> f32 {
        //self.constant_buffer_data.player.w
        self.alpha
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha;
        self.constant_buffer_data.player.w = self.alpha;
    }

    pub fn update_cb(&self, device_context: &Dx11Context) {
        self.constant_buffer.update_singleton(device_context, &self.constant_buffer_data);
        self.constant_buffer_pixel.update_singleton(device_context, &self.constant_buffer_pixel_data);
    }

    pub fn update_map(&mut self, map: &MapTarget) {
        let bounds = &map.bounds_draw;
        let size = bounds.size();
        let (left, right) = (bounds.min.x, bounds.max.x);
        let (bottom, top) = (bounds.min.z, bounds.max.z);
        let (near, far) = (0.001f32, 1000.0f32);
        let mid = Vec3::new(map.centre.x, bounds.center().y, map.centre.y);
        let (map_rotation, counter_rot) = match map.rotation {
            Some(amt) => (Quat::from_rotation_y(-amt.to_radians()), Quat::from_rotation_y(amt.to_radians())),
            None => (Quat::IDENTITY, Quat::IDENTITY),
        };
        let trans = Vec3::new(-mid.x, -mid.y, -mid.z);
        self.constant_buffer_mapv_data.model = Mat4::from_quat(counter_rot);
        self.constant_buffer_mapv_data.world = Mat4::from_quat(map_rotation) * Mat4::from_scale_rotation_translation(
            Vec3::ONE,
            Quat::IDENTITY,
            trans,
        );
        let screen_mid = map.bounds_screen.center();
        let screen_sz = map.bounds_screen.size();
        let screen = self.display_size;
        let scl = Vec3::new(screen_sz.width / screen.width, screen_sz.height / screen.height, 1.0);
        let window_trans = Vec2::new(-mid.x, -mid.z);
        let window_trans = Vec2::new(window_trans.x - (screen_mid.x / screen.width - 0.5) * size.width, window_trans.y + (screen_mid.y / screen.height - 0.5) * size.depth);
        self.constant_buffer_mapv_data.view = Mat4::orthographic_lh(
            left + window_trans.x, right + window_trans.x,
            bottom + window_trans.y, top + window_trans.y,
            near, far,
        ) * Mat4::from_scale_rotation_translation(
            scl,
            Quat::IDENTITY,
            Vec3::ZERO,
        );
    }

    pub fn update_map_cb(&self, device_context: &Dx11Context) {
        self.constant_buffer_mapv.update_singleton(device_context, &self.constant_buffer_mapv_data);
        self.constant_buffer_mapp.update_singleton(device_context, &self.constant_buffer_mapp_data);
    }
    pub fn set_map_cb(&self, device_context: &Dx11Context, slot: u32) {
        self.constant_buffer_mapv.set(device_context, slot);
        self.constant_buffer_mapp.set(device_context, slot);
    }
}

impl D3dContextBindableSlot<Dx11Context> for PerspectiveHandler {
    fn set(&self, context: &Dx11Context, slot: u32) {
        self.constant_buffer.set(context, slot);
        self.constant_buffer_pixel.set(context, slot);
    }
}

impl PerspectiveData {
    pub const INITIAL: Self = Self {
        view: Mat4::IDENTITY,
        projection: Mat4::IDENTITY,
        player: Vec4::ZERO,
        billboard: Mat4::IDENTITY,
        expand: Vec4::ZERO,
    };
}

unsafe impl D3dBufferData for PerspectiveData {}

impl PixelData {
    pub const INITIAL: Self = Self {
        distance_param: Vec4::new(Self::OVERLAP_DEFAULT, Self::INTENSITY_DEFAULT, 0.0, 0.0),
    };

    pub const OVERLAP_THRESHOLD_OFF: f32 = 0.01;
    pub const OVERLAP_THRESHOLD_DEFAULT: f32 = SpaceSettings::DEFAULT_PLAYER_OVERLAP_THRESHOLD;
    pub const OVERLAP_DEFAULT: f32 = Self::OVERLAP_THRESHOLD_DEFAULT;

    pub const INTENSITY_OFF: f32 = 1_000_000.0;
    pub const INTENSITY_DEFAULT: f32 = SpaceSettings::DEFAULT_DISTANCE_FADE_INTENSITY;

    pub fn set_overlap_threshold(&mut self, threshold: Option<f32>) {
        self.distance_param.x = match threshold {
            Some(thresh) => thresh,
            None => Self::OVERLAP_THRESHOLD_OFF,
        };
    }

    pub fn overlap_threshold(&self) -> f32 {
        self.distance_param.x
    }

    pub fn set_intensity(&mut self, intensity: Option<f32>) {
        self.distance_param.y = match intensity {
            Some(v) => v,
            None => Self::INTENSITY_OFF,
        };
    }

    pub fn intensity(&self) -> Option<f32> {
        match self.distance_param.y {
            Self::INTENSITY_OFF => None,
            intensity => Some(intensity),
        }
    }
}

unsafe impl D3dBufferData for PixelData {}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct MapDataV {
    pub model: Mat4,
    pub world: Mat4,
    pub view: Mat4,
    pub expand: Vec4,
}

impl MapDataV {
    pub const INITIAL: Self = Self {
        model: Mat4::IDENTITY,
        world: Mat4::IDENTITY,
        view: Mat4::IDENTITY,
        expand: Vec4::ZERO,
    };
}

impl Default for MapDataV {
    fn default() -> Self {
        Self::INITIAL
    }
}

unsafe impl D3dBufferData for MapDataV {}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct MapDataP {
    pub colour: Vec4,
}

impl MapDataP {
    pub const INITIAL: Self = Self {
        colour: Vec4::ONE,
    };
}

unsafe impl D3dBufferData for MapDataP {}
