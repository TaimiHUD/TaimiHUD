use {
    crate::{
        render::machine::{frame_log, RenderMachine, RenderPosition},
        settings::goggles::GogglesEnables,
        space::DrawSpace,
    },
    super::{
        g2,
        tracking::{print_ferret, search_ferret, FerretPattern},
        GogglesShared,
    },
    core::{
        ffi::c_void,
        ptr,
        slice,
        num::NonZero,
    },
    std::collections::{BTreeMap, BTreeSet},
    glamour::{Point3, Vector3},
    glam::{Mat4, Vec3A},
    taimi_meta::{
        coords::{self, LocalSpace, GameSpace},
        map::MapProjectionDepth,
        packs::MapIndex,
    },
    taimi_d3d::{
        buffer::math::Mat43,
        dx11::{
            prelude::*,
            Buffer,
            Resource,
        },
    },
    windows::core::Interface,
};
use {
    bitvec::{array::BitArray, order::Lsb0},
    core::ops,
};

#[inline(always)]
pub(super) fn wants_update_camera(resource: *mut c_void) -> bool {
    let exp = GogglesShared::found_camera().map(|(b, ..)| b as *mut c_void).unwrap_or(ptr::null_mut());
    resource == exp
}
#[inline(always)]
pub(super) fn wants_update_perspective(resource: *mut c_void) -> bool {
    let exp = GogglesShared::found_perspective().map(|(b, ..)| b as *mut c_void).unwrap_or(ptr::null_mut());
    resource == exp
}
pub(super) unsafe fn update_camera(
    resource: &Resource,
    subresource: u32,
    data: *const u8,
    dest_offset: u32,
    _len: Option<NonZero<u32>>,
) {
    if dest_offset != 0 || subresource != 0 { return }
    #[cfg(todo = "unnecessary")]
    if !GogglesShared::wants_snatch_camera() { return }
    let Some((_expecting, offset, is_m43)) = GogglesShared::found_camera() else { return };
    let Some(offset) = (offset * 4).checked_sub(dest_offset as usize) else { return };
    let data = data as *const u32;
    let precheck = slice::from_raw_parts(data.byte_add(offset + CameraFerret::M43_LEN32 * 4), CameraFerret::M4_LEN32);
    let lost = match is_m43 {
        #[cfg(todo = "unused")]
        true => !PerspectiveFerret::matches_shape(precheck),
        _ => !CameraFerret::is_w(precheck)
    };
    let lost = match lost {
        #[cfg(todo)]
        false if !mounted => {
            // mounts can roll apparently bleh
            !CameraFerret::dir_up_matches(precheck)
        },
        lost => lost,
    };
    if lost {
        GogglesShared::clear_camera_found();
        return
    }
    let m = slice::from_raw_parts(data.byte_add(offset), CameraFerret::M43_LEN32);
    CameraFerret::report_matrix(m, None, resource.as_d3d().as_raw());
}
pub(super) unsafe fn update_perspective(
    resource: &Resource,
    subresource: u32,
    data: *const u8,
    dest_offset: u32,
    _len: Option<NonZero<u32>>,
) {
    if dest_offset != 0 || subresource != 0 { return }
    let wants_persp = GogglesShared::wants_snatch_perspective();
    #[cfg(todo)]
    if !wants_persp && !GogglesShared::wants_snatch_camera_smooth() { return }
    let Some((_expecting, offset)) = GogglesShared::found_perspective() else { return };
    let Some(offset) = (offset * 4).checked_sub(dest_offset as usize) else { return };
    let data = data as *const u32;
    let m = slice::from_raw_parts(data.byte_add(offset), CameraFerret::M4_LEN32);
    let lost = !PerspectiveFerret::matches_shape(m);
    if wants_persp {
        if lost {
            GogglesShared::clear_perspective_found();
        } else {
            PerspectiveFerret::report_matrix(m, None, resource.as_d3d().as_raw());
        }
    }
    if lost {
        return
    }
    if let Some(cam_smooth_offset) = offset.checked_sub(CameraFerret::M43_LEN32 * 4) {
        let mdata = slice::from_raw_parts(data.byte_add(cam_smooth_offset), CameraFerret::M43_LEN32);
        let m = SnatchMatrix::from(CameraFerret::m43_at_unchecked(mdata));
        if CameraFerret::matrix_matches_shape43(&m.data) {
            g2!(*&mut ferret.cam.snatch_camera_smooth = m);
        }
    }
}
#[inline(always)]
pub(super) fn wants_anything(
    _resource: &Dx11Resource,
) -> bool {
    g2!(*&ferret.cam.size_range.end) != CameraShared::SIZE_RANGE_EMPTY.end
}
#[inline(always)]
pub(super) fn wants_update_subresource_pre(
    resource: *mut c_void,
    (matched_cam, matched_persp): (bool, bool),
) -> bool {
    let blacklisted = !CameraSearch::with_mut_unchecked(|s| s.visit(resource));
    if blacklisted { return false }
    (!matched_cam && !GogglesShared::has_found_camera() && GogglesShared::wants_camera())
    || (!matched_persp && !GogglesShared::has_found_perspective() && GogglesShared::wants_perspective())
}
pub(super) fn wants_update_subresource(
    buffer_size: u32,
    region: ops::Range<u32>,
    (_matched_cam, _matched_persp): (bool, bool),
) -> bool {
    #[cfg(todo)]
    if frame_log!(::is_enabled()) { return true }
    buffer_size < 0x10000
        && region.len() >= CameraFerret::M4_LEN32 * 4
        && g2!(*&ferret.cam.size_range).contains(&(buffer_size as u16))
}
pub(super) fn update_subresource(
    resource: &Resource,
    _buffer: &Buffer,
    data: &[u32],
    _offset: u32,
    (matched_cam, matched_persp): (bool, bool),
) {
    if frame_log!(::is_enabled()) {
        #[cfg(todo)]
        if matched_cam || matched_persp {
            frame_log!(;"matched cam={matched_cam} persp={matched_persp}");
            print_ferret(data, (_offset / 4) as usize, data.len());
        } else {
            let found_persp = search_ferret(
                data,
                2,
                |data| data.len() >= PerspectiveFerret::M4_LEN32,
                |d| match PerspectiveFerret::matches_shape(d) {
                    true => Some(PerspectiveFerret::M4_LEN32),
                    false => None,
                },
                None,
            );
            let fcam = g2!(*&ferret.cam.camera);
            let found_cam = if found_persp.is_some() {
                frame_log!(;"persp-shape btw!");
                None
            } else if !fcam.is_empty() {
                fcam.search(data, 2)
            } else { None };
            if found_cam.is_some() {
                frame_log!(;"cam-match btw!");
            }
        }
        return
    }
    let gran = GogglesShared::get_granularity() as usize;
    let wants_cam = !matched_cam && GogglesShared::wants_camera() && !GogglesShared::has_found_camera();
    let wants_persp = !matched_persp && GogglesShared::wants_perspective() && !GogglesShared::has_found_perspective();
    let mut persp_offset = None;
    let mut cam_offset = None;
    let searchspace = data.get(..0x68).unwrap_or(data);
    if wants_cam {
        let fcam = g2!(*&ferret.cam.camera);
        let require_persp_suffix = false;
        // TODO: fallback after a couple attempts? might be better to find persp and work back from that instead...
        let fallback_fuzzy = true;
        cam_offset = match fcam.find_m43_exact(searchspace, require_persp_suffix) {
            Some(Ok(cam_offset)) => Some(cam_offset),
            None if fallback_fuzzy => fcam.find_m43(searchspace, require_persp_suffix),
            _ => None,
        };
        if let Some(cam_offset) = cam_offset {
            persp_offset = Some(cam_offset + CameraFerret::M43_LEN32);
            unsafe {
                let m = data.get_unchecked(cam_offset..);
                let accuracy = fcam.expected_dir;
                CameraFerret::report_matrix(m, Some((cam_offset, true, accuracy)), resource.as_d3d().as_raw());
            }
        }
    }
    if wants_persp {
        let fpersp = g2!(*&ferret.cam.perspective);
        let persp_offset = match persp_offset {
            None => {
                fpersp.search(searchspace, gran)
                    .map(|found| unsafe { found.as_ptr().offset_from(data.as_ptr()) as usize })
            },
            Some(persp_offset) => {
                let matches = unsafe { fpersp.matches(data.get_unchecked(persp_offset..)) };
                matches.then_some(persp_offset)
            },
        };
        if let Some(persp_offset) = persp_offset {
            unsafe {
                let m = data.get_unchecked(persp_offset..);
                PerspectiveFerret::report_matrix(m, Some(persp_offset), resource.as_d3d().as_raw());
                if let Some(cam_smooth_offset) = persp_offset.checked_sub(CameraFerret::M43_LEN32) {
                    let mdata = data.get_unchecked(cam_smooth_offset..);
                    let m = SnatchMatrix::from(CameraFerret::m43_at_unchecked(mdata));
                    if CameraFerret::matrix_matches_shape43(&m.data) {
                        g2!(*&mut ferret.cam.snatch_camera_smooth = m);
                    } else {
                        // TODO: try to prioritise this? seems kinda fine though since the buffer is usually updated multiple times per frame
                        #[cfg(todo = "unnecessary")]
                        GogglesShared::clear_perspective_found();
                    }
                }
            }
        }
    }
}

