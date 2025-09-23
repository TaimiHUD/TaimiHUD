use taimi_d3d::{
    dx11::{
        prelude::*,
        shader::{InputLayout, ShaderP, ShaderV},
    },
    D3dContextBindable,
};

pub struct ShaderPair(pub (ShaderV, InputLayout), pub Option<ShaderP>);

impl D3dContextBindable<Dx11Context> for ShaderPair {
    fn set(&self, device_context: &Dx11Context) {
        let (vertex, layout) = &self.0;
        layout.set(device_context);
        vertex.set(device_context);
        if let Some(pixel) = &self.1 {
            pixel.set(device_context);
        }
    }
}
