use {
    crate::win32::core::{Error as WinError, Result as WinResult},
    anyhow::Context,
    arcffi::cstr::{CStrPtr, CStrRef},
    std::{marker::PhantomData, mem, ptr::NonNull, slice},
    windows::Win32::System::Memory::{FILE_MAP_READ, FILE_MAP_WRITE},
};
pub use {
    crate::win32::Win32::Foundation::{CloseHandle, HANDLE},
    windows::Win32::System::Memory::{
        MapViewOfFile,
        OpenFileMappingA,
        UnmapViewOfFile,
        FILE_MAP,
        MEMORY_MAPPED_VIEW_ADDRESS,
    },
};

#[derive(Debug, Default)]
pub struct FileMapping {
    pub handle: HANDLE,
    pub access: FILE_MAP,
}

impl FileMapping {
    pub const ACCESS_READ: FILE_MAP = FILE_MAP_READ;
    pub const ACCESS_WRITE: FILE_MAP = FILE_MAP_WRITE;

    pub fn open_named<'n, N: AsRef<CStrRef>>(name: N, access: FILE_MAP) -> anyhow::Result<Self> {
        let name = name.as_ref();
        let res = Self::open_file_mapping(name.as_c_ptr(), access, false);
        res.with_context(move || format!("OpenFileMappingA({name}, {})", access.0))
            .map(move |handle| Self { handle, access })
    }

    pub fn open_file_mapping(
        name: CStrPtr<'_>,
        access: FILE_MAP,
        inherit_handle: bool,
    ) -> WinResult<HANDLE> {
        unsafe {
            //let name = name.immortal();
            OpenFileMappingA(access.0, inherit_handle, name)
        }
        .map(Into::into)
        .map_err(Into::into)
    }

    pub fn map(&self, offset: usize, len: usize) -> anyhow::Result<FileMapped<'_>> {
        let offset_hi = ((offset as u64) << 32) as u32;
        let offset_lo = offset as u32;
        let view = unsafe { MapViewOfFile(self.handle.into(), self.access, offset_hi, offset_lo, len) };
        match view {
            view if view.Value.is_null() => Err(WinError::from_win32()),
            view => Ok(unsafe { FileMapped::from_view(view, self.access, len) }),
        }
        .context("MapViewOfFile")
    }
    pub fn map_write(&mut self, offset: usize, len: usize) -> anyhow::Result<FileMapped<'_>> {
        self.map(offset, len)
    }

    pub fn leak_map(self, offset: usize, len: usize) -> anyhow::Result<FileMapped<'static>> {
        let map = self.map(offset, len)?;
        Ok(unsafe {
            let map = map.leak();
            mem::forget(self);
            map
        })
    }

    pub fn close(&mut self) -> anyhow::Result<()> {
        if self.handle.is_invalid() {
            return Ok(())
        }
        let handle = mem::take(&mut self.handle);
        unsafe { CloseHandle(handle) }.context("CloseHandle")
    }
}
impl Drop for FileMapping {
    fn drop(&mut self) {
        let cleanup = self.close();
        if let Err(e) = cleanup {
            log::warn!("{e:#}");
        }
    }
}
pub struct FileMapped<'a> {
    pub view: MEMORY_MAPPED_VIEW_ADDRESS,
    pub size: usize,
    pub access: FILE_MAP,
    pub handle: PhantomData<&'a FileMapping>,
}
impl<'a> FileMapped<'a> {
    pub unsafe fn from_view(view: MEMORY_MAPPED_VIEW_ADDRESS, access: FILE_MAP, size: usize) -> Self {
        Self {
            view,
            size,
            access,
            handle: PhantomData,
        }
    }

    pub fn data(&self) -> Option<&[u8]> {
        if self.access.0 & FileMapping::ACCESS_READ.0 == 0 {
            return None
        }
        let ptr = NonNull::new(self.view.Value).map(NonNull::cast::<u8>);
        ptr.map(|ptr| unsafe { slice::from_raw_parts(ptr.as_ptr() as *const u8, self.size) })
    }
    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        if self.access.0 & FileMapping::ACCESS_WRITE.0 == 0 {
            return None
        }
        let ptr = NonNull::new(self.view.Value).map(NonNull::cast::<u8>);
        ptr.map(|ptr| unsafe { slice::from_raw_parts_mut(ptr.as_ptr(), self.size) })
    }

    pub fn unmap(&mut self) -> anyhow::Result<()> {
        if self.view.Value.is_null() {
            return Ok(())
        }
        let view = mem::take(&mut self.view);
        unsafe { UnmapViewOfFile(view) }.context("UnmapViewOfFile")
    }

    pub unsafe fn as_ptr<T>(&self) -> anyhow::Result<&T> {
        let size = mem::size_of::<T>();
        self.data()
            .and_then(|d| match d.len() >= size {
                true => Some(d),
                false => None,
            })
            .context("map ptr")
            .map(|data| unsafe { &*(data.as_ptr() as *const T) })
    }
    pub unsafe fn leak_ptr<T: 'a>(self) -> anyhow::Result<&'a T> {
        let size = mem::size_of::<T>();
        mem::ManuallyDrop::new(self)
            .data()
            .and_then(|d| match d.len() >= size {
                true => Some(d),
                false => None,
            })
            .context("map ptr")
            .map(|data| unsafe { &*(data.as_ptr() as *const T) })
    }

    pub unsafe fn leak(self) -> FileMapped<'static> {
        let map = mem::ManuallyDrop::new(self);
        FileMapped {
            view: map.view,
            size: map.size,
            access: map.access,
            handle: PhantomData,
        }
    }
}
impl Drop for FileMapped<'_> {
    fn drop(&mut self) {
        let cleanup = self.unmap();
        if let Err(e) = cleanup {
            log::warn!("{e:#}");
        }
    }
}
