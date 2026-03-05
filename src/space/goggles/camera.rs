use {
    crate::render::{machine::frame_log, RenderState},
    core::{ffi::c_void, mem, num::NonZero, ptr, slice},
    glam::Mat4,
    glamour::{Point3, Vector3},
    std::collections::{BTreeMap, BTreeSet},
    sync_unsafe_cell::SyncUnsafeCell,
    taimi_d3d::buffer::math::Mat43,
    taimi_d3d::dx11::buffer::Buffer,
    taimi_d3d::dx11::Resource,
    taimi_meta::coords::{self, GameSpace, LocalSpace},
    //taimi_d3d::prelude::*,
    windows::core::Interface,
};
use {
    bitvec::{array::BitArray, order::Lsb0},
    core::ops,
};

pub(super) fn wants_update_camera(resource: *mut c_void) -> bool {
    let exp = FerretResource::found_camera()
        .map(|(b, ..)| b as *mut c_void)
        .unwrap_or(ptr::null_mut());
    resource == exp
}
pub(super) fn wants_update_perspective(resource: *mut c_void) -> bool {
    let exp = FerretResource::found_perspective()
        .map(|(b, ..)| b as *mut c_void)
        .unwrap_or(ptr::null_mut());
    resource == exp
}
pub(super) unsafe fn update_camera(
    resource: &Resource,
    subresource: u32,
    data: *const u8,
    dest_offset: u32,
    _len: Option<NonZero<u32>>,
) {
    if dest_offset != 0 || subresource != 0 {
        return
    }
    #[cfg(todo = "unnecessary")]
    if !FerretResource::wants_snatch_camera() {
        return
    }
    let Some((_expecting, offset, is_m43)) = FerretResource::found_camera() else {
        return
    };
    let Some(offset) = (offset * 4).checked_sub(dest_offset as usize) else {
        return
    };
    let data = data as *const u32;
    let precheck = slice::from_raw_parts(
        data.byte_add(offset + CameraFerret::M43_LEN32 * 4),
        CameraFerret::M4_LEN32,
    );
    let lost = match is_m43 {
        #[cfg(todo = "unused")]
        true => !PerspectiveFerret::matches_shape(precheck),
        _ => !CameraFerret::is_w(precheck),
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
        FerretResource::clear_camera_found();
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
    if dest_offset != 0 || subresource != 0 {
        return
    }
    if !FerretResource::wants_snatch_perspective() {
        return
    }
    let Some((_expecting, offset)) = FerretResource::found_perspective() else {
        return
    };
    let Some(offset) = (offset * 4).checked_sub(dest_offset as usize) else {
        return
    };
    let data = data as *const u32;
    let m = slice::from_raw_parts(data.byte_add(offset), CameraFerret::M4_LEN32);
    let lost = !PerspectiveFerret::matches_shape(m);
    if lost {
        FerretResource::clear_perspective_found();
        return
    }
    PerspectiveFerret::report_matrix(m, None, resource.as_d3d().as_raw());
    if let Some(cam_smooth_offset) = offset.checked_sub(CameraFerret::M43_LEN32 * 4) {
        let mdata = slice::from_raw_parts(data.byte_add(cam_smooth_offset), CameraFerret::M43_LEN32);
        let m = SnatchMatrix::from(CameraFerret::m43_at_unchecked(mdata));
        if CameraFerret::matrix_matches_shape43(&m.data) {
            g2!(*&mut ferret.snatch_camera_smooth = m);
        }
    }
}
pub(super) fn wants_update_subresource_pre(
    resource: *mut c_void,
    (matched_cam, matched_persp): (bool, bool),
) -> bool {
    if !RenderState::is_render_thread() {
        return false
    }
    let blacklisted = !CameraSearch::with_mut_unchecked(|s| s.visit(resource));
    if blacklisted {
        return false
    }
    (!matched_cam && !FerretResource::has_found_camera() && FerretResource::wants_camera())
        || (!matched_persp
            && !FerretResource::has_found_perspective()
            && FerretResource::wants_perspective())
}
pub(super) fn wants_update_subresource(
    buffer_size: u32,
    region: ops::Range<u32>,
    (_matched_cam, _matched_persp): (bool, bool),
) -> bool {
    if frame_log!(::is_game()) {
        return true
    }
    buffer_size < 0x10000
        && region.len() >= CameraFerret::M4_LEN32 * 4
        && g2!(*&ferret.size_range).contains(&(buffer_size as u16))
}
pub(super) fn update_subresource(
    resource: &Resource,
    buffer: &Buffer,
    data: &[u32],
    _offset: u32,
    (matched_cam, matched_persp): (bool, bool),
) {
    if frame_log!(::is_game()) {
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
            let fcam = g2!(*&ferret.camera);
            let found_cam = if found_persp.is_some() {
                frame_log!(;"persp-shape btw!");
                None
            } else if !fcam.is_empty() {
                fcam.search(data, 2)
            } else {
                None
            };
            if found_cam.is_some() {
                frame_log!(;"cam-match btw!");
            }
        }
        return
    }
    let gran = FerretResource::get_granularity() as usize;
    let wants_cam = !matched_cam && FerretResource::wants_camera() && !FerretResource::has_found_camera();
    let wants_persp =
        !matched_persp && FerretResource::wants_perspective() && !FerretResource::has_found_perspective();
    let mut persp_offset = None;
    let mut cam_offset = None;
    let searchspace = data.get(..0x68).unwrap_or(data);
    if wants_cam {
        let fcam = g2!(*&ferret.camera);
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
                CameraFerret::report_matrix(
                    m,
                    Some((cam_offset, true, accuracy)),
                    resource.as_d3d().as_raw(),
                );
            }
        }
    }
    if wants_persp {
        let fpersp = g2!(*&ferret.perspective);
        let persp_offset = match persp_offset {
            None => fpersp
                .search(searchspace, gran)
                .map(|found| unsafe { found.as_ptr().offset_from(data.as_ptr()) as usize }),
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
                        g2!(*&mut ferret.snatch_camera_smooth = m);
                    }
                }
            }
        }
    }
}

