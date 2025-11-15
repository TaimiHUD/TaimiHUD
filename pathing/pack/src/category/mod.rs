use {
    self::id::IdNameSeg,
    crate::{
        attributes::{
            cell::{
                pack_attr,
                AttrKeyValue,
                GetAttrDyn,
                PackKeyId,
                PackValueCell,
                PackValueDyn,
                SetAttrDyn,
            },
            keys::{self, GetAttr, SetAttr},
            parse_bool,
            MarkerAttributes,
        },
        pack::PackBuilderCategoryWarnings,
    },
    anyhow::{anyhow, Context},
    bitflags::bitflags,
    std::{borrow::Cow, mem, sync::Arc},
};

pub use self::id::CategoryId;

pub mod id;

#[derive(Debug, Clone)]
pub struct Category {
    pub full_id: CategoryId,
    pub display_name: Option<Arc<str>>,
    pub flags: CategoryFlags,
    // Map of local to global name.
    pub sub_categories: Box<[CategoryId]>,
    /// Attributes for markers attached to this category.
    pub marker_attributes: MarkerAttributes,
}

impl Category {
    pub fn from_xml(
        warnings: &mut PackBuilderCategoryWarnings,
        attrs: Vec<xml::attribute::OwnedAttribute>,
    ) -> anyhow::Result<Category> {
        let mut marker_attributes = MarkerAttributes::default();
        let mut attributes_bh = MarkerAttributes::default();

        let mut id = None;
        let mut bh_id = None;
        let mut display_name = None;
        let mut bh_display_name = None;
        let mut is_separator = None;
        let mut bh_is_separator = None;
        let mut is_hidden = None;
        let mut bh_is_hidden = None;
        let mut default_toggle = None;
        let mut bh_default_toggle = None;

        for attr in attrs {
            let attr_name = &attr.name.local_name;
            let res = if attr_name.eq_ignore_ascii_case("name") {
                id = Some(attr.value);
                Ok(())
            } else if attr_name.eq_ignore_ascii_case("displayname") {
                display_name = Some(attr.value);
                Ok(())
            } else if attr_name.eq_ignore_ascii_case("isseparator") {
                parse_bool(&attr.value)
                    .map(|val| is_separator = Some(val))
                    .map_err(From::from)
            } else if attr_name.eq_ignore_ascii_case("ishidden") {
                parse_bool(&attr.value)
                    .map(|val| is_hidden = Some(val))
                    .map_err(From::from)
            } else if attr_name.eq_ignore_ascii_case("defaulttoggle") {
                parse_bool(&attr.value)
                    .map(|val| default_toggle = Some(val))
                    .map_err(From::from)
            } else if let Some(attr_name) = attr_name.strip_prefix("bh-") {
                if attr_name.eq_ignore_ascii_case("name") {
                    bh_id = Some(attr.value);
                    Ok(())
                } else if attr_name.eq_ignore_ascii_case("displayname") {
                    bh_display_name = Some(attr.value);
                    Ok(())
                } else if attr_name.eq_ignore_ascii_case("isseparator") {
                    parse_bool(&attr.value)
                        .map(|val| bh_is_separator = Some(val))
                        .map_err(From::from)
                } else if attr_name.eq_ignore_ascii_case("ishidden") {
                    parse_bool(&attr.value)
                        .map(|val| bh_is_hidden = Some(val))
                        .map_err(From::from)
                } else if attr_name.eq_ignore_ascii_case("defaulttoggle") {
                    parse_bool(&attr.value)
                        .map(|val| bh_default_toggle = Some(val))
                        .map_err(From::from)
                } else {
                    match attributes_bh.try_add(attr.name.borrow(), attr.value) {
                        Ok(false) => {
                            warnings.attr_warning(&attr.name, &"Category");
                            Ok(())
                        },
                        res => res.map(drop),
                    }
                }
            } else {
                match marker_attributes.try_add(attr.name.borrow(), attr.value) {
                    Ok(false) => {
                        warnings.attr_warning(&attr.name, &"Category");
                        Ok(())
                    },
                    res => res.map(drop),
                }
            }
            .with_context(|| format!("parsing category attribute '{}'", attr.name));
            if let Err(e) = res {
                log::warn!("{e:#}");
            }
        }

        let id = id.or(bh_id).ok_or_else(|| anyhow!("category missing name"))?;
        #[cfg(todo)]
        let (id, name) = match to_taco_safe_name(&name, false) {
            Ok(..) => (name, None),
            Err(safe) => (safe, Some(name)),
        };

        let Some(id) = CategoryId::try_with_full_id(&id) else {
            anyhow::bail!("empty category name");
        };

        // TODO: support bh features properly...
        marker_attributes.merge(&attributes_bh, false);

        let display_name = display_name.or(bh_display_name);

        let is_separator = is_separator.or(bh_is_separator).unwrap_or(false);
        let is_hidden = is_hidden.or(bh_is_hidden).unwrap_or(false);
        let default_toggle = default_toggle.or(bh_default_toggle).unwrap_or(true);
        let flags = [
            is_separator.then_some(CategoryFlag::Separator),
            is_hidden.then_some(CategoryFlag::Hidden),
            (!default_toggle).then_some(CategoryFlag::Disabled),
        ]
        .into_iter()
        .filter_map(|f| f)
        .collect::<CategoryFlags>();

        Ok(Category {
            display_name: display_name.map(|n| n.into()),
            full_id: id,
            flags,
            sub_categories: Default::default(),
            marker_attributes,
        })
    }

