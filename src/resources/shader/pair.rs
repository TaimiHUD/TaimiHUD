use {
    super::{PixelShader, VertexShader},
    std::sync::Arc,
    windows::Win32::Graphics::Direct3D11::ID3D11DeviceContext,
};

pub struct ShaderPair(pub Arc<VertexShader>, pub Arc<PixelShader>);

impl ShaderPair {
    pub fn set(&self, device_context: &ID3D11DeviceContext) {
        self.0.set(device_context);
        self.1.set(device_context);
    }
}
