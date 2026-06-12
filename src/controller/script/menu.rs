use {
    crate::{controller::script::PlugSharedData, exports::runtime as rt},
    core::borrow::Borrow,
    std::{borrow::Cow, collections::BTreeMap, sync::RwLock},
    taimi_pack::{
        attributes::{
            cell::{pack_attr, AttrKeyValue, GetAttrDyn, PackKeyId, PackValueCell, SetAttrDyn},
            keys::{self, GetAttr, SetAttr},
            AttrString,
        },
        category::{id, CategoryId},
        pack::to_taco_safe_name,
        script::{
            self,
            pathing::{MenuDesc, MenuHandle, MenuHandleMut, MenuInstance},
        },
    },
};

pub type PlugMenusData = PlugMenusById;
pub type PlugMenusById = BTreeMap<CategoryId, PlugMenuState>;
#[derive(Debug, Default)]
pub struct PlugMenusShared {
    pub shared: RwLock<PlugMenusById>,
}
impl PlugMenusShared {
    pub fn menu_add(&self, id: CategoryId, menu: PlugMenuState) -> bool {
        let mut menus = self.shared.write().unwrap_or_else(|e| e.into_inner());
        if menus.contains_key(&id) {
            return false
        }
        menus.insert(id, menu);
        true
    }
    pub fn menu_remove<Q>(&self, id: &Q, recursive: bool) -> bool
    where
        Q: ?Sized + Ord,
        CategoryId: Borrow<Q>,
    {
        let mut menus = self.shared.write().unwrap_or_else(|e| e.into_inner());
        let Some((id, _removed)) = menus.remove_entry(id) else { return false };
        if recursive {
            #[cfg(todo = "unnecessary")]
            let id = IdCmpRelaxed::with_ref(id);
            menus.retain(|k, _| !id::AsFullId::id_starts_with(k, &id));
        }
        true
    }
    pub fn menu_write<Q, F, R>(&self, id: &Q, f: F) -> Option<R>
    where
        F: FnOnce(&mut PlugMenuState) -> R,
        Q: ?Sized + Ord,
        CategoryId: Borrow<Q>,
    {
        let mut menus = self.shared.write().unwrap_or_else(|e| e.into_inner());
        menus.get_mut(id).map(f)
    }
    pub fn menu_read<Q, F, R>(&self, id: &Q, f: F) -> Option<R>
    where
        F: FnOnce(&CategoryId, &PlugMenuState) -> R,
        CategoryId: Borrow<Q>,
        Q: ?Sized + Ord,
    {
        let menus = self.shared.read().unwrap_or_else(|e| e.into_inner());
        menus.get_key_value(id).map(|(k, v)| f(k, v))
    }
}

