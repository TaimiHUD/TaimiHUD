use {
    super::PerspectiveInputData,
    anyhow::{anyhow, Context},
    glam::{Mat4, Vec3},
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
}

pub struct PerspectiveHandler {
    constant_buffer: ID3D11Buffer,
    constant_buffer_data: PerspectiveData,
    aspect_ratio: f32,
    up: Vec3,
    near: f32,
    far: f32,
    last_display_size: [f32; 2],
}

impl PerspectiveHandler {
    pub fn setup(device: &ID3D11Device, display_size: &[f32; 2]) -> anyhow::Result<Self> {
        let aspect_ratio = display_size[0] / display_size[1];
        let constant_buffer_data = PerspectiveData {
            view: Mat4::IDENTITY,
            projection: Mat4::IDENTITY,
        };
        let constant_buffer = Self::create_constant_buffer(device, &constant_buffer_data)?;
        Ok(Self {
            up: Vec3::new(0.0, 1.0, 0.0),
            aspect_ratio,
            last_display_size: *display_size,
            constant_buffer,
            constant_buffer_data,
            near: 0.1,
            far: 1000.0,
        })
    }

    pub fn update_perspective(&mut self, display_size: &[f32; 2]) {
        if let Some(data) = PerspectiveInputData::read() {
            if *display_size != self.last_display_size {
                self.aspect_ratio = display_size[0] / display_size[1];
                self.last_display_size = *display_size;
            }

            self.constant_buffer_data.view = Mat4::look_to_lh(data.pos, data.front, self.up);
            self.constant_buffer_data.projection =
                Mat4::perspective_lh(data.fov, self.aspect_ratio, self.near, self.far);
        }
    }

    pub fn create_constant_buffer(device: &ID3D11Device, initial: &PerspectiveData) -> anyhow::Result<ID3D11Buffer> {
        let constant_buffer_desc = D3D11_BUFFER_DESC {
            ByteWidth: size_of::<PerspectiveData>().next_multiple_of(16) as u32,
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
            CPUAccessFlags: 0,
            MiscFlags: 0,
            StructureByteStride: 0,
        };

        let constant_subresource_data = D3D11_SUBRESOURCE_DATA {
            pSysMem: initial as *const PerspectiveData as *const _,
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

    fn update_cb(&mut self, device_context: &ID3D11DeviceContext) {
        unsafe {
            device_context.UpdateSubresource(
                &self.constant_buffer,
                0,
                None,
                &self.constant_buffer_data as *const _ as *const _,
                0,
                0,
            );
        }
    }
    fn set_cb(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        unsafe {
            device_context.VSSetConstantBuffers(slot, Some(&[Some(self.constant_buffer.clone())]));
        }
    }
    pub fn set(&mut self, device_context: &ID3D11DeviceContext, slot: u32) {
        self.set_cb(device_context, slot);
        self.update_cb(device_context);
    }
}