pub struct CameraShared {
    pub size_range: ops::Range<u16>,
    pub perspective: PerspectiveFerret,
    pub camera: CameraFerret,
    pub granularity: u8,
    pub snatch_camera: SnatchMatrix,
    pub found_camera: Option<(usize, usize, bool)>,
    pub found_perspective: Option<(usize, usize)>,
    pub snatch_perspective: SnatchMatrix,
    /// backup via perspective
    pub snatch_camera_smooth: SnatchMatrix,
}
impl CameraShared {
    pub const EMPTY: Self = Self {
        perspective: PerspectiveFerret::EMPTY,
        camera: CameraFerret::EMPTY,
        granularity: Self::DEFAULT_GRANULARITY,
        size_range: Self::SIZE_RANGE_EMPTY,
        snatch_camera: SnatchMatrix::DEFAULT,
        snatch_camera_smooth: SnatchMatrix::DEFAULT,
        snatch_perspective: SnatchMatrix::DEFAULT,
        found_perspective: None,
        found_camera: None,
    };
    const SIZE_RANGE_EMPTY: ops::Range<u16> = 0u16..0u16;
    const DEFAULT_GRANULARITY: u8 = 2;
}
impl GogglesShared {
    pub fn get_granularity() -> u8 {
        g2!(*&ferret.cam.granularity).max(1)
    }
    #[inline]
    pub fn set_granularity(v: u8) {
        g2!(*&mut ferret.cam.granularity = v)
    }
    pub fn wants_perspective() -> bool {
        g2!(*&ferret.cam.perspective.expected_h).to_bits() != GogglesShared::ZERO32
    }
    #[inline]
    pub fn set_perspective(v: PerspectiveFerret) {
        g2!(*&mut ferret.cam.perspective = v)
    }
    pub fn wants_camera() -> bool {
        !g2!(*&ferret.cam.camera.expected_dir.x).is_infinite()
    }
    #[inline]
    pub fn set_camera(v: CameraFerret) {
        g2!(*&mut ferret.cam.camera = v)
    }
    #[inline]
    pub fn set_size_range(v: ops::Range<u16>) {
        g2!(*&mut ferret.cam.size_range = v)
    }
    #[inline]
    pub fn trip_snatch_camera() {
        g2!(*&mut ferret.cam.snatch_camera = SnatchMatrix::DEFAULT)
    }
    #[inline]
    pub fn snatch_camera() -> SnatchMatrix {
        g2!(*&ferret.cam.snatch_camera)
    }
    pub fn wants_snatch_camera() -> bool {
        let w = g2!(&raw const ferret.cam.snatch_camera.data.w_axis);
        unsafe { (&*w).w }.is_infinite()
    }
    #[inline]
    pub fn clear_camera_found() {
        g2!(*&mut ferret.cam.found_camera = None)
    }
    #[inline]
    pub fn set_camera_found(buf: *mut c_void, off: usize, is_m43: bool) {
        g2!(*&mut ferret.cam.found_camera = Some((buf as usize, off, is_m43)))
    }
    /// TODO: flag instead?
    /// (same for has_found_perspective)
    #[inline]
    pub fn has_found_camera() -> bool {
        Self::found_camera().is_some()
    }
    pub fn found_camera() -> Option<(usize, usize, bool)> {
        g2!(*&ferret.cam.found_camera)
    }
    #[inline]
    pub fn trip_snatch_perspective() {
        g2!(*&mut ferret.cam.snatch_perspective = SnatchMatrix::DEFAULT)
    }
    #[inline]
    pub fn snatch_perspective() -> SnatchMatrix {
        g2!(*&ferret.cam.snatch_perspective)
    }
    pub fn wants_snatch_camera_smooth() -> bool {
        let w = g2!(&raw const ferret.cam.snatch_camera_smooth.data.w_axis);
        unsafe { (&*w).w }.is_infinite()
    }
    #[inline]
    pub fn trip_snatch_camera_smooth() {
        g2!(*&mut ferret.cam.snatch_camera_smooth = SnatchMatrix::DEFAULT)
    }
    #[inline]
    pub fn snatch_camera_smooth() -> SnatchMatrix {
        g2!(*&ferret.cam.snatch_camera_smooth)
    }
    pub fn wants_snatch_perspective() -> bool {
        let w = g2!(&raw const ferret.cam.snatch_perspective.data.w_axis);
        unsafe { (&*w).w }.is_infinite()
    }
    #[inline]
    pub fn clear_perspective_found() {
        g2!(*&mut ferret.cam.found_perspective = None)
    }
    #[inline]
    pub fn set_perspective_found(buf: *mut c_void, off: usize) {
        g2!(*&mut ferret.cam.found_perspective = Some((buf as usize, off)))
    }
    #[inline]
    pub fn has_found_perspective() -> bool {
        Self::found_perspective().is_some()
    }
    pub fn found_perspective() -> Option<(usize, usize)> {
        g2!(*&ferret.cam.found_perspective)
    }
}
#[derive(Debug, Copy, Clone)]
pub struct PerspectiveFerret {
    pub expected_w: f32,
    pub expected_h: f32,
}
impl PerspectiveFerret {
    pub const EMPTY: Self = Self { expected_w: 0.0, expected_h: 0.0 };
    pub fn new(fov_y: f32, aspect_ratio: f32) -> Self {
        let mut ferret = Self::EMPTY;
        ferret.set_expected_perspective(fov_y, aspect_ratio);
        ferret
    }
    pub fn with_parts(expected_w: f32, expected_h: f32) -> Self {
        Self {
            expected_w,
            expected_h,
        }
    }
    pub const fn is_empty(&self) -> bool {
        self.expected_h.to_bits() == GogglesShared::ZERO32
    }

