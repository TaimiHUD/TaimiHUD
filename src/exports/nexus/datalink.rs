pub use windows::Win32::Foundation::HANDLE;
use {
    crate::exports::runtime as rt,
    anyhow::Context,
    std::{
        ffi::{CStr, CString},
        fmt,
        marker::PhantomData,
        mem,
        ptr::{self, NonNull},
        slice,
    },
    windows::{
        core::{Error as WinError, PCSTR},
        Win32::{
            Foundation::CloseHandle,
            System::Memory::{
                MapViewOfFile,
                OpenFileMappingA,
                UnmapViewOfFile,
                FILE_MAP,
                FILE_MAP_READ,
                FILE_MAP_WRITE,
                MEMORY_MAPPED_VIEW_ADDRESS,
            },
        },
    },
};

#[derive(Debug, Default)]
pub struct LinkHandle {
    pub handle: HANDLE,
    pub access: FILE_MAP,
}

impl LinkHandle {
    pub const ACCESS_READ: FILE_MAP = FILE_MAP_READ;
    pub const ACCESS_WRITE: FILE_MAP = FILE_MAP_WRITE;

    pub fn open_shared(object_name: &CStr, access: FILE_MAP) -> anyhow::Result<Self> {
        unsafe { OpenFileMappingA(access.0, false, PCSTR(object_name.as_ptr() as *const _)) }
            .context("OpenFileMappingA")
            .map(|handle| Self { handle, access })
    }

    pub fn map(&self, offset: usize, len: usize) -> anyhow::Result<LinkMap<'_>> {
        let offset_hi = ((offset as u64) << 32) as u32;
        let offset_lo = offset as u32;
        let view = unsafe { MapViewOfFile(self.handle, self.access, offset_hi, offset_lo, len) };
        match view {
            view if view.Value.is_null() => Err(WinError::from_win32()),
            view => Ok(unsafe { LinkMap::from_view(view, self.access, len) }),
        }
        .context("MapViewOfFile")
    }
    pub fn map_write(&mut self, offset: usize, len: usize) -> anyhow::Result<LinkMap<'_>> {
        self.map(offset, len)
    }

    pub fn leak_map(self, offset: usize, len: usize) -> anyhow::Result<LinkMap<'static>> {
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
        let handle = mem::replace(&mut self.handle, HANDLE::default());
        unsafe { CloseHandle(handle) }.context("CloseHandle")
    }

    pub fn open_data_link(name: &dyn fmt::Display, access: FILE_MAP) -> anyhow::Result<Self> {
        let object_name = unsafe {
            //let process_id = windows::Win32::System::Threading::GetCurrentProcessId();
            let process_id = std::process::id();
            let name = format!("{name}_{process_id}");
            CString::from_vec_unchecked(name.into())
        };
        Self::open_shared(&object_name, access)
            .with_context(move || unsafe { String::from_utf8_unchecked(object_name.into_bytes()) })
    }

    pub fn check_for_data(name: &str) -> bool {
        match Self::open_data_link(&name, FILE_MAP_READ) {
            Ok(mut handle) => {
                let res = handle
                    .close()
                    .with_context(|| format!("cleanup after checking for {name} failed"));
                rt::log::debug_ok(res);
                true
            },
            Err(_e) => {
                // TODO: does it matter what error code we expect, ERROR_OBJECT_NOT_FOUND?
                #[cfg(debug_assertions)]
                {
                    log::debug!("NexusLink({name}) unavailable: {_e}");
                }
                false
            },
        }
    }
}
impl Drop for LinkHandle {
    fn drop(&mut self) {
        let cleanup = self.close();
        if let Err(e) = cleanup {
            log::warn!("{e:#}");
        }
    }
}
pub struct LinkMap<'a> {
    pub view: MEMORY_MAPPED_VIEW_ADDRESS,
    pub size: usize,
    pub access: FILE_MAP,
    pub handle: PhantomData<&'a LinkHandle>,
}
impl<'a> LinkMap<'a> {
    pub unsafe fn from_view(view: MEMORY_MAPPED_VIEW_ADDRESS, access: FILE_MAP, size: usize) -> Self {
        Self {
            view,
            size,
            access,
            handle: PhantomData,
        }
    }

    pub fn data(&self) -> Option<&[u8]> {
        if self.access.0 & FILE_MAP_READ.0 == 0 {
            return None
        }
        let ptr = NonNull::new(self.view.Value).map(NonNull::cast::<u8>);
        ptr.map(|ptr| unsafe { slice::from_raw_parts(ptr.as_ptr() as *const u8, self.size) })
    }
    pub fn data_mut(&mut self) -> Option<&mut [u8]> {
        if self.access.0 & FILE_MAP_WRITE.0 == 0 {
            return None
        }
        let ptr = NonNull::new(self.view.Value).map(NonNull::cast::<u8>);
        ptr.map(|ptr| unsafe { slice::from_raw_parts_mut(ptr.as_ptr(), self.size) })
    }

    pub fn unmap(&mut self) -> anyhow::Result<()> {
        if self.view.Value.is_null() {
            return Ok(())
        }
        let view = mem::replace(&mut self.view, Default::default());
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

    pub unsafe fn leak(self) -> LinkMap<'static> {
        let map = mem::ManuallyDrop::new(self);
        LinkMap {
            view: map.view,
            size: map.size,
            access: map.access,
            handle: PhantomData,
        }
    }
}
impl Drop for LinkMap<'_> {
    fn drop(&mut self) {
        let cleanup = self.unmap();
        if let Err(e) = cleanup {
            log::warn!("{e:#}");
        }
    }
}

pub fn check_for_nexus_link() -> bool {
    LinkHandle::check_for_data("DL_NEXUS_LINK")
}