    pub fn merge(&mut self, mut new: Category) {
        if self.full_id != new.full_id {
            log::error!(
                "Invalid category state. Attempted to merge {} onto {}",
                new.full_id,
                self.full_id
            );
            return;
        }
        new.attributes_mut().merge(&self.marker_attributes, false);
        if self.display_name.is_none() {
            self.display_name = new.display_name;
        }
        self.append_children(new.sub_categories);
    }
    pub fn get_display_name(&self) -> Option<&str> {
        self.display_name.as_ref().map(|n| &**n)
    }
    pub fn display_name(&self) -> &str {
        self.get_display_name().unwrap_or(self.id().as_str())
    }

    /// TODO: way too thrashy :<
    pub fn append_children<I: IntoIterator<Item = CategoryId>>(&mut self, children: I) {
        let mut sub_categories = Vec::from(mem::take(&mut self.sub_categories));
        for id in children {
            let contained = sub_categories.iter();
            #[cfg(todo = "unnecessary")]
            let contained = contained.map(IdCmpRelaxed::with_ref);
            if { contained }.any(|c| c == &id) {
                continue
            }
            sub_categories.push(id);
        }
        self.sub_categories = sub_categories.into()
    }

    #[inline]
    pub fn attributes_mut(&mut self) -> &mut MarkerAttributes {
        &mut self.marker_attributes
    }

    pub fn id(&self) -> &IdNameSeg {
        self.full_id.name()
    }

