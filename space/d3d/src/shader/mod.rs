use {
    arcffi::cstr::{CStrBox, CStrPtr, CStrRef},
    crate::prelude::*,
    std::{
        collections::{BTreeMap, HashMap},
        ffi::{CStr, CString},
    },
};
pub use {
    crate::d3d::{
        Fxc::D3DCompile,
        D3D_SHADER_MACRO, ID3DInclude,
    },
    self::target::{ShaderTarget, ShaderKind},
};

mod target;

pub fn compile(
    filename: &CStr,
    source: &[u8],
    target: ShaderTarget,
    entry_point: &CStr,
    defines: &[ShaderDefinition],
    includes: Option<&ID3DInclude>,
    flags1: u32,
    flags2: u32,
) -> anyhow::Result<(Blob, CString)> {
    let error_context = || anyhow!("D3DCompile({target:?}, {}:{})", CStrRef::with_cstr(filename), CStrRef::with_cstr(entry_point));
    let entry_point = PCSTR(entry_point.as_ptr() as *const _);
    let filename = PCSTR(filename.as_ptr() as *const _);
    let defines = match defines {
        defs if defs.is_empty() => None,
        defines => Some(ShaderDefinition::slice_as_d3d_macros(defines)),
    };
    let target_name = PCSTR(target.c_name().as_ptr() as *const _);
    let mut out: Option<ID3DBlob> = None;
    let mut messages: Option<ID3DBlob> = None;
    let res = unsafe {
        D3DCompile(
            source.as_ptr() as *const _,
            source.len(),
            filename,
            defines.map(|d| d.as_ptr() as *const _),
            includes,
            entry_point,
            target_name,
            flags1,
            flags2,
            &mut out,
            Some(&mut messages),
        )
    }.map_err(anyhow::Error::from)
    .with_context(error_context)
    .and_then(|()| out.ok_or_else(|| anyhow!("failed to produce {} shader bytecode pointer", target)))
    .map(Blob::with_blob);
    let messages = messages.map(Blob::with_blob);
    let messages = messages.as_ref()
        .map(|m| unsafe { CStr::from_bytes_with_nul_unchecked(m.as_bytes()) });

    match (res, messages) {
        (Err(e), None) => Err(e),
        (Err(e), Some(messages)) =>
            Err(anyhow::Error::msg(messages.to_string_lossy().into_owned()).context(e)),
        // TODO: skip this allocation
        (Ok(b), messages) =>
            Ok((b, messages.map(|m| m.to_owned()).unwrap_or_default())),
    }
}

#[repr(C)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShaderDefinition {
    pub name: CStrBox,
    pub definition: CStrBox,
}

impl ShaderDefinition {
    pub fn as_ptr_tuple(&self) -> (CStrPtr<'_>, CStrPtr<'_>) {
        (self.name.as_c_ptr(), self.definition.as_c_ptr())
    }

    #[inline]
    pub const unsafe fn from_d3d_macro(defs: &D3D_SHADER_MACRO) -> &Self {
        unsafe {
            mem::transmute(defs)
        }
    }

    #[inline]
    pub const fn as_d3d_macro(&self) -> &D3D_SHADER_MACRO {
        unsafe {
            mem::transmute(self)
        }
    }

    #[inline]
    pub const fn slice_as_d3d_macros(defs: &[Self]) -> &[D3D_SHADER_MACRO] {
        unsafe {
            mem::transmute(defs)
        }
    }

    pub fn try_from_str<N, D>(n: N, d: D) -> Result<Self, NulError> where
        N: Into<Vec<u8>>,
        D: Into<Vec<u8>>,
    {
        let n = CString::new(n)?;
        let d = CString::new(d)?;
        Ok(Self {
            name: CStrBox::with_cstring(n),
            definition: CStrBox::with_cstring(d),
        })
    }
}

impl<N, D> From<(N, D)> for ShaderDefinition where
    N: Into<CStrBox>,
    D: Into<CStrBox>,
{
    fn from((name, def): (N, D)) -> Self {
        Self {
            name: name.into(),
            definition: def.into(),
        }
    }
}

