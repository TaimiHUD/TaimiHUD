use {
    super::{cb_as_cb_list, PerspectiveInputData},
    anyhow::{anyhow, Context},
    crate::space::{max_depth, min_depth},
    glam::{Mat4, Vec3, Vec4},
    windows::Win32::Graphics::Direct3D11::{
        ID3D11Buffer, ID3D11Device, ID3D11DeviceContext, D3D11_BIND_CONSTANT_BUFFER,
        D3D11_BUFFER_DESC, D3D11_SUBRESOURCE_DATA, D3D11_USAGE_DEFAULT,
    },
};

#[repr(C)]
#[derive(Debug)]
pub struct PerspectiveData {
    view: Mat4,
    projection: Mat4,
    /// Billboard transform for current camera.
    billboard: Mat4,
    player: Vec4,
}

#[repr(C)]
#[derive(Debug)]
pub struct PixelData {
    distance_param: Vec4,
}

pub struct PerspectiveHandler {
    constant_buffer: ID3D11Buffer,
    constant_buffer_data: PerspectiveData,
    constant_buffer_pixel: ID3D11Buffer,
    pub constant_buffer_pixel_data: PixelData,
    aspect_ratio: f32,
    pub alpha: f32,
    up: Vec3,
    near: f32,
    far: f32,
    last_display_size: [f32; 2],
}

impl PerspectiveHandler {
    pub fn setup(device: &ID3D11Device, display_size: &[f32; 2]) -> anyhow::Result<Self> {
        let aspect_ratio = display_size[0] / display_size[1];
        let constant_buffer_data = PerspectiveData::INITIAL;
        let constant_buffer = Self::create_constant_buffer(device, &constant_buffer_data)?;
        let constant_buffer_pixel_data = PixelData::INITIAL;
        let constant_buffer_pixel = Self::create_constant_buffer(device, &constant_buffer_pixel_data)?;
        Ok(Self {
            up: Vec3::new(0.0, 1.0, 0.0),
            aspect_ratio,
            alpha: 1.0,
            last_display_size: *display_size,
            constant_buffer,
            constant_buffer_data,
            constant_buffer_pixel,
            constant_buffer_pixel_data,
            near: min_depth(),
            far: max_depth(),
        })
    }

    pub fn update_perspective(&mut self, display_size: &[f32; 2]) {
        let data = PerspectiveInputData::get();
            if *display_size != self.last_display_size {
                self.aspect_ratio = display_size[0] / display_size[1];
                self.last_display_size = *display_size;
            }

            self.constant_buffer_data.view = Mat4::look_to_lh(data.pos, data.front, self.up);
            self.near = min_depth();
            self.far = max_depth();
            self.constant_buffer_data.projection =
                Mat4::perspective_lh(data.fov, self.aspect_ratio, self.near, self.far);
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
        };
    }

    fn create_constant_buffer<D>(device: &ID3D11Device, initial: &D) -> anyhow::Result<ID3D11Buffer> {
        let constant_buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<D>().next_multiple_of(16) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let constant_subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: initial as *const D as *const _,
            .. D3D11_SUBRESOURCE_DATA::default()
        };

        let mut constant_buffer_ptr: Option<ID3D11Buffer> = None;
        let constant_buffer = unsafe {
            device.CreateBuffer(
                &constant_buffer_desc,
                Some(&constant_subresource_data),
                Some(&mut constant_buffer_ptr),
            )
        }.context("constant buffer creation failed")
        .and_then(|()| constant_buffer_ptr.ok_or_else(|| anyhow!("no constant buffer")))?;

        Ok(constant_buffer)
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

    pub fn update_cb(&self, device_context: &ID3D11DeviceContext) {
        unsafe {
            device_context.UpdateSubresource(
                &self.constant_buffer,
                0,
                None,
                &self.constant_buffer_data as *const PerspectiveData as *const _,
                0,
                0,
            );
            device_context.UpdateSubresource(
                &self.constant_buffer_pixel,
                0,
                None,
                &self.constant_buffer_pixel_data as *const PixelData as *const _,
                0,
                0,
            );
        }
    }
    pub fn set_cb(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        unsafe {
            device_context.VSSetConstantBuffers(slot, Some(cb_as_cb_list(&self.constant_buffer)));
            device_context.PSSetConstantBuffers(slot, Some(cb_as_cb_list(&self.constant_buffer_pixel)));
        }
    }
    pub fn set(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        self.set_cb(device_context, slot);
        self.update_cb(device_context);
    }
}

impl PerspectiveData {
    pub const INITIAL: Self = Self {
        view: Mat4::IDENTITY,
        projection: Mat4::IDENTITY,
        player: Vec4::ZERO,
        billboard: Mat4::IDENTITY,
    };
}

impl PixelData {
    pub const INITIAL: Self = Self {
        distance_param: Vec4::new(Self::OVERLAP_DEFAULT, Self::INTENSITY_DEFAULT, 0.0, 0.0),
    };

    pub const OVERLAP_THRESHOLD_OFF: f32 = 0.01;
    pub const OVERLAP_THRESHOLD_DEFAULT: f32 = 38.0;
    pub const OVERLAP_DEFAULT: f32 = Self::OVERLAP_THRESHOLD_DEFAULT;

    pub const INTENSITY_OFF: f32 = 1_000_000.0;
    pub const INTENSITY_DEFAULT: f32 = 88.0;

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