    pub fn set_expected_perspective(&mut self, fov_y: f32, aspect_ratio: f32) {
        let fov = 0.5 * fov_y;
        self.expected_h = match fov {
            #[cfg(todo = "unnecessary")]
            fov => fov.tan().recip(),
            fov => {
                let (fov_sin, fov_cos) = fov.sin_cos();
                fov_cos / fov_sin
            },
        };
        self.expected_w = self.expected_h / aspect_ratio;
    }
    const M4_LEN32: usize = 4 * 4;
    /// column-major
    const M4_PERSP_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 1, 0,
    ];
    /// row-major
    #[cfg(todo)]
    const M4_PERSP_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 1,
        0, 0, 0, 0,
    ];
    /// 1.0 @ (2,2)
    const M4_PERSP_ONE: usize = 8;
    const M4_PERSP_EPSILON: f32 = 0.05;
    const M4_ZERO_EPSILON: f32 = 0.00001;
    #[inline]
    pub fn matches_pre(&self, data: &[u32]) -> bool {
        Self::matches_shape(data)
    }
    pub fn matches_shape(data: &[u32]) -> bool {
        if data.len() < Self::M4_LEN32 {
            return false
        }
        let mut zerocount = 0;
        let checks = Self::M4_PERSP_MASK
            .iter()
            .take(Self::M4_LEN32)
            .zip(data)
            .filter_map(|(mask, &v)| match *mask {
                false => Some(v),
                true => {
                    if v == GogglesShared::ZERO32 || matches!(f32::from_bits(v), -0.00001..=0.00001) {
                        zerocount += 1;
                    }
                    None
                },
            });
        for (i, v) in checks.enumerate() {
            let f = f32::from_bits(v);
            let expectedf = match i {
                Self::M4_PERSP_ONE => 1.0,
                _ => 0.0,
            };
            if !((f - expectedf).abs() <= Self::M4_ZERO_EPSILON) {
                return false
            }
        }
        zerocount == 0
    }
    /// post-filter used after checking [Self::matches_pre]
    pub unsafe fn matches(&self, data: &[u32]) -> bool {
        if self.is_empty() {
            return true
        }
        let exp = [self.expected_w, self.expected_h];
        let checks = Self::M4_PERSP_MASK
            .iter_ones()
            .map(|i| f32::from_bits(*unsafe { data.get_unchecked(i) }))
            .zip(exp);
        for (v, e) in checks {
            let delta = (v - e).abs();
            if delta > Self::M4_PERSP_EPSILON {
                return false
            }
        }
        true
    }

    /// matches pattern like [-1.204  -0.081  -0.000  -0.000] where first num
    /// is probably `1/tan(fov/2)*aspectratio`, expected to precede camera matrix
    pub fn matches_compact(&self, data: &[u32]) -> bool {
        let &[w, _unk, _tiny0, _tiny1, ..] = data else { return false };
        let w = f32::from_bits(w);
        if (w - self.expected_w).abs() > Self::M4_PERSP_EPSILON { return false }

        true
    }
    pub const COMPACT_LEN32: usize = 4;

    unsafe fn report_matrix(m: &[u32], offset: Option<usize>, buf_dest: *mut c_void) {
        if GogglesShared::wants_snatch_perspective() {
            let m = CameraFerret::m4_at_unchecked(m);
            g2!(*&mut ferret.cam.snatch_perspective = m.into());
            if let Some(offset) = offset {
                if !GogglesShared::has_found_perspective() {
                    log::info!("found new perspective {buf_dest:p}@{offset:#x}");
                    GogglesShared::set_perspective_found(buf_dest, offset);
                }
            }
        }
    }
    fn find_anywhere(&self, data: &[u32]) -> Option<ops::RangeInclusive<usize>> {
        if self.is_empty() { return None }
        let exp = [self.expected_w, self.expected_h];
        find_all(data, &exp, 0.01)
    }
}
impl FerretPattern for PerspectiveFerret {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]> {
        search_ferret(
            data,
            granularity,
            |data| self.matches_pre(data),
            |data| unsafe { self.matches(data) }.then_some(Self::M4_LEN32),
            Some(Self::M4_LEN32),
        )
    }
}

#[derive(Debug, Copy, Clone)]
pub struct CameraFerret {
    pub expected_dir: glam::Vec3,
    pub expected_pos: glam::Vec3,
    /// TODO: any other axis would be more reliable
    /// (fewer false positives, imprecise due to up or side vector)
    pub expected_eye_vector_z: f32,
}
impl CameraFerret {
    pub const EMPTY: Self = Self { expected_dir: glam::Vec3::INFINITY, expected_pos: glam::Vec3::INFINITY, expected_eye_vector_z: f32::INFINITY };
    pub fn new(pos: Point3<LocalSpace>, dir: Vector3<LocalSpace>, up: Vector3<LocalSpace>) -> Self {
        let mut ferret = Self::EMPTY;
        ferret.set_expected(pos, dir, up);
        ferret
    }
    pub(crate) fn with_game(pos: Point3<GameSpace>, dir: Vector3<GameSpace>, _up: Vector3<GameSpace>, expected_eye_vector_z: f32) -> Self {
        Self {
            expected_dir: dir.to_raw(),
            expected_pos: pos.to_raw(),
            expected_eye_vector_z,
        }
    }
    pub const fn is_empty(&self) -> bool {
        self.expected_dir.x.is_infinite()
    }

