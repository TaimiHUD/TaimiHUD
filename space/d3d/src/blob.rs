use {
    crate::dx11::prelude::*,
    std::{ffi, mem, ptr, slice},
};
pub use crate::d3d::Fxc::D3DCreateBlob;

#[derive(Debug, Clone)]
#[repr(transparent)]
pub struct Blob {
    pub blob: ID3DBlob,
}

impl Blob {
    pub fn with_blob(blob: ID3DBlob) -> Self {
        Self {
            blob,
        }
    }

    pub fn with_blob_ref(blob: &ID3DBlob) -> &Self {
        unsafe {
            mem::transmute(blob)
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        unsafe {
            slice::from_raw_parts(self.as_ptr() as *const u8, self.size() as usize)
        }
    }

    pub fn as_ptr(&self) -> *const ffi::c_void {
        unsafe {
            self.blob.GetBufferPointer()
        }
    }

    pub fn size(&self) -> usize {
        unsafe {
            self.blob.GetBufferSize()
        }
    }

    pub fn with_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        unsafe {
            D3DCreateBlob(bytes.len())
        }.map_err(anyhow::Error::from)
        .context("D3DCreateBlob")
        .map(|blob| {
            let blob = Self::with_blob(blob);
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), blob.as_ptr() as *mut ffi::c_void as *mut u8, bytes.len());
            }
            blob
        })
    }
}

impl AsRef<[u8]> for Blob {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl From<ID3DBlob> for Blob {
    fn from(blob: ID3DBlob) -> Self {
        Self::with_blob(blob)
    }
}
impl From<Blob> for ID3DBlob {
    fn from(tex2: Blob) -> Self {
        tex2.blob
    }
}
