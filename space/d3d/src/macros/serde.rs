#[cfg(feature = "windows")]
pub use crate::buffer::dxgi::serde_imp::format as dxgi_format;
#[cfg(feature = "dx11")]
pub mod dx11 {
    pub use crate::dx11::serde_imp::{input_classification, input_layout_element};
}

pub mod cstr_box {
    use {
        arcffi::cstr::{CStrBox, CStrRef},
        serde::{Deserialize, Deserializer, Serialize, Serializer},
        std::ffi::CString,
    };

    pub mod cow {
        use {arcffi::cstr::CStrRef, serde::Deserializer, std::borrow::Cow};

        pub use super::{is_empty, serialize};

        pub fn deserialize<'de, D: Deserializer<'de>>(
            deserializer: D,
        ) -> Result<Cow<'de, CStrRef>, D::Error> {
            // XXX: borrowed bytes are possible but unlikely
            super::deserialize(deserializer)
                .map(|s| s.into_cstring())
                .map(Cow::Owned)
        }

        pub fn empty<'a>() -> Cow<'a, CStrRef> {
            Cow::Borrowed(CStrRef::EMPTY)
        }
    }

    pub fn empty() -> CStrBox {
        CStrBox::with_cstring(CString::default())
    }

    pub fn is_empty(s: &CStrRef) -> bool {
        s.is_empty()
    }

    pub fn serialize<S: Serializer>(s: &CStrRef, serializer: S) -> Result<S::Ok, S::Error> {
        // TODO: just impl AsRef<CStr>... (cargo update etc .-.)
        let s = s.to_c_str();
        match s.to_str() {
            Ok(s) => s.serialize(serializer),
            Err(..) => s.to_bytes_with_nul().serialize(serializer),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<CStrBox, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum CStrBoxDe {
            Bytes(Vec<u8>),
            Str(String),
        }
        CStrBoxDe::deserialize(deserializer)
            .map(|c| match c {
                CStrBoxDe::Bytes(v) if v.last().copied() == Some(0u8) => Err(v),
                CStrBoxDe::Bytes(v) => Ok(v),
                CStrBoxDe::Str(v) => Ok(v.into_bytes()),
            })
            .and_then(|c| match c {
                Ok(s) => CString::new(s).map_err(|e| serde::de::Error::custom(e)),
                Err(b) => CString::from_vec_with_nul(b).map_err(|e| serde::de::Error::custom(e)),
            })
            .map(CStrBox::new)
    }
}