    pub fn set_expected(&mut self, pos: Point3<LocalSpace>, dir: Vector3<LocalSpace>, _up: Vector3<LocalSpace>) {
        self.expected_dir = LocalSpace::norm_to_game(dir).to_raw();
        self.expected_pos = LocalSpace::to_game(pos).to_raw();
        self.expected_eye_vector_z = -self.expected_pos.dot(self.expected_dir);
    }
    const M4_LEN32: usize = 4 * 4;
    const M43_LEN32: usize = 4 * 3;
    /// column-major
    #[cfg(todo)]
    const M4_CAM_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 1, 1, 0,
        1, 1, 1, 0,
        1, 1, 1, 0,
        1, 1, 1, 0,
    ];
    /// row-major
    const M4_CAM_MASK: BitArray<[u32; 1], Lsb0> = bitvec::bitarr![
        const u32, Lsb0;
        1, 1, 1, 1,
        1, 1, 1, 1,
        1, 1, 1, 1,
        0, 0, 0, 0,
    ];
    /// 1.0 @ (2,2)
    const M4_CAM_ONE: usize = 3;
    const M4_CAM_Z_EPSILON: f32 = 0.02;
    const M4_CAM_DIR_EPSILON: f32 = Self::M4_CAM_Z_EPSILON;
    const M4_CAM_POS_EPSILON: f32 = 0.1;
    const M4_ZERO_EPSILON: f32 = 0.0005;
    pub fn matches_pre(&self, data: &[u32]) -> bool {
        if data.len() < Self::M4_LEN32 {
            return false
        }
        let mut zerocount = 0;
        let checks = Self::M4_CAM_MASK
            .iter()
            .take(Self::M4_LEN32)
            .zip(data)
            .filter_map(|(mask, &v)| match *mask {
                false => Some(v),
                true => {
                    if v == GogglesShared::ZERO32 || matches!(f32::from_bits(v), -0.00001..=0.00001) {
                        zerocount += 1;
                    }
                    None
                },
            });
        for (i, v) in checks.enumerate() {
            let f = f32::from_bits(v);
            let expectedf = match i {
                Self::M4_CAM_ONE => 1.0,
                _ => 0.0,
            };
            if !((f.abs() - expectedf).abs() <= Self::M4_ZERO_EPSILON) {
                return false
            }
        }
        zerocount <= 5
    }
    /// post-filter used after checking [Self::matches_pre]
    pub unsafe fn matches(&self, data: &[u32]) -> bool {
        if self.is_empty() {
            return true
        }
        let look_elems = unsafe {
            &*(data.as_ptr() as *const u32 as *const [f32; Self::M4_LEN32])
        };
        let look = glam::Mat4::from_cols_array(look_elems);
        // TODO: only check z, then move on to decompose below
        let look_dir = &look.z_axis;
        #[cfg(todo)]
        for (&d, exp) in dir.to_array().iter().zip(self.expected_dir.to_array()) {
            // TODO: check signs?
        }
        if (look_dir.truncate().normalize().y.abs() - self.expected_dir.y.abs()).abs() > Self::M4_CAM_Z_EPSILON {
            frame_log!("\tcamdir.y mismatch of {look_dir:?}");
            print_ferret(data, 0, Self::M4_LEN32);
            return false
        }

        let (look, look_eye) = coords::decompose_look_to32_rows(look);
        let (eye, dir, _up) = coords::decompose_look32_rows(look, look_eye);
        #[cfg(todo)]
        let (exp, found) = (self.expected_dir.xz(), dir.xz());
        let (exp, found) = (Vec3A::from(self.expected_dir), dir);
        if !exp.abs_diff_eq(found, Self::M4_CAM_DIR_EPSILON) {
            frame_log!("\tcamdir mismatch of {dir:?}");
            print_ferret(data, 0, Self::M4_LEN32);

            return false
        }
        if !self.expected_pos.x.is_infinite() {
            // TODO: check up vector too?
            if !self.expected_pos.abs_diff_eq(eye.into(), Self::M4_CAM_POS_EPSILON) {
                frame_log!("\tcampos mismatch of {eye:?}, expected: {:?}", self.expected_pos);
                print_ferret(data, 0, Self::M4_LEN32);
                return false
            }
        }

        #[cfg(todo)]
        let check_shuffle = data.get(Self::M4_LEN32..)
            .map(|d| Self::check_shuffle(d));
        #[cfg(todo)]
        if check_shuffle != Some(true) {
            frame_log!("\tshuffle failed!");
            print_ferret(data, 0, Self::M4_LEN32);
            return false
        }

        true
    }
    unsafe fn m4_at_unchecked(data: &[u32]) -> Mat4 {
        let look_elems = unsafe {
            &*(data.as_ptr() as *const u32 as *const [f32; Self::M4_LEN32])
        };
        Mat4::from_cols_array(look_elems)
    }
    unsafe fn m43_at_unchecked(data: &[u32]) -> Mat4 {
        let look_elems = unsafe {
            &*(data.as_ptr() as *const u32 as *const [f32; Self::M43_LEN32])
        };
        Mat43::from_cols_array_ref(look_elems).to_mat4().into()
    }
    fn eye_matches(&self, look: Mat4, check_dir: bool) -> bool {
        let (look, look_eye) = coords::decompose_look_to32_rows(look);
        let (eye, dir, _up) = coords::decompose_look32_rows(look, look_eye);
        if check_dir {
            #[cfg(todo)]
            let (exp, found) = (self.expected_dir.xz(), dir.xz());
            let (exp, found) = (Vec3A::from(self.expected_dir), dir);
            if !exp.abs_diff_eq(found, Self::M4_CAM_DIR_EPSILON) {
                return false
            }
            // TODO: check up vector too?
        }
        if !self.expected_pos.abs_diff_eq(eye.into(), Self::M4_CAM_POS_EPSILON) {
            //log::debug!("failed to match eye in {eye:?}, expected {:?}", self.expected_pos);
            return false
        }
        true
    }
    fn dir_x_matches(&self, data: &[u32]) -> bool {
        let Some(x) = data.get(Self::M43_DIR_OFFSET) else { return false };
        let x = f32::from_bits(*x);
        let exp = self.expected_dir.x;
        (x - exp).abs() <= Self::M4_CAM_Z_EPSILON
    }
    fn dir_up_matches(&self, data: &[u32]) -> bool {
        let Some(up_z) = data.get(Self::M43_DIR_OFFSET - 2) else { return false };
        let up_z = f32::from_bits(*up_z);
        if Self::up_matches(up_z) { return true }
        let Some(z) = data.get(Self::M43_DIR_OFFSET + 2) else { return false };
        let z = f32::from_bits(*z);
        let exp = self.expected_dir.z;
        (z - exp).abs() <= Self::M4_CAM_Z_EPSILON || Self::up_matches(z)
    }
    #[inline]
    fn up_matches(z: f32) -> bool {
        matches!(z, -1.0..-0.953)
    }
    fn dir_matches(&self, data: &[u32], check_z: bool) -> bool {
        let z_axis = match data.get(Self::M43_DIR_OFFSET..) {
            Some(&[x, y, z, w, ..]) => glam::Vec4::new(
                f32::from_bits(x), f32::from_bits(y), f32::from_bits(z), f32::from_bits(w),
            ),
            _ => return false,
        };
        let exp = Vec3A::from(self.expected_dir);
        if !Vec3A::from_vec4(z_axis).abs_diff_eq(exp, Self::M4_CAM_DIR_EPSILON) {
            if z_axis.y != 0.0 && z_axis.z != 0.0 {
                //log::debug!("failed to match dir in {z_axis:?}, expected {exp:?}");
            }
            return false
        }
        match check_z {
            true => {
                let res = (z_axis.w - self.expected_eye_vector_z).abs() <= Self::M4_CAM_POS_EPSILON;
                if !res {
                    //log::debug!("failed to match eye.z in {z_axis:?}, expected {:?}", self.expected_eye_vector_z);
                }
                res
            },
            false => true,
        }
    }
    pub fn find_m43_exact(&self, data: &[u32], require_persp_suffix: bool) -> Option<Result<usize, ()>> {
        let off = self.bitfind_m43_offset(data)?;
        let suffix = data.get((off + Self::M43_LEN32)..)?;
        if !self.dir_up_matches(unsafe { data.get_unchecked(off..) }) { return None }
        if !require_persp_suffix && Self::is_w(suffix) {
        } else if !require_persp_suffix || !PerspectiveFerret::matches_shape(suffix) {
            return Some(Err(()))
        }
        let m43 = unsafe {
            let found = data.get_unchecked(off..);
            Self::m43_at_unchecked(found)
        };
        Some(self.eye_matches(m43, false)
            .then_some(off).ok_or(())
        )
    }
    pub fn find_m43(&self, data: &[u32], require_persp_suffix: bool) -> Option<usize> {
        search_ferret(
            data,
            4,
            |data| {
                if !self.dir_x_matches(data) || !self.dir_up_matches(data) {
                    return false
                }
                let matches = self.dir_matches(data, true);
                if !matches { return false }
                let Some(suffix) = data.get(Self::M43_LEN32..) else { return false };
                if !require_persp_suffix && Self::is_w(suffix) {
                } else if !require_persp_suffix || !PerspectiveFerret::matches_shape(suffix) {
                    return false
                }
                true
            },
            |data| unsafe {
                let m43 = Self::m43_at_unchecked(data);
                self.eye_matches(m43, true).then_some(Self::M43_LEN32)
            },
            None,
        ).map(|found| unsafe { found.as_ptr().offset_from(data.as_ptr()) as usize })
    }
    fn is_w(data: &[u32]) -> bool {
        let axis = match data {
            &[x, y, z, w, ..] => glam::Vec4::new(
                f32::from_bits(x), f32::from_bits(y), f32::from_bits(z), f32::from_bits(w),
            ),
            _ => return false
        };
        axis == glam::Vec4::W
    }
    const M43_DIR_OFFSET: usize = 4 * 2;
    pub fn bitfind_m43_offset(&self, data: &[u32]) -> Option<usize> {
        let searchspace = data.get(Self::M43_DIR_OFFSET..).unwrap_or(&[]);
        let off = self.bitfind_dir3a_offset(searchspace)?;

        let eye_z = searchspace.get(off + 3)
            .map(|eye_z| (f32::from_bits(*eye_z) - self.expected_eye_vector_z).abs() <= Self::M4_CAM_POS_EPSILON);
        if !eye_z.unwrap_or(false) { return None }

        Some(off)
    }
    pub fn bitfind_dir3a_offset(&self, data: &[u32]) -> Option<usize> {
        let exp_x = self.expected_dir.x.to_bits();
        // XXX: could allow trailing 3 but unlikely to not be 128bit aligned...
        let mut off = 0usize;
        while off + 4 <= data.len() {
            let [x, y, z] = unsafe {
                &*(data.as_ptr().add(off) as *const [u32; 3])
            };
            if *x == exp_x && *y == self.expected_dir.y.to_bits() && *z == self.expected_dir.z.to_bits() {
                return Some(off)
            }
            off += 4;
        }
        None
    }

    unsafe fn report_matrix(m: &[u32], offset: Option<(usize, bool, glam::Vec3)>, buf_dest: *mut c_void) {
        let m = SnatchMatrix::from(CameraFerret::m43_at_unchecked(m));
        let dir = m.data.z_axis;
        if GogglesShared::wants_snatch_camera() {
            g2!(*&mut ferret.cam.snatch_camera = m);
        }
        if let Some((offset, is_m43, accuracy)) = offset {
            let accuracy = ((dir.truncate() - accuracy).abs().element_sum() * 100.0).min(u8::MAX as f32) as u8;
            CameraSearch::with_mut_unchecked(|s| s.report(buf_dest, CameraMatch {
                #[cfg(feature = "goggles2-project")]
                drawn: super::project::ProjectShared::has_drawn_space(),
                is_m43,
                offset: offset as _,
                accuracy,
            }));
        }
    }
    /// row-major btw
    const SHUFFLE_EXPECTED_M4: glam::Mat4 = glam::Mat4::from_cols(
        glam::Vec4::X,
        glam::Vec4::Y,
        glam::Vec4::X,
        glam::Vec4::Y,
    );
    const SHUFFLE_EXPECTED: [f32; 16] = Self::SHUFFLE_EXPECTED_M4.to_cols_array();
    /// :<
    const SHUFFLE_EXPECTED2: [f32; 8] = [
        Self::SHUFFLE_EXPECTED[0], Self::SHUFFLE_EXPECTED[1], Self::SHUFFLE_EXPECTED[2], Self::SHUFFLE_EXPECTED[3],
        Self::SHUFFLE_EXPECTED[4], Self::SHUFFLE_EXPECTED[5], Self::SHUFFLE_EXPECTED[6], Self::SHUFFLE_EXPECTED[7],
        //0.0, 0.0, 0.0, 1.0,
    ];
    fn check_shuffle(data: &[u32]) -> bool {
        let dataf =
        data.iter()
            .map(|d| f32::from_bits(*d))
            .chain(core::iter::repeat(f32::MAX));
        if dataf.clone()
            .zip(Self::SHUFFLE_EXPECTED.iter())
            .all(|(d, ex)| d == *ex) { return true }
        if dataf
            .zip(Self::SHUFFLE_EXPECTED2.iter())
            .all(|(d, ex)| d == *ex) { return true }
        false
    }
    fn find_anywhere(&self, data: &[u32]) -> Option<ops::RangeInclusive<usize>> {
        if self.is_empty() { return None }
        find_all(data, &self.expected_dir.to_array(), Self::M4_ZERO_EPSILON)
    }

    fn matrix_matches_shape43(look: &glam::Mat4) -> bool {
        #[cfg(todo)]
        let (side, dir) = (&look.x_axis, &look.z_axis);
        let up = &look.y_axis;
        if Self::up_matches(up.z) {
            return true
        }
        let cam = unsafe {
            &*g2!(&raw const ferret.cam.camera)
        };
        let z = look.z_axis.z;
        #[cfg(todo)]
        let up_z = look.y_axis.z;
        if !cam.is_empty() && (cam.expected_dir.z - z).abs() <= Self::M4_CAM_Z_EPSILON * 6.0 {
            return true
        }
        false
    }
}
fn find_all(data: &[u32], needle: &[f32], eps: f32) -> Option<ops::RangeInclusive<usize>> {
    #[cfg(todo)]
    if needle.is_empty() { return None }
    let mut found = Vec::new();
    'data: for (i, &d) in data.iter().enumerate() {
        if matches!(d, GogglesShared::ZERO32 | GogglesShared::NEGZERO32) { continue }
        let f = f32::from_bits(d);
        let fabs = f.abs();
        for (ni, &n) in needle.iter().enumerate() {
            let nabs = n.abs();
            if !((fabs - nabs).abs() <= eps) {
                continue
            }
            if found.len() <= ni {
                found.resize(ni + 1, usize::MAX);
            }
            let fi = unsafe { found.get_unchecked_mut(ni) };
            if *fi != usize::MAX { continue }
            *fi = i;
            if found.iter().filter(|&&fi| fi != usize::MAX).count() >= needle.len() {
                break 'data
            }
            #[cfg(todo)]
            break;
        }
    }
    let found = found.iter().filter(|&&fi| fi != usize::MAX);
    if found.clone().count() >= needle.len() {
        found.clone().min()
            .and_then(move |&min| found.max().map(|&max| min..=max))
    } else {
        None
    }
}
impl FerretPattern for CameraFerret {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]> {
        let found = search_ferret(
            data,
            granularity,
            |data| self.matches_pre(data),
            |data| unsafe { self.matches(data) }.then_some(Self::M4_LEN32),
            Some(Self::M4_LEN32),
        )?;
        let offset = unsafe { found.as_ptr().offset_from(data.as_ptr()) } as usize;

