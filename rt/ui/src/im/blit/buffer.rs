use crate::im::prelude::*;

pub trait ImBufferBlobInfo {
    fn blob_count(&self) -> usize;
    #[inline]
    fn elem_size(&self) -> usize {
        self.elem_stride()
    }
    fn elem_stride(&self) -> usize;
    #[cfg(todo)]
    fn elem_format_untyped(&self) -> usize;
}
pub trait ImBufferBlob: ImBufferBlobInfo {
    fn blob_ptr(&self) -> NonNull<()>;

    unsafe fn blob_read_unchecked(&self, dest: NonNull<()>, offset: usize, count: usize) {
        let stride = self.elem_stride();
        let src = self.blob_ptr().byte_add(offset * stride);
        let size = count * stride;
        ptr::copy_nonoverlapping(src.cast::<u8>().as_ptr(), dest.cast().as_ptr(), size)
    }
    unsafe fn elem_read_unchecked(&self, dest: NonNull<()>, offset: usize, count: usize) {
        let (size, stride) = (self.elem_size(), self.elem_stride());
        if size == stride {
            return self.blob_read_unchecked(dest, offset, count);
        }
        let mut src = self.blob_ptr().cast::<u8>();
        let mut dest = dest.cast::<u8>();
        let dest_end = dest.byte_add(size * count);
        while dest < dest_end {
            ptr::copy_nonoverlapping(src.as_ptr(), dest.as_ptr(), size);
            src = src.add(stride);
            dest = dest.add(size);
        }
    }
    unsafe fn elem_read(&self, dest: NonNull<()>, src: ops::Range<usize>) -> usize {
        let count = match src.end.min(self.blob_count()) {
            #[cfg(todo)]
            end => end.saturating_sub(src.start),
            #[cfg_attr(todo, debug_assertions)]
            end => end - src.start,
        };
        self.elem_read_unchecked(dest, src.start, count);
        count
    }
}
pub trait ImBufferBlobMut: ImBufferBlob {
    fn blob_capacity(&self) -> usize {
        self.blob_count()
    }
    unsafe fn blob_set_count(&mut self, new_len: usize);

    unsafe fn blob_write_unchecked(&self, src: NonNull<()>, offset: usize, count: usize) {
        let stride = self.elem_stride();
        let dest = self.blob_ptr().byte_add(offset * stride);
        let size = count * stride;
        ptr::copy_nonoverlapping(src.cast::<u8>().as_ptr(), dest.cast().as_ptr(), size)
    }
    unsafe fn elem_write_unchecked(&self, src: NonNull<()>, offset: usize, count: usize) {
        let (size, stride) = (self.elem_size(), self.elem_stride());
        if size == stride {
            return self.blob_write_unchecked(src, offset, count);
        }
        let mut dest = self.blob_ptr().cast::<u8>();
        let mut src = dest.cast::<u8>();
        let src_end = src.byte_add(size * count);
        while src < src_end {
            ptr::copy_nonoverlapping(src.as_ptr(), dest.as_ptr(), size);
            src = src.add(size);
            dest = dest.add(stride);
        }
    }
    unsafe fn elem_write(&self, src: NonNull<()>, dest: ops::Range<usize>) -> usize {
        let count = match dest.end.min(self.blob_capacity()) {
            #[cfg(todo)]
            end => end.saturating_sub(dest.start),
            #[cfg_attr(todo, debug_assertions)]
            end => end - dest.start,
        };
        self.elem_write_unchecked(src, dest.start, count);
        count
    }
}
pub trait ImBufferBlobGrow: ImBufferBlobMut {
    fn blob_reserve_to(&mut self, new_cap: usize);
    #[cfg(todo)]
    fn blob_grow(&mut self);
    unsafe fn blob_set_capacity_unchecked(&mut self, new_cap: usize);

    #[cfg(todo)]
    unsafe fn blob_push_elem(&mut self, src: NonNull<()>)
    where
        Self: Sized,
    {
        let start = self.blob_count();
        self.blob_grow_one();
        self.elem_write_unchecked(src, start, 1);
        self.blob_set_len(start + 1);
    }
}

pub trait ImBufferBlobExt: ImBufferBlob {
    #[inline]
    unsafe fn blob_elems_of_mut_unchecked<T>(&mut self) -> &mut [T] {
        slice::from_raw_parts_mut(self.blob_ptr().cast().as_ptr(), self.blob_count())
    }
    #[inline]
    unsafe fn blob_elems_of_unchecked<T>(&self) -> &[T] {
        slice::from_raw_parts(self.blob_ptr().cast().as_ptr(), self.blob_count())
    }
    /// TODO: `T: Pod`
    fn blob_elems_of<T>(&self) -> &[T]
    where
        T: Copy,
    {
        if mem::size_of::<T>() != self.elem_stride() {
            log::error!("blob elem mismatch");
            return &[]
        }
        unsafe { self.blob_elems_of_unchecked::<T>() }
    }
}
impl<T> ImBufferBlobExt for T where T: ?Sized + ImBufferBlob {}
