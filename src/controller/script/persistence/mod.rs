use {
    crate::settings::{
        source::data::{self, KvKey, KvValue},
        SettingsLock,
    },
    std::{collections::hash_map, fmt, mem, sync::Arc},
    taimi_hoard::lazyfmt,
    taimi_pack::script::{pathing::ScriptApiStorage, user::ScriptUserStr},
};

/// XXX: never use this from an async context, since tokio rwlock...
#[derive(Debug, Clone)]
pub struct ScriptHostPersistence {
    pub owner: Arc<str>,
    settings: SettingsLock,
}
impl ScriptHostPersistence {
    pub fn with_owner_id<I>(id: I, settings: SettingsLock) -> Self
    where
        I: Into<Arc<str>>,
    {
        Self { owner: id.into(), settings }
    }
    /// TODO: consider escaping chars or base64 encoding (please not json) etc
    #[cfg(feature = "paths")]
    pub fn id_for_pack(file_id: impl fmt::Display, root_ns: impl fmt::Display) -> Arc<str> {
        Arc::from(Self::id_for_pack_fmt(file_id, root_ns).to_string())
    }
    #[cfg(feature = "paths")]
    pub fn id_for_pack_fmt<'a>(
        file_id: impl fmt::Display + 'a,
        root_ns: impl fmt::Display + 'a,
    ) -> impl fmt::Display + 'a {
        let prefix = Self::id_prefix_for_pack(file_id);
        lazyfmt::fmt_args!(move "{prefix}{root_ns}..|")
    }
    #[cfg(feature = "paths")]
    pub fn id_prefix_for_pack<'a>(file_id: impl fmt::Display + 'a) -> impl fmt::Display + 'a {
        lazyfmt::fmt_args!(move "@{file_id}|")
    }
    pub fn full_key(&self, subresource: impl fmt::Display, namespace: Option<impl fmt::Display>) -> String {
        let ns = lazyfmt::or_empty(
            namespace
                .as_ref()
                .map(|ns| lazyfmt::fmt_fn(move |f| write!(f, "/{ns}/"))),
        );
        format!("{}{ns}{subresource}", &self.owner[..])
    }
    fn with_entry_ref<F, R>(&self, key: &str, f: F) -> Option<R>
    where
        F: FnOnce(&KvValue) -> R,
    {
        let settings = self.settings.blocking_read();
        settings.data_storage.source_kv.get(key).map(f)
    }
    fn with_entry_mut<K, F, R>(&self, key: K, f: F) -> R
    where
        K: Into<KvKey>,
        F: FnOnce(hash_map::Entry<KvKey, KvValue>, &mut bool) -> R,
    {
        let mut settings = self.settings.blocking_write();
        let entry = settings.data_storage.source_kv.entry(key.into());
        let mut dirty = false;
        let res = f(entry, &mut dirty);
        if dirty {
            settings.mark_dirty();
        }
        res
    }
}
impl ScriptApiStorage for ScriptHostPersistence {
    fn remove_key<K, N>(&self, key: K, namespace: Option<N>) -> taimi_pack::script::Result<()>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        let ns = namespace.map(|ns| ns.clone_to_string());
        let key = key.with_str(|k| self.full_key(k, ns));
        let _removed = self.with_entry_mut(key, |entry, dirty| match entry {
            hash_map::Entry::Occupied(e) => {
                e.remove();
                *dirty = true;
                true
            },
            hash_map::Entry::Vacant(..) => false,
        });
        Ok(())
    }

    fn get_string<K, N>(&self, key: K, namespace: Option<N>) -> taimi_pack::script::Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
    {
        let ns = namespace.map(|ns| ns.clone_to_string());
        let key = key.with_str(|k| self.full_key(k, ns));
        Ok(self
            .with_entry_ref(&key, |v| data::value_as_str(v).map(|s| s.into_owned()))
            .flatten())
    }

    fn insert_string<K, N, V>(
        &self,
        key: K,
        namespace: Option<N>,
        value: V,
    ) -> taimi_pack::script::Result<Option<String>>
    where
        K: ScriptUserStr,
        N: ScriptUserStr,
        V: ScriptUserStr,
    {
        let ns = namespace.map(|ns| ns.clone_to_string());
        let key = key.with_str(|k| self.full_key(k, ns));
        Ok(self.with_entry_mut(key, |entry, dirty| match entry {
            hash_map::Entry::Occupied(e) => {
                let e = e.into_mut();
                let prev = value.with_str(|v| mem::replace(e, v.into()));
                // could compare for changes but...
                *dirty = true;
                Some(data::value_to_string(prev))
            },
            hash_map::Entry::Vacant(e) => {
                value.with_str(|v| e.insert(v.into()));
                *dirty = true;
                None
            },
        }))
    }
}