        Some(found)
    }
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct SnatchMatrix {
    pub data: glam::Mat4,
}
impl SnatchMatrix {
    pub const DEFAULT: Self = Self {
        data: glam::Mat4::from_cols(
            glam::Vec4::X,
            glam::Vec4::Y,
            glam::Vec4::Z,
            glam::Vec4::INFINITY,
        ),
    };
    pub const fn new(data: glam::Mat4) -> Self {
        Self { data }
    }
    pub fn is_empty(&self) -> bool {
        self.data.w_axis.w.is_infinite()
    }

    #[inline]
    pub fn get_as_look_raw(&self) -> (Vec3A, Vec3A, Vec3A) {
        let (look, look_eye) = coords::decompose_look_to32_rows(self.data);
        coords::decompose_look32_rows(look, look_eye)
    }
    pub fn get_as_look(&self) -> (Point3<LocalSpace>, Vector3<LocalSpace>) {
        let (eye, dir, _up) = self.get_as_look_raw();
        (GameSpace::to_local(eye.into()).into(), GameSpace::norm_to_local(dir.into()).into())
    }
    pub fn get_look_up(&self) -> glamour::Vector3<LocalSpace> {
        GameSpace::norm_to_local(
            self.data.y_axis.truncate().into()
        )
    }
    pub fn get_look_side(&self) -> glamour::Vector3<LocalSpace> {
        GameSpace::norm_to_local(
            self.data.x_axis.truncate().into()
        )
    }
    pub fn get_ferret_look(&self) -> CameraFerret {
        let (eye, dir, up) = {
            let (look, look_eye) = coords::decompose_look_to32_rows(self.data);
            coords::decompose_look32_rows(look, look_eye)
        };
        #[cfg(todo)]
        let up = GameSpace::UP;
        CameraFerret::with_game(
            eye.into(),
            dir.into(),
            up.into(),
            self.data.z_axis.w,
        )
    }
    pub fn get_as_perspective(&self) -> (f32, ops::Range<f32>) {
        let h = self.data.y_axis.y;
        #[cfg(todo)]
        let _d = self.data.z_axis.w;
        let r_neg = -self.data.z_axis.z;
        let c = self.data.w_axis.z;
        let near = c / r_neg;
        let far = c / (1.0 + r_neg);
        (h, near..far)
    }
    pub fn perspective_aspect_ratio(&self) -> f32 {
        let h = self.data.y_axis.y;
        let w = self.data.x_axis.x;
        h / w
    }
    pub fn get_ferret_perspective(&self) -> PerspectiveFerret {
        let h = self.data.y_axis.y;
        let w = self.data.x_axis.x;
        PerspectiveFerret::with_parts(
            w,
            h,
        )
    }
}
impl From<glam::Mat4> for SnatchMatrix {
    fn from(data: glam::Mat4) -> Self {
        Self::new(data)
    }
}
impl Default for SnatchMatrix {
    fn default() -> Self { Self::DEFAULT }
}

