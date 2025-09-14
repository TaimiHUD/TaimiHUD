use {
    super::{
        super::{
            dx11::{prelude::*, InstanceBuffer, InstanceBufferData, VertexBuffer},
            resources::{Model, ObjMaterial, ShaderPair},
        },
        PrimitiveTopology,
    },
    glam::Mat4,
};

pub struct ObjectRenderBacking {
    pub metadata: ObjectRenderMetadata,
    pub instance_buffer: InstanceBuffer,
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
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
        data: &[InstanceBufferData],
    ) -> anyhow::Result<()> {
        // TODO: extract inner error somehow, arc's suggestion didn't work o:
        self.instance_buffer.update(device, device_context, data)?;
        Ok(())
    }

    pub fn set_shaders(&self, device_context: &ID3D11DeviceContext) {
        self.shaders.set(device_context);
    }

    pub fn set_texture(&self, slot: u32, device_context: &ID3D11DeviceContext) {
        if let Some(diffuse) = &self.metadata.material.diffuse {
            diffuse.texture.set(device_context, slot);
        }
    }

    pub fn set_buffers(&self, slot: u32, device_context: &ID3D11DeviceContext) {
        VertexBuffer::set_all(device_context, slot, &[
            &self.vertex_buffer,
            &self.instance_buffer,
        ])
    }

    pub fn draw(&self, start: u32, device_context: &ID3D11DeviceContext) {
        let instances = self.instance_buffer.get_count();
        let total = self.vertex_buffer.count + instances as u32;
        self.metadata.topology.set(device_context);
        unsafe {
            device_context.DrawInstanced(total, instances as u32, start, 0)
        }
    }
    pub fn set_and_draw(
        &self,
        slot: u32,
        device: &ID3D11Device,
        device_context: &ID3D11DeviceContext,
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

impl D3d11ContextBindableSlot for ObjectRenderBacking {
    fn set(&self, device_context: &ID3D11DeviceContext, slot: u32) {
        self.set_shaders(device_context);
        self.set_texture(slot, device_context);
        self.set_buffers(slot, device_context);
    }
}