pub struct PlugMenuInstance<T, R> {
    pub root: Option<R>,
    pub shared: T,
}
impl<T, R> PlugMenuInstance<T, R> {
    pub fn new<S>(shared: S, root: Option<R>) -> Self
    where
        S: Into<T>,
    {
        Self { shared: shared.into(), root }
    }
}
impl<T, R> PlugMenuInstance<T, R>
where
    T: AsRef<PlugSharedData>,
{
    pub fn menus(&self) -> &PlugMenusShared {
        &AsRef::<PlugSharedData>::as_ref(&self.shared).menus
    }
    pub fn imp_gen_id(
        &self,
        parent: Option<&id::FullIdRef>,
        name: Option<&id::IdNameSeg>,
    ) -> script::Result<String>
    where
        R: MenuDesc,
    {
        use {
            core::hash::{Hash, Hasher},
            rustc_hash::FxHasher,
        };
        let menus = self.menus().shared.read().unwrap_or_else(|e| e.into_inner());
        let name = name.map(|n| n.as_str()).unwrap_or("item");
        let name = match to_taco_safe_name(name, parent.is_none()) {
            Ok(n) => Cow::Borrowed(n),
            Err(n) => Cow::Owned(n),
        };
        let root_id;
        let parent = match parent {
            Some(p) => Some(p),
            None => match &self.root {
                Some(r) => {
                    root_id = rt::log::warn_ok(r.get_id());
                    root_id.as_ref().map(|ns| ns.as_id())
                },
                None => None,
            },
        };
        let base;
        let base = match parent {
            None => &name[..],
            Some(p) => {
                base = format!("{p}{}{name}", id::SEP_STR);
                &base[..]
            },
        };
        if !menus.contains_key(&base[..]) {
            return Ok(base.into())
        }

        let mut postfix = FxHasher::default();
        for k in menus.keys() {
            k.hash(&mut postfix)
        }
        for i in 0..32 {
            i.hash(&mut postfix);
            let postfix = postfix.finish();
            let whee = format!("{base}_{postfix:04x}");
            if !menus.contains_key(&whee[..]) {
                return Ok(whee)
            }
        }
        Err(script::format_err!("failed"))
    }
    pub fn imp_remove_id(&self, id: &id::FullIdRef, recursive: bool) -> script::Result<()> {
        if self.menus().menu_remove(id, recursive) {
            Ok(())
        } else {
            Err(script::format_err!("menu {id} not found"))
        }
    }
}
impl<T, R> MenuInstance for PlugMenuInstance<T, R>
where
    T: ToOwned + AsRef<PlugSharedData>,
    T::Owned: AsRef<PlugSharedData>,
    R: MenuDesc,
{
    fn gen_id(
        &self,
        parent: Option<&id::FullIdRef>,
        name: Option<&id::IdNameSeg>,
    ) -> script::Result<String> {
        self.imp_gen_id(parent, name)
    }
    fn lookup_id(&self, id: &id::FullIdRef) -> script::Result<Option<Self::Menu>> {
        Ok(self
            .menus()
            .menu_read(id, |k, _| PlugMenu::new(k.clone(), self.shared.to_owned())))
    }
    fn remove_id(&self, id: &id::FullIdRef, recursive: bool) -> script::Result<()> {
        if self.menus().menu_remove(id, recursive) {
            Ok(())
        } else {
            Err(script::format_err!("menu {id} not found"))
        }
    }
    fn register_id(&self, id: CategoryId) -> script::Result<Self::RegisteredMenu> {
        let menu = PlugMenuState {
            #[cfg(todo = "unnecessary")]
            display_name: id.as_str().into(),
            ..Default::default()
        };
        match self.menus().menu_add(id.clone(), menu) {
            true => Ok(PlugMenu::new(id, self.shared.to_owned())),
            false => Err(script::format_err!("duplicated menu {id}")),
        }
    }
    type Menu = PlugMenu<T::Owned>;
    type RegisteredMenu = PlugMenu<T::Owned>;
}
impl<T, R> MenuDesc for PlugMenuInstance<T, R>
where
    R: MenuDesc,
{
    fn get_id(&self) -> script::Result<CategoryId> {
        self.root
            .as_ref()
            .map(MenuDesc::get_id)
            .unwrap_or_else(|| Ok(unsafe { CategoryId::try_with_full_id("menu").unwrap_unchecked() }))
    }
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        self.root
            .as_ref()
            .map(|root| MenuDesc::get_menu_attr_dyn(root, id))
            .unwrap_or(Ok(None))
    }
}