#[derive(Debug, Copy, Clone, PartialOrd, Ord, PartialEq, Eq, Hash)]
pub struct CameraMatch {
    #[cfg(feature = "goggles2-project")]
    pub drawn: bool,
    pub is_m43: bool,
    pub accuracy: u8,
    pub offset: u32,
}
#[derive(Debug)]
pub struct CameraSearch {
    pub matches: BTreeMap<usize, CameraMatch>,
    pub blacklist: BTreeSet<usize>,
    pub seen: BTreeSet<usize>,
}
impl CameraSearch {
    pub const DEFAULT: Self = Self {
        matches: BTreeMap::new(),
        blacklist: BTreeSet::new(),
        seen: BTreeSet::new(),
    };
    /// no need to clear everything (like blacklist) every frame...
    pub fn clear_active(&mut self) {
        if self.blacklist.len() * 5 / 4 > self.seen.len() {
            // in case we miss the start of a frame and get confused...
            self.blacklist.clear();
        }
        self.matches.clear();
        self.seen.clear();
    }
    pub fn clear(&mut self) {
        self.clear_active();
        self.blacklist.clear();
    }
    pub fn visit(&mut self, resource: *mut c_void) -> bool {
        let key = resource as usize;
        let res = self.seen.insert(key);
        if !res {
            self.blacklist.insert(key);
        }
        res
    }
    pub fn report(&mut self, resource: *mut c_void, m: CameraMatch) {
        let key = resource as usize;
        if self.matches.insert(key, m).is_some() {
            self.blacklist.insert(key);
        }
    }
    pub fn distill(&self) -> Option<(*mut c_void, CameraMatch)> {
        self.matches.iter()
            .filter(|&(k, _)| !self.blacklist.contains(k))
            .map(|(k, m)| (*k as *mut c_void, *m))
            .min_by_key(|(_, m)| *m)
    }
    pub fn with_mut_unchecked<R, F: FnOnce(&mut Self) -> R>(f: F) -> R {
        use sync_unsafe_cell::SyncUnsafeCell;
        static SEARCH: SyncUnsafeCell<CameraSearch> = SyncUnsafeCell::new(CameraSearch::DEFAULT);
        let search = SEARCH.get();
        unsafe {
            f(&mut *search)
        }
    }
    #[inline]
    pub fn seen_frame(&self) -> bool {
        !self.seen.is_empty()
    }
}

