use {
    crate::script::{user::ScriptUserStr, Result},
    anyhow::Context,
    std::borrow::Cow,
};

pub trait ScriptApiVersion {
    #[inline]
    fn blish_pathing_compat_version(&self) -> Cow<'_, str> {
        Cow::Borrowed(<dyn ScriptApiVersion>::BLISH_PATHING_COMPAT_STR)
    }
    #[inline]
    fn taimi_api_version(&self) -> Cow<'_, str> {
        Cow::Borrowed(<dyn ScriptApiVersion>::TAIMI_API_VER)
    }
    /// actual package version goes here (not the subcrate!)
    fn taimi_version(&self) -> Cow<'_, str>;
}
impl dyn ScriptApiVersion {
    /// semver metadata to tell us apart
    pub const SEMVER_METADATA_SUFFIX: &'static str = "+taimi";
    /// a non-existent version prior to the recent 1.12 release
    ///
    /// iirc 1.12 had no lua changes, but 1.11.3ish added trail APIs.
    /// (expect to have to bump this soon if trails receive more updates/attention)
    pub const BLISH_PATHING_COMPAT_VER: &'static str = "1.11.999";
    /// format!("{BLISH_PATHING_COMPAT_VER}{SEMVER_METADATA_SUFFIX}")
    pub const BLISH_PATHING_COMPAT_STR: &'static str = "1.11.999+taimi";
    /// versioning scheme for our API implementation,
    /// distinct from the [addon version](ScriptApiVersion::taimi_version)
    pub const TAIMI_API_VER: &'static str = "0.0.1";

    pub fn version_mmp_trunc(s: &[u8]) -> &str {
        let valid_len = s
            .iter()
            .position(|&b| !matches!(b, b'0'..=b'9' | b'.'))
            .unwrap_or(s.len());
        unsafe { str::from_utf8_unchecked(s.get_unchecked(..valid_len)) }
    }
    pub fn version_mmp(s: &[u8]) -> Option<(&str, &str, &str)> {
        let s = Self::version_mmp_trunc(s);
        let mut segs = s.as_bytes().split(|&c| c == b'.');
        let major = segs.next()?;
        let minor = segs.next()?;
        let patch = segs.next()?;
        match segs.next() {
            Some(..) => None,
            None => Some(unsafe {
                (
                    str::from_utf8_unchecked(major),
                    str::from_utf8_unchecked(minor),
                    str::from_utf8_unchecked(patch),
                )
            }),
        }
    }
    /// common impl for [ScriptApiVersionString::version_scrub_extra]
    pub fn version_str_scrub_extra(s: Cow<'_, str>) -> Cow<'_, str> {
        let valid_len = Self::version_mmp_trunc(s[..].as_bytes()).len();
        match s {
            Cow::Borrowed(s) => Cow::Borrowed(unsafe { s.get_unchecked(..valid_len) }),
            #[cfg(todo = "unnecessary")]
            Cow::Owned(mut s) => unsafe {
                s.as_mut_vec().set_len(valid_len);
                Cow::Owned(s)
            },
            Cow::Owned(mut s) => {
                s.truncate(valid_len);
                Cow::Owned(s)
            },
        }
    }
    pub fn is_version_str_at_least(s: &str, req: &str) -> Result<bool> {
        if Self::version_str_is_taimi_compat(req) {
            return Ok(false)
        }

        let req_mmp = Self::version_mmp(req[..].as_bytes())
            .with_context(|| format!("unsupported version request {req:?}"))?;
        let (req_major, req_minor, req_patch) = req_mmp;
        let req_major = req_major.trim_start_matches('0');
        let req_minor = req_minor.trim_start_matches('0');
        let req_patch = req_patch.trim_start_matches('0');

        let (major, minor, patch) = Self::version_mmp(s[..].as_bytes())
            .with_context(|| format!("unsupported version code {s:?}"))?;

        if req_major.len() > major.len() || req_major.as_bytes() > major.as_bytes() {
            return Ok(false)
        }
        if req_minor.len() > minor.len() || req_minor.as_bytes() > minor.as_bytes() {
            return Ok(false)
        }
        if req_patch.len() > patch.len() || req_patch.as_bytes() > patch.as_bytes() {
            return Ok(false)
        }

        let trailing = unsafe {
            let mmp_len = req_patch
                .as_ptr()
                .add(req_patch.len())
                .offset_from_unsigned(req.as_ptr());
            req.get_unchecked(mmp_len..)
        };
        if !trailing.is_empty() {
            log::info!("TODO: {req:?} requested prerelease or trailing info");
        }

        Ok(true)
    }
    /// if script feeds our own version back at us, reject it
    /// (TODO: check if doing do succeeds on blishhud pathing or if it doesn't like metadata, uses constraint queries, or needs changing to a hyphen to signify prerelease, etc)
    #[inline]
    pub fn version_str_is_taimi_compat(s: &str) -> bool {
        s.ends_with(Self::SEMVER_METADATA_SUFFIX)
    }
}
/// semver parsing etc...
pub trait ScriptApiVersionString {
    fn version_as_str(&self) -> Cow<'_, str>;

    fn is_version_at_least<S: ScriptUserStr>(&self, version: S) -> Result<bool> {
        version.with_str(|req| <dyn ScriptApiVersion>::is_version_str_at_least(&self.version_as_str(), req))
    }

    /// trim metadata and prerelease info
    #[inline]
    fn version_scrub_extra(&self) -> Cow<'_, str> {
        <dyn ScriptApiVersion>::version_str_scrub_extra(self.version_as_str())
    }
}
impl ScriptApiVersionString for str {
    #[inline]
    fn version_as_str(&self) -> Cow<'_, str> {
        Cow::Borrowed(self)
    }
    #[inline]
    fn version_scrub_extra(&self) -> Cow<'_, str> {
        Cow::Borrowed(<dyn ScriptApiVersion>::version_mmp_trunc(self.as_bytes()))
    }
}
impl ScriptApiVersionString for String {
    #[inline]
    fn version_as_str(&self) -> Cow<'_, str> {
        ScriptApiVersionString::version_as_str(&self[..])
    }
    #[inline]
    fn version_scrub_extra(&self) -> Cow<'_, str> {
        ScriptApiVersionString::version_scrub_extra(&self[..])
    }
}
impl ScriptApiVersionString for Cow<'_, str> {
    #[inline]
    fn version_as_str(&self) -> Cow<'_, str> {
        ScriptApiVersionString::version_as_str(&self[..])
    }
    #[inline]
    fn version_scrub_extra(&self) -> Cow<'_, str> {
        ScriptApiVersionString::version_scrub_extra(&self[..])
    }
}