    #[cfg(todo = "unnecessary")]
    pub fn child_names(&self) -> impl Iterator<Item = Arc<str>> + Clone + '_ {
        self.sub_categories.iter().map(|(id, full_id)| id)
    }
    pub fn child_ids(&self) -> impl Iterator<Item = &CategoryId> + Clone {
        self.sub_categories.iter()
    }

    pub fn is_separator(&self) -> bool {
        self.flags.is_separator()
    }
    pub fn is_hidden(&self) -> bool {
        self.flags.is_hidden()
    }
    pub fn default_toggle(&self) -> bool {
        !self.flags.is_disabled()
    }

    /// Once all child markers have inherited attributes from a category,
    /// it doesn't really have any further need for related attribute data
    pub fn trim_attributes(&mut self) {
        let is_sep = self.is_separator();
        let attrs = self.attributes_mut();
        let _ = attrs.render.take();
        let _ = attrs.filters.take();
        let _ = attrs.script.take();
        if !is_sep
            || attrs
                .interaction
                .as_ref()
                .map(|i| i.copy_value.is_none())
                .unwrap_or(true)
        {
            // separator categories are an exception that can be directly copied,
            // so avoid clearing those...
            let _ = attrs.interaction.take();
        }
    }
}
impl GetAttr<keys::CategoryRef> for Category {
    #[inline]
    fn has_attr(&self) -> bool {
        !self.flags.is(CategoryFlag::Root)
        //&& self.full_id.parent().is_some()
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::CategoryRef>> {
        (!self.flags.is(CategoryFlag::Root))
            .then(|| self.full_id.parent())
            .flatten()
            .map(|p| Cow::Owned(p.into()))
    }
}
#[cfg(todo)]
impl SetAttr<keys::CategoryRef> for Category {}
impl GetAttr<keys::NameId> for Category {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::NameId>> {
        let id = match self.flags.is(CategoryFlag::Root) {
            false => self.id().into(),
            true => self.full_id[..].into(),
        };
        Some(Cow::Owned(id))
    }
}
#[cfg(todo)]
impl SetAttr<keys::NameId> for Category {}
impl GetAttr<keys::DisplayName> for Category {
    #[inline]
    fn has_attr(&self) -> bool {
        self.display_name.is_some()
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::DisplayName>> {
        self.display_name.as_ref().map(|n| Cow::Owned(n.into()))
    }
}
impl SetAttr<keys::DisplayName> for Category {
    #[inline]
    fn set_attr(&mut self, v: keys::DisplayName) {
        self.display_name = (!v[..].is_empty()).then_some(v[..].into());
    }
}
impl Category {
    fn holds_attr_dyn_inherent(key: PackKeyId) -> bool {
        pack_attr!(=id_is_in(key, [
            keys::DisplayName,
            keys::NameId,
            keys::CategoryRef,
        ]))
    }
}
impl GetAttrDyn for Category {
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        Self::holds_attr_dyn_inherent(key)
            || CategoryFlags::holds_attr_dyn(key)
            || MarkerAttributes::holds_attr_dyn(key)
    }
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        let has = pack_attr!(imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::CategoryRef,
        ]);
        let has = match has {
            Some(has) => return has,
            #[cfg(todo)]
            _ => pack_attr!(=id_is_in(key, [
                keys::DisplayName,
                keys::NameId,
            ])),
            _ => Self::holds_attr_dyn_inherent(key),
        };
        has || self.flags.has_attr_dyn(key) || self.marker_attributes.has_attr_dyn(key)
    }
    #[inline]
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        self.marker_attributes.get_attr_dyn_ref(key)
    }
    #[inline]
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        if Self::holds_attr_dyn_inherent(key) {
            self.clone_attr_dyn(key).map(Cow::Owned)
        } else if CategoryFlags::holds_attr_dyn(key) {
            self.flags.get_attr_dyn(key)
        } else {
            self.marker_attributes.get_attr_dyn(key)
        }
    }
    #[inline]
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        let v = pack_attr! { imp GetAttrDyn::clone_attr_dyn(self, key) in [
            keys::DisplayName,
            keys::NameId,
            keys::CategoryRef,
        ] };
        if let Some(v) = v {
            v
        } else if CategoryFlags::holds_attr_dyn(key) {
            self.flags.clone_attr_dyn(key)
        } else {
            self.marker_attributes.clone_attr_dyn(key)
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::DisplayName,
            keys::NameId,
            keys::CategoryRef,
        ] }
        .chain(self.flags.iter_attrs_dyn())
        .chain(self.marker_attributes.iter_attrs_dyn())
    }
}
impl SetAttrDyn for Category {
    #[inline]
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in
            [
                keys::DisplayName,
                //keys::NameId,
                //keys::CategoryRef,
            ],
            _ => if CategoryFlags::holds_attr_dyn(value.id()) {
                self.flags.set_attr_dyn(value)
            } else {
                self.marker_attributes.set_attr_dyn(value)
            },
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum CategoryFlag {
    /// separator
    Separator = 1,
    /// ishidden
    Hidden = 2,
    /// !defaulttoggle
    Disabled = 3,
    /// no parent
    Root = Self::REPR_MAX,
}
impl CategoryFlag {
    pub const INDEX_MAX: u8 = Self::REPR_MAX - Self::REPR_MIN;
    pub const REPR_MIN: u8 = 1;
    pub const REPR_MAX: u8 = 4;