#[derive(Debug, Clone, Default)]
pub struct GogglesCamera {
    pub camera_enabled: bool,
    pub camera_paused: bool,
    pub camera_lost: u16,
    pub perspective_search: bool,
    pub dir_search: bool,
    pub debug_toggle_up: bool,
    pub debug_smooth: bool,
    pub debug_interpolate: bool,
    pub debug_interpolate_off: bool,
    pub perspective_lost: u16,
    pub perspective_params: (f32, f32, f32, f32),
    pub save_map: Option<MapIndex>,
}
impl GogglesCamera {
    pub(super) fn act_map_enter(&mut self, hard: bool, map_id: MapIndex) {
        self.camera_paused = false;
        if self.camera_enabled {
            if hard {
                self.camera_clear();
                self.save_map = Some(map_id);
            } else {
                self.reset_search();
                if self.save_map != Some(map_id) {
                    self.save_map = None;
                }
            }
        }
    }
    pub(super) fn act_map_exit(&mut self) {
        self.save_map = None;
    }
    /// TODO: awkwardly called by engine, hacky...
    pub(super) fn act_render_post(&mut self) {
        if !self.camera_enabled { return }

        if CameraSearch::with_mut_unchecked(|s| s.seen_frame()) {
            if let Some((_b, _o, _is_m43)) = GogglesShared::found_camera() {
                let lost_cam = GogglesShared::wants_snatch_camera();
                if lost_cam {
                    if self.camera_lost == 0 {
                        log::debug!("lost cambuf {:p}@{_o:#x}", _b as *mut ());
                    }
                    self.camera_lost = self.camera_lost.max(1);
                    GogglesShared::clear_camera_found();
                } else {
                    self.camera_lost = Default::default();
                }
            }

            if let Some((_b, _o)) = GogglesShared::found_perspective() {
                let lost_persp = GogglesShared::wants_snatch_perspective();
                if lost_persp {
                    if self.perspective_lost == 0 {
                        log::debug!("lost perspbuf {:p}@{_o:#x}", _b as *mut ());
                    }
                    self.perspective_lost = self.perspective_lost.max(1);
                    GogglesShared::clear_perspective_found();
                } else {
                    self.perspective_lost = Default::default();
                }
            }
        }

        CameraSearch::with_mut_unchecked(|s| s.clear_active());
        if !self.camera_paused {
            if self.dir_search {
                GogglesShared::trip_snatch_camera();
            }
            if self.perspective_search {
                GogglesShared::trip_snatch_camera_smooth();
                GogglesShared::trip_snatch_perspective();
            }
        }
    }

    pub(crate) fn act_pre_render(&mut self, _visible: bool) {
        if self.camera_enabled && self.dir_search && GogglesShared::wants_camera() {
            let mut alternatives = 0;
            let found = CameraSearch::with_mut_unchecked(|s| {
                alternatives = s.matches.len();
                s.distill()
            });
            if let Some((buf_dest, found)) = found {
                log::info!("found new cam at {buf_dest:p}@{:#x} out of {alternatives} choices", found.offset);
                GogglesShared::set_camera_found(buf_dest, found.offset as _, found.is_m43);
            }
        }
    }
    #[cfg(deleteme)]
    pub(crate) fn act_pre_render_post(&mut self, visible: bool) {
        if visible {
            self.camera_commit_perspective();
        }
    }
    pub(crate) fn reset_search(&mut self) {
        if self.perspective_search {
            self.perspective_lost = self.perspective_lost.min(1);
        }
        if self.dir_search {
            self.camera_lost = self.camera_lost.min(1);
        }
    }

