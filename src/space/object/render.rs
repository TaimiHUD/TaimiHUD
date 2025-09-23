use {
    crate::space::{
        dx11::InstanceBufferData,
        resources::{Model, ObjMaterial, ShaderPair},
    },
    glam::Mat4,
    std::cell::UnsafeCell,
    taimi_d3d::{
        dx11::{
            prelude::*,
            buffer::{
                Buffer,
                BufferOf,
                VertexBuffer,
            },
        },
        state::PrimitiveTopology,
        D3dContextBindableSlot,
    },
};

pub struct ObjectRenderBacking {
    pub metadata: ObjectRenderMetadata,
    pub instance_buffer: UnsafeCell<Buffer>,
    pub vertex_buffer: VertexBuffer,
    pub shaders: ShaderPair,
}

#[allow(unused)]
pub struct ObjectRenderMetadata {
    pub model: Model,
    pub material: ObjMaterial,
    pub model_matrix: Mat4,
    pub topology: PrimitiveTopology,
}

impl ObjectRenderBacking {
    pub fn update_instance_buffer(
        &self,
        device: &Dx11Device,
        device_context: &Dx11Context,
        data: &[InstanceBufferData],
    ) -> anyhow::Result<()> {
        unsafe {
            let count = self.instance_buffer().count();
            if count != data.len() {
                let buffer = self.instance_buffer.get();
                let buffer = &mut *buffer;
                buffer.replace(device, device_context, data)?;
            } else {
                self.instance_buffer().update_all_unchecked(device_context, data, 0);
            }
        }
        Ok(())
    }

    pub fn instance_buffer(&self) -> &BufferOf<InstanceBufferData> {
        unsafe {
            BufferOf::with_buffer_ref(&*self.instance_buffer.get())
        }
    }

    pub fn set_shaders(&self, device_context: &Dx11Context) {
        self.shaders.set(device_context);
    }

    pub fn set_texture(&self, slot: u32, device_context: &Dx11Context) {
        if let Some(diffuse) = &self.metadata.material.diffuse {
            diffuse.texture.set(device_context, slot);
        }
    }

    pub fn set_buffers(&self, slot: u32, device_context: &Dx11Context) {
        VertexBuffer::set_all(device_context, slot, &[
            &self.vertex_buffer,
            self.instance_buffer(),
        ])
    }

    pub fn draw(&self, start: u32, device_context: &Dx11Context) {
        let instances = self.instance_buffer().count_of::<InstanceBufferData>();
        let total = self.vertex_buffer.count + instances as u32;
        self.metadata.topology.set(device_context);
        unsafe {
            device_context.DrawInstanced(total, instances as u32, start, 0)
        }
    }
    pub fn set_and_draw(
        &self,
        slot: u32,
        device: &Dx11Device,
        device_context: &Dx11Context,
        data: &[InstanceBufferData],
    ) -> anyhow::Result<()> {
        self.update_instance_buffer(device, device_context, data)?;
        self.set_shaders(device_context);
        self.set_texture(slot, device_context);
        self.set_buffers(slot, device_context);
        self.draw(slot, device_context);
        Ok(())
    }
}

impl D3dContextBindableSlot<Dx11Context> for ObjectRenderBacking {
    fn set(&self, device_context: &Dx11Context, slot: u32) {
        self.set_shaders(device_context);
        self.set_texture(slot, device_context);
        self.set_buffers(slot, device_context);
    }
}

unsafe impl Send for ObjectRenderBacking {}
/// a lie...
unsafe impl Sync for ObjectRenderBacking {}