#[derive(Debug, Clone, Default)]
pub struct PlugMenuState {
    pub checked: Option<bool>,
    pub emit_click: bool,
    pub display_name: AttrString,
    pub tooltip: Option<AttrString>,
    pub tooltip_title: Option<AttrString>,
}
impl PlugMenuState {
    pub fn click_state(&mut self) {
        if let Some(s) = &mut self.checked {
            *s ^= true;
        }
    }
}
pack_attr! {
    impl Attr{keys::DisplayName} for &struct{PlugMenuState}.display_name {}
    impl Attr{keys::TipDescription} for &struct{PlugMenuState}.tooltip? {}
    impl Attr{keys::TipName} for &struct{PlugMenuState}.tooltip_title? {}
    impl Attr{keys::DefaultToggle} for &struct{PlugMenuState}.checked? {}
}
impl GetAttr<keys::IsSeparator> for PlugMenuState {
    fn has_attr(&self) -> bool {
        true
    }
    fn get_attr(&self) -> Option<Cow<'_, keys::IsSeparator>> {
        Some(Cow::Owned(self.checked.is_some().into()))
    }
}
impl SetAttr<keys::IsSeparator> for PlugMenuState {
    fn set_attr(&mut self, value: keys::IsSeparator) {
        match bool::from(value) {
            true if self.checked.is_none() => self.checked = Some(false),
            false if self.checked.is_some() => self.checked = None,
            _ => (),
        }
    }
    fn unset_attr(&mut self) {
        self.checked = None;
    }
}
#[cfg(todo = "unnecessary")]
impl GetAttr<keys::DefaultToggle> for PlugMenuState {
    fn has_attr(&self) -> bool {
        self.checked.is_some()
    }
    fn get_attr_ref(&self) -> &keys::DefaultToggle {
        self.checked.as_ref().map(keys::DefaultToggle::from_ref)
    }
}
#[cfg(todo = "unnecessary")]
impl SetAttr<keys::DefaultToggle> for PlugMenuState {
    fn set_attr(&mut self, value: keys::DefaultToggle) {
        if let Some(checked) = &mut self.checked {
            *checked = value
        }
    }
    fn unset_attr(&mut self) {
        if let Some(checked) = &mut self.checked {
            *checked = false;
        }
    }
}
impl GetAttrDyn for PlugMenuState {
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        pack_attr! { =id_is_in(key, [
            keys::DisplayName,
            keys::TipDescription,
            keys::TipName,
            keys::IsSeparator,
            keys::DefaultToggle,
        ]) }
    }
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::TipDescription,
            keys::TipName,
        ] }
        .unwrap_or_else(|| {
            pack_attr! { =id_is_in(key, [
                keys::DisplayName,
                keys::IsSeparator,
                keys::DefaultToggle,
            ]) }
        })
    }
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        pack_attr! { imp GetAttrDyn::get_attr_dyn(self, key) in [
            keys::DisplayName,
            keys::TipDescription,
            keys::TipName,
            keys::IsSeparator,
            keys::DefaultToggle,
        ] }
        .flatten()
    }
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::DisplayName,
            keys::TipDescription,
            keys::TipName,
            keys::DefaultToggle,
        ] }
        .flatten()
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::DisplayName,
            keys::TipDescription,
            keys::TipName,
            keys::IsSeparator,
            keys::DefaultToggle,
        ] }
    }
}
impl SetAttrDyn for PlugMenuState {
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::DisplayName,
            keys::TipDescription,
            keys::TipName,
            keys::IsSeparator,
            keys::DefaultToggle,
        ] }
    }
}

#[derive(Debug, Clone)]
pub struct PlugMenu<S> {
    shared: S,
    id: CategoryId,
}
impl<S> PlugMenu<S> {
    pub fn new<T>(id: CategoryId, shared: T) -> Self
    where
        T: Into<S>,
    {
        Self { id, shared: shared.into() }
    }
}
impl<S> PlugMenu<S>
where
    S: AsRef<PlugSharedData>,
{
    pub fn menus(&self) -> &PlugMenusShared {
        &AsRef::<PlugSharedData>::as_ref(&self.shared).menus
    }
}
impl<S> MenuDesc for PlugMenu<S>
where
    S: AsRef<PlugSharedData>,
{
    fn get_id(&self) -> script::Result<CategoryId> {
        Ok(self.id.clone())
    }
    fn get_menu_attr_dyn(&self, id: PackKeyId) -> script::Result<Option<PackValueCell>> {
        pack_attr! { match =id_is(id) {
            = keys::NameId => Ok(Some(PackValueCell::new_boxed(keys::NameId::from(self.id.name())))),
            = keys::CategoryRef => Ok(self.id.get_parent().map(|v| PackValueCell::new_boxed(keys::CategoryRef::from(v)))),
            _ => self.menus().menu_read(&self.id, |_, state|
                state.get_attr_dyn(id).map(|v| v.into())
            ).ok_or_else(|| script::format_err!("menu removed?")),
        } }
    }
}
impl<S> MenuHandle for PlugMenu<S>
where
    S: AsRef<PlugSharedData>,
{
    fn get_check_state(&self) -> script::Result<Option<bool>> {
        self.menus()
            .menu_read(&self.id, |_, state| state.checked)
            .ok_or_else(|| script::format_err!("menu removed?"))
    }
}
impl<S> MenuHandleMut for PlugMenu<S>
where
    S: AsRef<PlugSharedData>,
{
    fn set_check_state(&self, v: Option<bool>) -> script::Result<()> {
        self.menus()
            .menu_write(&self.id, |state| state.checked = v)
            .ok_or_else(|| script::format_err!("menu removed"))
    }
    fn set_menu_attr_dyn(&self, v: PackValueCell) -> script::Result<()> {
        let key = v.id();
        let res = self.menus().menu_write(&self.id, |state| state.set_attr_dyn(v));
        match res {
            Some(true) => Ok(()),
            Some(false) => Err(script::format_err!("menu cannot store {key}")),
            None => Err(script::format_err!("menu removed")),
        }
    }
}