    pub(crate) fn camera_init(&mut self, enables: GogglesEnables) {
        self.dir_search = enables.contains(GogglesEnables::CAMERA_DIR);
        self.perspective_search = enables.contains(GogglesEnables::CAMERA_PERSPECTIVE);
        self.camera_enable();
    }
    pub(crate) fn camera_enable(&mut self) {
        self.camera_enabled = true;
        self.camera_clear();
        #[cfg(todo)]
        let (min, max) = (0x160, 0x1b0+1);
        #[cfg(todo)]
        let (min, max) = (60, 0x390);
        let (min, max) = (60, 0x5c0);
        GogglesShared::set_size_range(min..max + 1);
        GogglesShared::set_granularity(4);
        //GogglesShared::set_granularity(8);
    }
    pub(crate) fn camera_disable(&mut self) {
        self.camera_enabled = false;
        self.camera_clear();
        self.save_map = None;
        GogglesShared::set_size_range(CameraShared::SIZE_RANGE_EMPTY);
    }
    fn camera_lost_defer(lost: &mut u16, update: u8) -> bool {
        let mut deferred = update != 0;
        let retry = match *lost {
            0 => return false,
            1 => {
                deferred = false;
                true
            },
            2..=3 => true,
            lost @ 4..=0x100 => lost & 0x0f == 0,
            lost => lost & 0xff == 0,
        };
        if !deferred {
            let next = lost.wrapping_add(1);
            *lost = match next {
                0 => 0x100,
                lost => lost,
            };
        }
        !retry
    }
    pub(crate) fn camera_setup(&mut self, cam: Option<RenderPosition<DrawSpace>>, (fov_y, aspect): (f32, f32), update: u8) {
        self.camera_paused = false;
        let cam = match cam {
            Some(..) if !self.dir_search => {
                self.camera_lost = Default::default();
                None
            },
            Some(..) if Self::camera_lost_defer(&mut self.camera_lost, update) => None,
            cam => cam,
        };
        if let Some((pos, dir, up)) = cam {
            let up = up.normalize_or(RenderMachine::LOCAL_UP);
            let cam = CameraFerret::new(pos, dir, up);
            GogglesShared::set_camera(cam);
        } else {
            GogglesShared::set_camera(CameraFerret::EMPTY);
        }
        let update_persp = || match cam.is_some() {
            //#[cfg(todo)]
            true => {
                // desynchronize updates to reduce per-frame impact of the search
                update.wrapping_sub(1)
            },
            _ => update,
        };
        if !self.perspective_search {
            GogglesShared::set_perspective(PerspectiveFerret::EMPTY);
            self.perspective_lost = Default::default();
        } else if !Self::camera_lost_defer(&mut self.perspective_lost, update_persp()) {
            let persp = PerspectiveFerret::new(fov_y, aspect);
            GogglesShared::set_perspective(persp);
        } else {
            GogglesShared::set_perspective(PerspectiveFerret::EMPTY);
        }
    }
    pub(crate) fn camera_pause(&mut self, intermission: bool) {
        self.camera_paused = !intermission;
        if intermission {
            let needs_persp = match () {
                #[cfg(todo)]
                _ => !GogglesShared::wants_perspective(),
                _ => self.perspective_search,
            };
            if needs_persp {
                let persp = GogglesShared::snatch_perspective();
                if !persp.is_empty() {
                    GogglesShared::set_perspective(persp.get_ferret_perspective());
                }
            }
            if self.dir_search {
                let cam = GogglesShared::snatch_camera();
                if !cam.is_empty() {
                    GogglesShared::set_camera(cam.get_ferret_look());
                }
            }
            // TODO: else if !fallback_cam.is_empty()?
        } else {
            GogglesShared::set_perspective(PerspectiveFerret::EMPTY);
            GogglesShared::set_camera(CameraFerret::EMPTY);
        }
    }
    pub(super) fn camera_clear(&mut self) {
        GogglesShared::clear_camera_found();
        GogglesShared::clear_perspective_found();
        GogglesShared::set_camera(CameraFerret::EMPTY);
        GogglesShared::set_perspective(PerspectiveFerret::EMPTY);
        CameraSearch::with_mut_unchecked(|s| s.clear());
        self.perspective_params = Default::default();
        self.perspective_lost = Default::default();
        self.camera_lost = Default::default();
    }
    pub(crate) fn perspective_params(&self) -> Option<(f32, f32, f32, f32)> {
        if self.has_persp() {
            Some(self.perspective_params)
        } else {
            None
        }
    }
    pub(crate) fn perspective_farz(&self) -> Option<MapProjectionDepth> {
        let (_, _, _n, far) = self.perspective_params()?;
        Some(MapProjectionDepth::with_far_in(far))
    }
    fn perspective_params_for(m: &SnatchMatrix) -> (f32, f32, f32, f32) {
        let (h, range) = m.get_as_perspective();
        let aspect = m.perspective_aspect_ratio();
        (h, aspect, range.start, range.end)
    }

    pub(crate) fn has_camera_primary(&self) -> bool {
        GogglesShared::has_found_camera() && !GogglesShared::wants_snatch_camera()
    }
    /// TODO
    pub(crate) fn has_camera(&self) -> bool {
        if !self.camera_enabled { return false }
        match () {
            _ if self.has_camera_primary() =>
                true,
            _ if self.has_camera_fallback() => true,
            _ => false,
        }
    }
    pub(crate) fn has_camera_fallback(&self) -> bool {
        match () {
            _ if !GogglesShared::has_found_perspective() => false,
            _ => !GogglesShared::wants_snatch_camera_smooth(),
        }
    }
    pub(crate) fn has_persp(&self) -> bool {
        self.perspective_params.0.to_bits() != 0.0f32.to_bits()
    }
    pub(crate) fn camera_commit_perspective(&mut self) {
        let had_persp = self.has_persp();
        if self.camera_enabled && !self.camera_paused && !GogglesShared::wants_snatch_perspective() {
            let persp = GogglesShared::snatch_perspective();
            #[cfg(deleteme)]
            if self.perspective_params().is_none() {
                let (h, range) = persp.get_as_perspective();
                let (_near, far) = (range.start, range.end);
                let map_id = crate::exports::runtime::mumble_link_ptr()
                    .map(|ml| ml.read_map_id()).unwrap_or(0);
                log::error!("ON.mapid={map_id:04} FOUND NEW PERSP.far = {far:?} ({_near}..{far}) h={h:?}");
            }
            self.perspective_params = Self::perspective_params_for(&persp);
        }

        if self.has_persp() && !had_persp {
            if let Some(map_id) = self.save_map.take() {
                let farz =
                    self.perspective_farz()
                        .map(|farz| (map_id, farz));
                #[cfg(taimi_debug)]
                log::debug!("saving? {:?}", self.perspective_params);
                let settings = farz.is_some().then(||
                    crate::SETTINGS.get().and_then(|s| s.try_write().ok())
                ).flatten();
                if let (Some((map_id, farz)), Some(mut settings)) = (farz, settings) {
                    let dirty = if let Some(pathing) = settings.pathing.as_mut() {
                        pathing.space.goggles.set_map_proj_seen_depth(map_id.get(), farz)
                    } else { false };
                    if dirty {
                        settings.mark_dirty();
                        #[cfg(taimi_debug)]
                        if let Some((h, _, near, far)) = self.perspective_params() {
                            log::error!("ON.mapid={map_id:04} FOUND NEW PERSP.far = {far:?} ({near}..{far}) h={h:?}")
                        }
                    }
                }
            }
        }
    }
    pub(super) fn set_perspective(&mut self, on: bool) {
        self.perspective_search = on;
        GogglesShared::clear_perspective_found();
        if !on {
            GogglesShared::set_perspective(PerspectiveFerret::EMPTY);
        }
        self.perspective_params = Default::default();
    }
    pub(super) fn set_dir(&mut self, on: bool) {
        self.dir_search = on;
        GogglesShared::clear_camera_found();
        if !on {
            GogglesShared::set_camera(CameraFerret::EMPTY);
        }
    }
}