impl Into<(String, String)> for ShaderDefinition {
    fn into(self) -> (String, String) {
        let Self { name, definition } = self;
        (
            name.into_cstring().to_string_lossy().into_owned(),
            definition.into_cstring().to_string_lossy().into_owned(),
        )
    }
}

impl Into<(CStrBox, CStrBox)> for ShaderDefinition {
    fn into(self) -> (CStrBox, CStrBox) {
        let Self { name, definition } = self;
        (
            name,
            definition,
        )
    }
}

impl Into<(CString, CString)> for ShaderDefinition {
    fn into(self) -> (CString, CString) {
        let Self { name, definition } = self;
        (
            name.into_cstring(),
            definition.into_cstring(),
        )
    }
}

#[derive(Debug, Clone, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(into = "std::collections::BTreeMap<String, String>"))]
pub struct ShaderDefinitions {
    pub defs: Vec<ShaderDefinition>,
}

impl ShaderDefinitions {
    pub const fn with_defs(defs: Vec<ShaderDefinition>) -> Self {
        Self {
            defs,
        }
    }

    pub fn new<D: Into<Vec<ShaderDefinition>>>(defs: D) -> Self {
        let defs = defs.into();
        Self {
            defs,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    #[inline]
    pub fn defs(&self) -> &[ShaderDefinition] {
        &self.defs
    }

    #[inline]
    pub fn as_d3d_macros(&self) -> &[D3D_SHADER_MACRO] {
        ShaderDefinition::slice_as_d3d_macros(&self.defs)
    }

    pub fn try_from_str<N, D, I>(defs: I) -> Result<Self, NulError> where
        N: Into<Vec<u8>>,
        D: Into<Vec<u8>>,
        I: IntoIterator<Item = (N, D)>,
    {
        defs.into_iter()
            .map(|(k, v)| ShaderDefinition::try_from_str(k, v))
            .collect()
    }
}

impl AsRef<[ShaderDefinition]> for ShaderDefinitions {
    fn as_ref(&self) -> &[ShaderDefinition] {
        self.defs()
    }
}

impl AsRef<[D3D_SHADER_MACRO]> for ShaderDefinitions {
    fn as_ref(&self) -> &[D3D_SHADER_MACRO] {
        self.as_d3d_macros()
    }
}

impl<D> FromIterator<D> for ShaderDefinitions where
    D: Into<ShaderDefinition>
{
    fn from_iter<T: IntoIterator<Item = D>>(iter: T) -> Self {
        let iter = iter.into_iter()
            .map(Into::into);
        Self::with_defs(iter.collect())
    }
}

impl<D> Extend<D> for ShaderDefinitions where
    D: Into<ShaderDefinition>
{
    fn extend<T: IntoIterator<Item = D>>(&mut self, iter: T) {
        let iter = iter.into_iter()
            .map(Into::into);
        self.defs.extend(iter)
    }
}

impl<N, D> From<BTreeMap<N, D>> for ShaderDefinitions where
    (N, D): Into<ShaderDefinition>,
{
    fn from(value: BTreeMap<N, D>) -> Self {
        value.into_iter().collect()
    }
}

impl<N, D> From<HashMap<N, D>> for ShaderDefinitions where
    (N, D): Into<ShaderDefinition>,
{
    fn from(value: HashMap<N, D>) -> Self {
        value.into_iter().collect()
    }
}

impl Into<BTreeMap<String, String>> for ShaderDefinitions {
    fn into(self) -> BTreeMap<String, String> {
        self.defs.into_iter()
            .map(Into::into)
            .collect()
    }
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ShaderDefinitions {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where
        D: serde::Deserializer<'de>,
    {
        let defs: BTreeMap<String, String> = serde::Deserialize::deserialize(deserializer)?;
        Self::try_from_str(defs)
            .map_err(|_| serde::de::Error::invalid_value(
                serde::de::Unexpected::Str("NUL byte"),
                &"C-compatible string",
            ))
    }
}

impl_d3d! {
    unsafe impl D3dInterfacePtr for ID3DInclude;
}