    #[inline]
    pub const fn index(self) -> u8 {
        self.repr() - Self::REPR_MIN
    }
    #[inline]
    pub const fn bit(self) -> CategoryFlags {
        CategoryFlags::from_bits_retain(1u8 << self.index())
    }
    #[inline]
    pub const fn repr(self) -> u8 {
        self as u8
    }
    pub const fn from_index(index: u8) -> Option<Self> {
        match index {
            0..=Self::INDEX_MAX => Some(unsafe { Self::from_index_unchecked(index) }),
            _ => None,
        }
    }
    pub const fn from_repr(index: u8) -> Option<Self> {
        match index {
            Self::REPR_MIN..=Self::REPR_MAX => Some(unsafe { Self::from_index_unchecked(index) }),
            _ => None,
        }
    }
    #[inline]
    pub const unsafe fn from_index_unchecked(index: u8) -> Self {
        Self::from_repr_unchecked(index + Self::REPR_MIN)
    }
    #[inline]
    pub const unsafe fn from_repr_unchecked(repr: u8) -> Self {
        mem::transmute(repr)
    }
}

bitflags! {
    #[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct CategoryFlags: u8 {
        const SEPARATOR = 1u8 << CategoryFlag::Separator.index();
        const HIDDEN = 1u8 << CategoryFlag::Hidden.index();
        const DISABLED = 1u8 << CategoryFlag::Disabled.index();
        const ROOT = 1u8 << CategoryFlag::Root.index();
    }
}
impl CategoryFlags {
    pub fn next_flag(self) -> Option<CategoryFlag> {
        let bits = self & Self::all();
        let index = bits.bits().trailing_zeros();
        CategoryFlag::from_index(index as u8)
    }
    pub fn flags(self) -> impl Iterator<Item = CategoryFlag> {
        self.into_iter().filter_map(|flag| flag.next_flag())
    }
    pub const fn is(&self, flag: CategoryFlag) -> bool {
        self.bits() & flag.bit().bits() != 0
    }

    #[inline]
    pub fn is_separator(&self) -> bool {
        self.intersects(CategoryFlags::SEPARATOR)
    }
    #[inline]
    pub fn is_hidden(&self) -> bool {
        self.intersects(CategoryFlags::HIDDEN)
    }
    #[inline]
    pub fn is_disabled(&self) -> bool {
        self.intersects(CategoryFlags::DISABLED)
    }
}
impl From<CategoryFlag> for CategoryFlags {
    fn from(flag: CategoryFlag) -> Self {
        flag.bit()
    }
}
impl FromIterator<CategoryFlag> for CategoryFlags {
    fn from_iter<I: IntoIterator<Item = CategoryFlag>>(iter: I) -> Self {
        Self::from_iter(iter.into_iter().map(Self::from))
    }
}
impl Extend<CategoryFlag> for CategoryFlags {
    fn extend<I: IntoIterator<Item = CategoryFlag>>(&mut self, iter: I) {
        self.extend(iter.into_iter().map(Self::from))
    }
}

impl GetAttr<keys::IsSeparator> for CategoryFlags {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::IsSeparator>> {
        Some(Cow::Owned(self.contains(CategoryFlags::SEPARATOR).into()))
    }
}
impl SetAttr<keys::IsSeparator> for CategoryFlags {
    #[inline]
    fn set_attr(&mut self, v: keys::IsSeparator) {
        self.set(CategoryFlags::SEPARATOR, v.into())
    }
}
impl GetAttr<keys::IsHidden> for CategoryFlags {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::IsHidden>> {
        Some(Cow::Owned(self.contains(CategoryFlags::HIDDEN).into()))
    }
}
impl SetAttr<keys::IsHidden> for CategoryFlags {
    #[inline]
    fn set_attr(&mut self, v: keys::IsHidden) {
        self.set(CategoryFlags::HIDDEN, v.into())
    }
}
impl GetAttr<keys::DefaultToggle> for CategoryFlags {
    #[inline]
    fn has_attr(&self) -> bool {
        true
    }
    #[inline]
    fn get_attr(&self) -> Option<Cow<'_, keys::DefaultToggle>> {
        Some(Cow::Owned(keys::DefaultToggle::from(
            !self.contains(CategoryFlags::DISABLED),
        )))
    }
}
impl SetAttr<keys::DefaultToggle> for CategoryFlags {
    #[inline]
    fn set_attr(&mut self, v: keys::DefaultToggle) {
        self.set(CategoryFlags::DISABLED, !bool::from(v))
    }
}
impl GetAttrDyn for CategoryFlags {
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        pack_attr!(=id_is_in(key, [
            keys::DefaultToggle,
            keys::IsSeparator,
            keys::IsHidden,
        ]))
    }
    #[inline]
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        Self::holds_attr_dyn(key)
    }
    #[inline]
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        self.clone_attr_dyn(key).map(Cow::Owned)
    }
    #[inline]
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        pack_attr! { imp GetAttrDyn::clone_attr_dyn(self, key) in [
            keys::DefaultToggle,
            keys::IsSeparator,
            keys::IsHidden,
        ] }
        .flatten()
    }

    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::DefaultToggle,
            keys::IsSeparator,
            keys::IsHidden,
        ] }
    }
}
impl SetAttrDyn for CategoryFlags {
    #[inline]
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::DefaultToggle,
            keys::IsSeparator,
            keys::IsHidden,
        ] }
    }
}
