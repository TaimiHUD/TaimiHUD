use {
    crate::{
        render::machine::{RenderMachine, RenderPosition},
        settings::pathing::SpaceSettings,
        controller::pathing::space::{PoiScale, TrailScale, TrailTextureMap},
        space::ScreenSpace,
    },
    glam::{Mat4, Quat, Vec2, Vec3, Vec4},
    glamour::{Box2, Box3, Point3, Size2, TransformMap},
    taimi_d3d::{
        dx11::{
            buffer::{ConstantBufferP, ConstantBufferV},
            prelude::*,
        },
        D3dContextBindableSlot,
    },
    taimi_meta::{
        coords::{billboard_from_look, LocalSpace},
        ui::MapContext,
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
    pub trail_expansion: TrailScale,
    /// unused?
    pub _poi_expansion: PoiScale,
    pub trail_texture: TrailTextureMap,
}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct PixelData {
    distance_param: Vec4,
    viewport_param: Vec4,
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
    pub alpha: f32,
}

impl PerspectiveHandler {
    pub fn setup(device: &Dx11Device) -> anyhow::Result<Self> {
        let constant_buffer_data = PerspectiveData::INITIAL;
        let constant_buffer = ConstantBufferV::new_with_data(device, &constant_buffer_data)?;
        let constant_buffer_pixel_data = PixelData::INITIAL;
        let constant_buffer_pixel = ConstantBufferP::new_with_data(device, &constant_buffer_pixel_data)?;
        let constant_buffer_mapv_data = MapDataV::INITIAL;
        let constant_buffer_mapv = ConstantBufferV::new_with_data(device, &constant_buffer_mapv_data)?;
        let constant_buffer_mapp_data = MapDataP::INITIAL;
        let constant_buffer_mapp = ConstantBufferP::new_with_data(device, &constant_buffer_mapp_data)?;
        Ok(Self {
            alpha: 1.0,
            constant_buffer,
            constant_buffer_data,
            constant_buffer_pixel,
            constant_buffer_pixel_data,
            constant_buffer_mapv,
            constant_buffer_mapv_data,
            constant_buffer_mapp,
            constant_buffer_mapp_data,
        })
    }

    pub fn update_perspective(
        &mut self,
        machine: &mut RenderMachine,
        camera: RenderPosition,
        poi_scale: Vec3,
    ) {
        let (player_pos, ..) = machine.get_player();
        self.constant_buffer_data.player = player_pos.extend(self.alpha).to_raw();
        self.constant_buffer_data.projection = machine.get_space_perspective().matrix.into();

        let view = RenderMachine::space_view(camera).matrix;
        self.constant_buffer_data.billboard =
            Mat4::from(billboard_from_look(view)) * Mat4::from_scale(poi_scale);
        self.constant_buffer_data.view = view.into();
    }

    pub fn alpha(&self) -> f32 {
        //self.constant_buffer_data.player.w
        self.alpha
    }

    pub fn set_alpha(&mut self, alpha: f32) {
        self.alpha = alpha;
        self.constant_buffer_data.player.w = self.alpha;
    }

    pub const FEATHER_SCALE_SQUARE: Vec2 = Vec2::new(0.065, 0.0825);
    pub fn set_feather_scale(&mut self, feather_scale: Option<f32>, display_size: Size2<ScreenSpace>) {
        let aspect_ratio = display_size.width / display_size.height;
        let feather = feather_scale.map(|scale| {
            let normalized = Vec2::new(1.0 / aspect_ratio, aspect_ratio) * Self::FEATHER_SCALE_SQUARE;
            scale / normalized
        });
        self.set_feather(feather, display_size)
    }

    pub fn set_feather(&mut self, feather_scale: Option<Vec2>, display_size: Size2<ScreenSpace>) {
        let viewport_size = feather_scale.is_some().then_some(display_size.to_raw());
        self.constant_buffer_pixel_data.set_viewport_size(viewport_size);
        self.constant_buffer_pixel_data.set_feather_scale(feather_scale);
    }

    pub fn update_cb(&self, device_context: &Dx11Context) {
        self.constant_buffer
            .update_singleton(device_context, &self.constant_buffer_data);
        self.constant_buffer_pixel
            .update_singleton(device_context, &self.constant_buffer_pixel_data);
    }

    const HEIGHT_OFFSET_BELOW: f32 = -200.0;
    const HEIGHT_OFFSET_ABOVE: f32 = 200.0;
    const HEIGHT_OFFSET_BELOW_MINI: f32 = -75.0;
    const HEIGHT_OFFSET_ABOVE_MINI: f32 = 95.0;

    pub fn map_local_bounds(machine: &mut RenderMachine) -> Box3<LocalSpace> {
        let map_to_local = machine.map.calibration.map_to_local();
        let open = machine.map_open();
        let ctx = machine.is_map_visible().unwrap_or(open.into());
        let fake_to_map = match ctx {
            #[cfg(todo)]
            _ => machine.map.fake_to_worldmap_for(ctx),
            MapContext::Global => machine.map.calibration.fake_to_worldmap(),
            MapContext::Minimap => taimi_meta::ui::MapCalibration::cast_compass_to_worldmap(
                machine.map.calibration.fake_to_compass(),
            ),
        }
        .then(machine.map.worldmap_to_map_for(ctx));
        let map_bounds_fake = machine.map.calibration.bounds_for(ctx);
        let map_bounds_global = Box2::new(
            fake_to_map.map(map_bounds_fake.min()),
            fake_to_map.map(map_bounds_fake.max()),
        );
        let map_bounds_local = Box2::new(
            map_to_local.map(map_bounds_global.min),
            map_to_local.map(map_bounds_global.max),
        );
        let map_bounds_local = Box2::new(
            map_bounds_local.min.min(map_bounds_local.max),
            map_bounds_local.min.max(map_bounds_local.max),
        );
        let (height_offset_below, height_offset_above) = match ctx {
            MapContext::Global => (Self::HEIGHT_OFFSET_BELOW, Self::HEIGHT_OFFSET_ABOVE),
            MapContext::Minimap => (Self::HEIGHT_OFFSET_BELOW_MINI, Self::HEIGHT_OFFSET_ABOVE_MINI),
        };
        let (player_pos_local, ..) = machine.get_player();
        Box3::<LocalSpace>::new(
            Point3::new(
                map_bounds_local.min.x,
                player_pos_local.y + height_offset_below,
                map_bounds_local.min.y,
            ),
            Point3::new(
                map_bounds_local.max.x,
                player_pos_local.y + height_offset_above,
                map_bounds_local.max.y,
            ),
        )
    }

    pub fn update_map(&mut self, machine: &mut RenderMachine, bounds: Box3<LocalSpace>, fwoom: bool) {
        let open = machine.map_open();
        let ctx = machine.is_map_visible().unwrap_or(open.into());

        let map_to_local = machine.map.calibration.map_to_local();
        let centre_local = map_to_local.map(machine.map.centre_for(ctx));

        let map_bounds_fake = machine.map.calibration.bounds_for(ctx);
        let bounds_screen: glamour::Rect<ScreenSpace> = machine.map.calibration.map(map_bounds_fake);

        let (_pos, cam_front, _up) = machine.get_camera(Default::default());
        let anim_progress = open.progress().and_then(|p| fwoom.then_some(p)).map(|p| {
            let dir = cam_front
                .to_raw()
                .cross(cam_front.z.signum() * -Vec3::Z)
                .normalize();
            //let front = Vec3::new(-front.x, front.z, -front.y);
            let progress = match open.is_open() {
                true => p,
                false => 1.0 - p,
            };
            (
                progress,
                p,
                Vec3::new(dir.x, cam_front.z.signum() * -dir.y, dir.z),
            )
        });
        let anim_rot = if let Some((progress, _p, front)) = anim_progress {
            let amt = progress * progress;
            let target = Quat::look_to_lh(front, /*pdata.camera_up().to_raw()*/ -Vec3::Z)
                * Quat::from_rotation_z(-0.95 * core::f32::consts::FRAC_PI_2)
                * if cam_front.z.is_sign_negative() && cam_front.x.is_sign_negative() {
                    //Quat::from_rotation_z(core::f32::consts::PI * 0.25)
                    Quat::from_rotation_z(core::f32::consts::PI * -0.49)
                    //Quat::from_rotation_z(core::f32::consts::PI * 0.98 * cam_front.x.signum())
                } else {
                    Quat::IDENTITY
                };
            target.normalize().lerp(Quat::IDENTITY, amt)
        } else {
            Quat::IDENTITY
        };

        let size = bounds.size();
        let (left, right) = (bounds.min.x, bounds.max.x);
        let (bottom, top) = (bounds.min.z, bounds.max.z);
        let (near, far) = (0.001f32, 1000.0f32);
        let mid = Vec3::new(centre_local.x, bounds.center().y, centre_local.y);
        let (map_rotation, counter_rot) = match machine.map.rotation() {
            Some(amt) => (
                Quat::from_rotation_y(amt.to_radians()),
                Quat::from_rotation_y(-amt.to_radians()),
            ),
            None => (Quat::IDENTITY, Quat::IDENTITY),
        };
        let trans = Vec3::new(-mid.x, -mid.y, -mid.z);
        self.constant_buffer_mapv_data.model = Mat4::from_quat(counter_rot);
        self.constant_buffer_mapv_data.world = Mat4::from_quat(map_rotation)
            * Mat4::from_scale_rotation_translation(Vec3::ONE, Quat::IDENTITY, trans);
        let screen_mid = bounds_screen.center();
        let screen_sz = bounds_screen.size;
        let screen = machine.map.calibration.display_size;
        let scl = Vec3::new(
            screen_sz.width / screen.width,
            screen_sz.height / screen.height,
            1.0,
        );
        let window_trans = Vec2::new(-mid.x, -mid.z);
        let window_trans = Vec2::new(
            window_trans.x - (screen_mid.x / screen.width - 0.5) * size.width,
            window_trans.y + (screen_mid.y / screen.height - 0.5) * size.depth,
        );
        self.constant_buffer_mapv_data.view = Mat4::orthographic_lh(
            left + window_trans.x,
            right + window_trans.x,
            bottom + window_trans.y,
            top + window_trans.y,
            near,
            far,
        ) * Mat4::from_scale_rotation_translation(
            if let Some((_p, progress, _)) = anim_progress {
                let (amt, prog) = match open.is_open() {
                    true => (
                        24.0 * taimi_meta::coords::MapLocalScale::METRES_PER_INCH
                            / machine.map.scale_for(ctx),
                        progress,
                    ),
                    false => (1.0, _p),
                };
                let amt = match open.is_open() {
                    true => 0.8 + ((1.0 - prog * prog * prog) * amt),
                    false => 1.0,
                };
                scl * amt
            } else {
                scl
            },
            anim_rot,
            Vec3::ZERO,
        );
    }

    pub fn update_map_cb(&self, device_context: &Dx11Context) {
        self.constant_buffer_mapv
            .update_singleton(device_context, &self.constant_buffer_mapv_data);
        self.constant_buffer_mapp
            .update_singleton(device_context, &self.constant_buffer_mapp_data);
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
        _poi_expansion: PoiScale::DEFAULT,
        trail_expansion: TrailScale::DEFAULT,
        trail_texture: TrailTextureMap::DEFAULT,
    };
}

unsafe impl D3dBufferData for PerspectiveData {}

impl PixelData {
    pub const INITIAL: Self = Self {
        distance_param: Vec4::new(
            Self::OVERLAP_DEFAULT,
            Self::INTENSITY_DEFAULT,
            Self::FEATHER_SCALE_NONE,
            Self::FEATHER_SCALE_NONE,
        ),
        viewport_param: Vec4::new(Self::VIEWPORT_NONE, Self::VIEWPORT_NONE, 0.0, 0.0),
    };

    pub const OVERLAP_THRESHOLD_OFF: f32 = 0.01;
    pub const OVERLAP_THRESHOLD_DEFAULT: f32 = SpaceSettings::DEFAULT_PLAYER_OVERLAP_THRESHOLD;
    pub const OVERLAP_DEFAULT: f32 = Self::OVERLAP_THRESHOLD_DEFAULT;

    pub const INTENSITY_OFF: f32 = 1_000_000.0;
    pub const INTENSITY_DEFAULT: f32 = SpaceSettings::DEFAULT_DISTANCE_FADE_INTENSITY;
    pub const FEATHER_SCALE_NONE: f32 = 1.0e8;
    pub const VIEWPORT_NONE: f32 = 1.0 / 10000.0;

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

    pub fn set_viewport_size(&mut self, size: Option<Vec2>) {
        let size = size
            .map(|size| size.map(|dim| 2.0 / dim))
            .unwrap_or(Vec2::splat(Self::VIEWPORT_NONE));
        self.viewport_param.x = size.x;
        self.viewport_param.y = size.y;
    }

    pub fn set_feather_scale(&mut self, scale: Option<Vec2>) {
        let scale = scale.unwrap_or(Vec2::splat(Self::FEATHER_SCALE_NONE));
        self.distance_param.z = scale.x;
        self.distance_param.w = scale.y;
    }
}

unsafe impl D3dBufferData for PixelData {}

#[repr(C, align(16))]
#[derive(Debug, Copy, Clone)]
pub struct MapDataV {
    pub model: Mat4,
    pub world: Mat4,
    pub view: Mat4,
    pub trail_expansion: TrailScale,
    pub poi_expansion: PoiScale,
    pub trail_texture: TrailTextureMap,
}

impl MapDataV {
    pub const INITIAL: Self = Self {
        model: Mat4::IDENTITY,
        world: Mat4::IDENTITY,
        view: Mat4::IDENTITY,
        poi_expansion: PoiScale::DEFAULT,
        trail_expansion: TrailScale::DEFAULT,
        trail_texture: TrailTextureMap::DEFAULT,
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
    pub const INITIAL: Self = Self { colour: Vec4::ONE };
}

unsafe impl D3dBufferData for MapDataP {}