pub struct FerretResource {
    pub display_size: glam::Vec2,
    pub size_range: ops::Range<u16>,
    pub perspective: PerspectiveFerret,
    pub camera: CameraFerret,
    pub granularity: u8,
    #[cfg(feature = "goggles2-project")]
    pub project: super::project::ProjectFerret,
    pub snatch_camera: SnatchMatrix,
    pub found_camera: Option<(usize, usize, bool)>,
    pub found_perspective: Option<(usize, usize)>,
    pub snatch_perspective: SnatchMatrix,
    /// backup via perspective
    pub snatch_camera_smooth: SnatchMatrix,
}
impl FerretResource {
    pub const DEFAULT: Self = Self {
        display_size: glam::Vec2::ZERO,
        perspective: PerspectiveFerret::EMPTY,
        camera: CameraFerret::EMPTY,
        granularity: Self::DEFAULT_GRANULARITY,
        size_range: 0u16..0u16,
        #[cfg(feature = "goggles2-project")]
        project: super::project::ProjectFerret::EMPTY,
        snatch_camera: SnatchMatrix::DEFAULT,
        snatch_camera_smooth: SnatchMatrix::DEFAULT,
        snatch_perspective: SnatchMatrix::DEFAULT,
        found_perspective: None,
        found_camera: None,
    };
    const DEFAULT_GRANULARITY: u8 = 2;
    #[inline(always)]
    pub fn get() -> *mut Self {
        static FERRET: SyncUnsafeCell<FerretResource> = SyncUnsafeCell::new(FerretResource::DEFAULT);
        FERRET.get()
    }
    #[inline]
    pub fn set_display_size(v: glam::Vec2) {
        g2!(*&mut ferret.display_size = v)
    }
    pub fn get_granularity() -> u8 {
        g2!(*&ferret.granularity).max(1)
    }
    #[inline]
    pub fn set_granularity(v: u8) {
        g2!(*&mut ferret.granularity = v)
    }
    pub fn wants_perspective() -> bool {
        g2!(*&ferret.perspective.expected_h).to_bits() != PerspectiveFerret::ZERO32
    }
    #[inline]
    pub fn set_perspective(v: PerspectiveFerret) {
        g2!(*&mut ferret.perspective = v)
    }
    pub fn wants_camera() -> bool {
        !g2!(*&ferret.camera.expected_dir.x).is_infinite()
    }
    #[inline]
    pub fn set_camera(v: CameraFerret) {
        g2!(*&mut ferret.camera = v)
    }
    #[inline]
    pub fn set_size_range(v: ops::Range<u16>) {
        g2!(*&mut ferret.size_range = v)
    }
    #[inline]
    pub fn trip_snatch_camera() {
        g2!(*&mut ferret.snatch_camera = SnatchMatrix::DEFAULT)
    }
    #[inline]
    pub fn snatch_camera() -> SnatchMatrix {
        g2!(*&ferret.snatch_camera)
    }
    pub fn wants_snatch_camera() -> bool {
        g2!(*&ferret.snatch_camera.data.w_axis.w).is_infinite()
    }
    #[inline]
    pub fn clear_camera_found() {
        g2!(*&mut ferret.found_camera = None)
    }
    #[inline]
    pub fn set_camera_found(buf: *mut c_void, off: usize, is_m43: bool) {
        g2!(*&mut ferret.found_camera = Some((buf as usize, off, is_m43)))
    }
    /// TODO: flag instead?
    /// (same for has_found_perspective)
    #[inline]
    pub fn has_found_camera() -> bool {
        Self::found_camera().is_some()
    }
    pub fn found_camera() -> Option<(usize, usize, bool)> {
        g2!(*&ferret.found_camera)
    }
    #[inline]
    pub fn trip_snatch_perspective() {
        g2!(*&mut ferret.snatch_perspective = SnatchMatrix::DEFAULT)
    }
    #[inline]
    pub fn snatch_perspective() -> SnatchMatrix {
        g2!(*&ferret.snatch_perspective)
    }
    pub fn wants_snatch_camera_smooth() -> bool {
        g2!(*&ferret.snatch_camera_smooth.data.w_axis.w).is_infinite()
    }
    #[inline]
    pub fn trip_snatch_camera_smooth() {
        g2!(*&mut ferret.snatch_camera_smooth = SnatchMatrix::DEFAULT)
    }
    #[inline]
    pub fn snatch_camera_smooth() -> SnatchMatrix {
        g2!(*&ferret.snatch_camera_smooth)
    }
    pub fn wants_snatch_perspective() -> bool {
        g2!(*&ferret.snatch_perspective.data.w_axis.w).is_infinite()
    }
    #[inline]
    pub fn clear_perspective_found() {
        g2!(*&mut ferret.found_perspective = None)
    }
    #[inline]
    pub fn set_perspective_found(buf: *mut c_void, off: usize) {
        g2!(*&mut ferret.found_perspective = Some((buf as usize, off)))
    }
    #[inline]
    pub fn has_found_perspective() -> bool {
        Self::found_perspective().is_some()
    }
    pub fn found_perspective() -> Option<(usize, usize)> {
        g2!(*&ferret.found_perspective)
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
        Self { expected_w, expected_h }
    }
    const ZERO32: u32 = 0.0f32.to_bits();
    const ONE32: u32 = 1.0f32.to_bits();
    const NEG32: u32 = (-1.0f32).to_bits();
    pub const fn is_empty(&self) -> bool {
        self.expected_h.to_bits() == Self::ZERO32
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
                    if v == Self::ZERO32 || matches!(f32::from_bits(v), -0.00001..=0.00001) {
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
        if (w - self.expected_w).abs() > Self::M4_PERSP_EPSILON {
            return false
        }

        true
    }
    pub const COMPACT_LEN32: usize = 4;

    unsafe fn report_matrix(m: &[u32], offset: Option<usize>, buf_dest: *mut c_void) {
        if FerretResource::wants_snatch_perspective() {
            let m = CameraFerret::m4_at_unchecked(m);
            g2!(*&mut ferret.snatch_perspective = m.into());
            if let Some(offset) = offset {
                if !FerretResource::has_found_perspective() {
                    log::info!("found new perspective {buf_dest:p}@{offset:#x}");
                    FerretResource::set_perspective_found(buf_dest, offset);
                }
            }
        }
    }
    fn find_anywhere(&self, data: &[u32]) -> Option<ops::RangeInclusive<usize>> {
        if self.is_empty() {
            return None
        }
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
    pub const EMPTY: Self = Self {
        expected_dir: glam::Vec3::INFINITY,
        expected_pos: glam::Vec3::INFINITY,
        expected_eye_vector_z: f32::INFINITY,
    };
    pub fn new(pos: Point3<LocalSpace>, dir: Vector3<LocalSpace>, up: Vector3<LocalSpace>) -> Self {
        let mut ferret = Self::EMPTY;
        ferret.set_expected(pos, dir, up);
        ferret
    }
    pub(crate) fn with_game(
        pos: Point3<GameSpace>,
        dir: Vector3<GameSpace>,
        _up: Vector3<GameSpace>,
        expected_eye_vector_z: f32,
    ) -> Self {
        Self {
            expected_dir: dir.to_raw(),
            expected_pos: pos.to_raw(),
            expected_eye_vector_z,
        }
    }
    const ZERO32: u32 = 0.0f32.to_bits();
    const NEGZERO32: u32 = (-0.0f32).to_bits();
    const ONE32: u32 = 1.0f32.to_bits();
    const NEG32: u32 = (-1.0f32).to_bits();
    pub const fn is_empty(&self) -> bool {
        self.expected_dir.x.is_infinite()
    }

    pub fn set_expected(
        &mut self,
        pos: Point3<LocalSpace>,
        dir: Vector3<LocalSpace>,
        _up: Vector3<LocalSpace>,
    ) {
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
                    if v == Self::ZERO32 || matches!(f32::from_bits(v), -0.00001..=0.00001) {
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
        let look_elems = unsafe { &*(data.as_ptr() as *const u32 as *const [f32; Self::M4_LEN32]) };
        let look = glam::Mat4::from_cols_array(look_elems);
        // TODO: only check z, then move on to decompose below
        let look_dir = &look.z_axis;
        #[cfg(todo)]
        for (&d, exp) in dir.to_array().iter().zip(self.expected_dir.to_array()) {
            // TODO: check signs?
        }
        if (look_dir.truncate().normalize().y.abs() - self.expected_dir.y.abs()).abs()
            > Self::M4_CAM_Z_EPSILON
        {
            frame_log!("\tcamdir.y mismatch of {look_dir:?}");
            print_ferret(data, 0, Self::M4_LEN32);
            return false
        }

        let (look, look_eye) = coords::decompose_look_to32_rows(look);
        let (eye, dir, _up) = coords::decompose_look32_rows(look, look_eye);
        #[cfg(todo)]
        let (exp, found) = (self.expected_dir.xz(), dir.xz());
        let (exp, found) = (glam::Vec3A::from(self.expected_dir), dir);
        if !exp.abs_diff_eq(found, Self::M4_CAM_DIR_EPSILON) {
            frame_log!("\tcamdir mismatch of {dir:?}");
            print_ferret(data, 0, Self::M4_LEN32);

            return false
        }
        if !self.expected_pos.x.is_infinite() {
            // TODO: check up vector too?
            if !self
                .expected_pos
                .abs_diff_eq(eye.into(), Self::M4_CAM_POS_EPSILON)
            {
                frame_log!("\tcampos mismatch of {eye:?}, expected: {:?}", self.expected_pos);
                print_ferret(data, 0, Self::M4_LEN32);
                return false
            }
        }

        #[cfg(todo)]
        let check_shuffle = data.get(Self::M4_LEN32..).map(|d| Self::check_shuffle(d));
        #[cfg(todo)]
        if check_shuffle != Some(true) {
            frame_log!("\tshuffle failed!");
            print_ferret(data, 0, Self::M4_LEN32);
            return false
        }

        true
    }
    unsafe fn m4_at_unchecked(data: &[u32]) -> Mat4 {
        let look_elems = unsafe { &*(data.as_ptr() as *const u32 as *const [f32; Self::M4_LEN32]) };
        Mat4::from_cols_array(look_elems)
    }
    unsafe fn m43_at_unchecked(data: &[u32]) -> Mat4 {
        let look_elems = unsafe { &*(data.as_ptr() as *const u32 as *const [f32; Self::M43_LEN32]) };
        Mat43::from_cols_array_ref(look_elems).to_mat4().into()
    }
    fn eye_matches(&self, look: Mat4, check_dir: bool) -> bool {
        let (look, look_eye) = coords::decompose_look_to32_rows(look);
        let (eye, dir, _up) = coords::decompose_look32_rows(look, look_eye);
        if check_dir {
            #[cfg(todo)]
            let (exp, found) = (self.expected_dir.xz(), dir.xz());
            let (exp, found) = (glam::Vec3A::from(self.expected_dir), dir);
            if !exp.abs_diff_eq(found, Self::M4_CAM_DIR_EPSILON) {
                return false
            }
            // TODO: check up vector too?
        }
        if !self
            .expected_pos
            .abs_diff_eq(eye.into(), Self::M4_CAM_POS_EPSILON)
        {
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
    fn dir_up_matches(data: &[u32]) -> bool {
        let Some(z) = data.get(Self::M43_DIR_OFFSET - 2) else { return false };
        let z = f32::from_bits(*z);
        Self::up_matches(z)
    }
    #[inline]
    fn up_matches(z: f32) -> bool {
        matches!(z, -1.0..-0.953)
    }
    fn dir_matches(&self, data: &[u32], check_z: bool) -> bool {
        let z_axis = match data.get(Self::M43_DIR_OFFSET..) {
            Some(&[x, y, z, w, ..]) => glam::Vec4::new(
                f32::from_bits(x),
                f32::from_bits(y),
                f32::from_bits(z),
                f32::from_bits(w),
            ),
            _ => return false,
        };
        let exp = glam::Vec3A::from(self.expected_dir);
        if !glam::Vec3A::from_vec4(z_axis).abs_diff_eq(exp, Self::M4_CAM_DIR_EPSILON) {
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
        if !Self::dir_up_matches(unsafe { data.get_unchecked(off..) }) {
            return None
        }
        if !require_persp_suffix && Self::is_w(suffix) {
        } else if !require_persp_suffix || !PerspectiveFerret::matches_shape(suffix) {
            return Some(Err(()))
        }
        let m43 = unsafe {
            let found = data.get_unchecked(off..);
            Self::m43_at_unchecked(found)
        };
        Some(self.eye_matches(m43, false).then_some(off).ok_or(()))
    }
    pub fn find_m43(&self, data: &[u32], require_persp_suffix: bool) -> Option<usize> {
        search_ferret(
            data,
            4,
            |data| {
                if !self.dir_x_matches(data) || !Self::dir_up_matches(data) {
                    return false
                }
                let matches = self.dir_matches(data, true);
                if !matches {
                    return false
                }
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
        )
        .map(|found| unsafe { found.as_ptr().offset_from(data.as_ptr()) as usize })
    }
    fn is_w(data: &[u32]) -> bool {
        let axis = match data {
            &[x, y, z, w, ..] => glam::Vec4::new(
                f32::from_bits(x),
                f32::from_bits(y),
                f32::from_bits(z),
                f32::from_bits(w),
            ),
            _ => return false,
        };
        axis == glam::Vec4::W
    }
    const M43_DIR_OFFSET: usize = 4 * 2;
    pub fn bitfind_m43_offset(&self, data: &[u32]) -> Option<usize> {
        let searchspace = data.get(Self::M43_DIR_OFFSET..).unwrap_or(&[]);
        let off = self.bitfind_dir3a_offset(searchspace)?;

        let eye_z = searchspace.get(off + 3).map(|eye_z| {
            (f32::from_bits(*eye_z) - self.expected_eye_vector_z).abs() <= Self::M4_CAM_POS_EPSILON
        });
        if !eye_z.unwrap_or(false) {
            return None
        }

        Some(off)
    }
    pub fn bitfind_dir3a_offset(&self, data: &[u32]) -> Option<usize> {
        let exp_x = self.expected_dir.x.to_bits();
        // XXX: could allow trailing 3 but unlikely to not be 128bit aligned...
        let mut off = 0usize;
        while off + 4 <= data.len() {
            let [x, y, z] = unsafe { &*(data.as_ptr().add(off) as *const [u32; 3]) };
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
        if FerretResource::wants_snatch_camera() {
            g2!(*&mut ferret.snatch_camera = m);
        }
        if let Some((offset, is_m43, accuracy)) = offset {
            let accuracy =
                ((dir.truncate() - accuracy).abs().element_sum() * 100.0).min(u8::MAX as f32) as u8;
            CameraSearch::with_mut_unchecked(|s| {
                s.report(buf_dest, CameraMatch {
                    #[cfg(feature = "goggles2-project")]
                    drawn: FerretResource::project_report_drawn(),
                    is_m43,
                    offset: offset as _,
                    accuracy,
                })
            });
        }
    }
    /// row-major btw
    const SHUFFLE_EXPECTED_M4: glam::Mat4 =
        glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Y, glam::Vec4::X, glam::Vec4::Y);
    const SHUFFLE_EXPECTED: [f32; 16] = Self::SHUFFLE_EXPECTED_M4.to_cols_array();
    /// :<
    const SHUFFLE_EXPECTED2: [f32; 8] = [
        Self::SHUFFLE_EXPECTED[0],
        Self::SHUFFLE_EXPECTED[1],
        Self::SHUFFLE_EXPECTED[2],
        Self::SHUFFLE_EXPECTED[3],
        Self::SHUFFLE_EXPECTED[4],
        Self::SHUFFLE_EXPECTED[5],
        Self::SHUFFLE_EXPECTED[6],
        Self::SHUFFLE_EXPECTED[7],
        //0.0, 0.0, 0.0, 1.0,
    ];
    fn check_shuffle(data: &[u32]) -> bool {
        let dataf = data
            .iter()
            .map(|d| f32::from_bits(*d))
            .chain(core::iter::repeat(f32::MAX));
        if dataf
            .clone()
            .zip(Self::SHUFFLE_EXPECTED.iter())
            .all(|(d, ex)| d == *ex)
        {
            return true
        }
        if dataf.zip(Self::SHUFFLE_EXPECTED2.iter()).all(|(d, ex)| d == *ex) {
            return true
        }
        false
    }
    fn find_anywhere(&self, data: &[u32]) -> Option<ops::RangeInclusive<usize>> {
        if self.is_empty() {
            return None
        }
        find_all(data, &self.expected_dir.to_array(), Self::M4_ZERO_EPSILON)
    }

    fn matrix_matches_shape43(look: &glam::Mat4) -> bool {
        #[cfg(todo)]
        let (side, dir) = (&look.x_axis, &look.z_axis);
        let up = &look.z_axis;
        Self::up_matches(up.z)
    }
}
fn find_all(data: &[u32], needle: &[f32], eps: f32) -> Option<ops::RangeInclusive<usize>> {
    #[cfg(todo)]
    if needle.is_empty() {
        return None
    }
    let mut found = Vec::new();
    'data: for (i, &d) in data.iter().enumerate() {
        if matches!(d, CameraFerret::ZERO32 | CameraFerret::NEGZERO32) {
            continue
        }
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
            if *fi != usize::MAX {
                continue
            }
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
        found
            .clone()
            .min()
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

pub trait FerretPattern {
    fn search<'d>(&self, data: &'d [u32], granularity: usize) -> Option<&'d [u32]>;
}

fn print_ferret(data: &[u32], offset: usize, len: usize) {
    if !frame_log!(::is_game()) {
        return
    }
    let displen = ((len + 3) / 4).max(16);
    let prior = (offset > 0)
        .then_some(offset.saturating_sub(16))
        .map(|off| {
            core::iter::once_with(move || {
                frame_log!(;"preceding 16:");
                unsafe { data.get_unchecked(off..offset) }
            })
        })
        .into_iter()
        .flatten();
    let found = unsafe { data.get_unchecked(offset..) };
    let chunks = found.chunks(4).take(displen).chain(prior);
    for chunk in chunks {
        use core::fmt::Write;
        let mut line = String::new();
        for &v in chunk {
            let _ = write!(&mut line, "  {:4.09}", f32::from_bits(v));
        }
        frame_log!(;"\t::{line}");
    }
}
fn search_ferret<F, M>(
    data: &[u32],
    granularity: usize,
    mut filter: F,
    mut matcher: M,
    matchlen: Option<usize>,
) -> Option<&[u32]>
where
    F: FnMut(&[u32]) -> bool,
    M: FnMut(&[u32]) -> Option<usize>,
{
    let mut haystack = data;
    let mut pmatch = None;
    while !haystack.is_empty() {
        let next = haystack.get(granularity..).unwrap_or(&[]);
        let search = mem::replace(&mut haystack, next);
        if !filter(search) {
            continue
        }
        let offset = unsafe { search.as_ptr().offset_from(data.as_ptr()) } as usize;
        //frame_log!(;"- pre-match @{offset:#x}?");
        let mut matchlen = matchlen;
        if let Some(len) = matcher(search) {
            frame_log!(;"- actual match offset={offset:#x}");
            print_ferret(data, offset, len);
            matchlen = Some(len);
            pmatch = Some(unsafe { search.get_unchecked(..len) });
            #[cfg(todo)]
            break;
        }
        if let Some(amt) = matchlen {
            haystack = search.get(amt.max(granularity)..).unwrap_or(&[]);
        }
    }
    pmatch
}

#[derive(Debug, Copy, Clone)]
#[repr(transparent)]
pub struct SnatchMatrix {
    pub data: glam::Mat4,
}
impl SnatchMatrix {
    pub const DEFAULT: Self = Self {
        data: glam::Mat4::from_cols(glam::Vec4::X, glam::Vec4::Y, glam::Vec4::Z, glam::Vec4::INFINITY),
    };
    pub const fn new(data: glam::Mat4) -> Self {
        Self { data }
    }
    pub fn is_empty(&self) -> bool {
        self.data.w_axis.w.is_infinite()
    }

    pub fn get_as_look(&self) -> (Point3<LocalSpace>, Vector3<LocalSpace>) {
        let (look, look_eye) = coords::decompose_look_to32_rows(self.data);
        let (eye, dir, _up) = coords::decompose_look32_rows(look, look_eye);
        (
            GameSpace::to_local(eye.into()).into(),
            GameSpace::norm_to_local(dir.into()).into(),
        )
    }
    pub fn get_look_up(&self) -> glamour::Vector3<LocalSpace> {
        GameSpace::norm_to_local(self.data.y_axis.truncate().into())
    }
    pub fn get_look_side(&self) -> glamour::Vector3<LocalSpace> {
        GameSpace::norm_to_local(self.data.x_axis.truncate().into())
    }
    pub fn get_ferret_look(&self) -> CameraFerret {
        let (eye, dir, up) = {
            let (look, look_eye) = coords::decompose_look_to32_rows(self.data);
            coords::decompose_look32_rows(look, look_eye)
        };
        #[cfg(todo)]
        let up = GameSpace::UP;
        CameraFerret::with_game(eye.into(), dir.into(), up.into(), self.data.z_axis.w)
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
        PerspectiveFerret::with_parts(w, h)
    }
}
impl From<glam::Mat4> for SnatchMatrix {
    fn from(data: glam::Mat4) -> Self {
        Self::new(data)
    }
}
impl Default for SnatchMatrix {
    fn default() -> Self {
        Self::DEFAULT
    }
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
        self.matches
            .iter()
            .filter(|&(k, _)| !self.blacklist.contains(k))
            .map(|(k, m)| (*k as *mut c_void, *m))
            .min_by_key(|(_, m)| *m)
    }
    pub fn with_mut_unchecked<R, F: FnOnce(&mut Self) -> R>(f: F) -> R {
        use sync_unsafe_cell::SyncUnsafeCell;
        static SEARCH: SyncUnsafeCell<CameraSearch> = SyncUnsafeCell::new(CameraSearch::DEFAULT);
        let search = SEARCH.get();
        unsafe { f(&mut *search) }
    }
    #[inline]
    pub fn seen_frame(&self) -> bool {
        !self.seen.is_empty()
    }
}

macro_rules! g2 {
    (*&mut ferret$(.$field:ident)+ = $v:expr$(;)?) => {
        match $v {
            #[allow(unused_unsafe)]
            ferret_v_ => unsafe {
                ::core::ptr::write(&raw mut (*$crate::space::goggles::FerretResource::get())$(.$field)+, ferret_v_)
            },
        }
    };
    (&raw mut ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                &raw mut (*$crate::space::goggles::FerretResource::get())$(.$field)+
            },
        }
    };
    (&raw const ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                &raw const (*$crate::space::goggles::FerretResource::get())$(.$field)+
            },
        }
    };
    (*&ferret$(.$field:ident)+) => {
        match () {
            #[allow(unused_unsafe)]
            () => unsafe {
                // read_volatile shouldn't matter...
                ::core::ptr::read(&raw const (*$crate::space::goggles::FerretResource::get())$(.$field)+)
            },
        }
    };
}
pub(crate) use g2;
