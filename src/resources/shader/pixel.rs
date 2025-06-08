use {
    super::ShaderDescription,
    anyhow::anyhow,
    core::ffi::c_char,
    std::{ffi::CStr, path::Path, slice::from_raw_parts},
    windows::Win32::Graphics::{
        Direct3D::{
            Fxc::{D3DCompileFromFile, D3DCOMPILE_DEBUG},
            ID3DBlob,
        },
        Direct3D11::{ID3D11Device, ID3D11DeviceContext, ID3D11PixelShader},
    },
    windows_strings::PCSTR,
};

#[derive(PartialEq)]
pub struct PixelShader {
    shader: ID3D11PixelShader,
}
impl PixelShader {
    pub fn create(
        shader_folder: &Path,
        device: &ID3D11Device,
        desc: &ShaderDescription,
    ) -> anyhow::Result<Self> {
        let blob = Self::compile(shader_folder, desc)?;

        let blob_bytes =
            unsafe { from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize()) };
        log::info!(
            "Creating {:?} of {} shader, entrypoint {:?}",
            &desc.path,
            &desc.kind,
            &desc.entrypoint
        );
        let mut shader_ptr: Option<ID3D11PixelShader> = None;
        let shader = unsafe { device.CreatePixelShader(blob_bytes, None, Some(&mut shader_ptr)) }
            .map_err(anyhow::Error::from)
            .and_then(|()| shader_ptr.ok_or_else(|| anyhow!("no pixel shader")))?;
        Ok(PixelShader { shader })
    }
    pub fn set(&self, context: &ID3D11DeviceContext) {
        unsafe {
            context.PSSetShader(&self.shader, None);
        }
    }
    pub fn compile(shader_folder: &Path, desc: &ShaderDescription) -> anyhow::Result<ID3DBlob> {
        let (filename, target, entrypoint_cstring) = desc.get(shader_folder)?;
        log::info!(
            "Beginning compile from {:?} of {} shader, entrypoint {:?}",
            &desc.path,
            &desc.kind,
            entrypoint_cstring
        );
        let mut blob_ptr: Option<ID3DBlob> = None;
        let mut error_blob: Option<ID3DBlob> = None;
        let entrypoint = PCSTR::from_raw(entrypoint_cstring.as_ptr() as *const u8);
        let blob = unsafe {
            D3DCompileFromFile(
                &filename,
                None,
                None,
                entrypoint,
                target,
                D3DCOMPILE_DEBUG,
                0,
                &mut blob_ptr,
                Some(&mut error_blob),
            )
        }
        .map_err(anyhow::Error::from)
        .and_then(|()| blob_ptr.ok_or_else(|| anyhow!("no {} shader", &desc.kind)))
        .map_err(|e| match error_blob {
            Some(ref error_blob) => {
                let msg = unsafe { CStr::from_ptr(error_blob.GetBufferPointer() as *const c_char) };
                let res = anyhow!("{}: {}", e, msg.to_string_lossy());
                let _ = error_blob;
                res
            }
            None => e,
        })?;

        log::info!(
            "Compile successful from {:?} of {} shader, entrypoint {:?}",
            &desc.path,
            &desc.kind,
            entrypoint
        );
        Ok(blob)
    }
}
