use {
    self::id::IdNameSeg,
    crate::attributes::{parse_bool, MarkerAttributes},
    anyhow::{anyhow, Context},
    bitflags::bitflags,
    core::{mem, ops},
    std::sync::Arc,
    taimi_hoard::flags::{
        set::{BitFlagForSet, FlagSet},
        BitSlice,
        BitVec,
        BitView,
        BitsNative,
    },
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
    pub fn from_xml(attrs: Vec<xml::attribute::OwnedAttribute>) -> anyhow::Result<Category> {
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
                        Ok(false) => Ok(log::debug!("unrecognized category attribute `{}`", attr.name)),
                        res => res.map(drop),
                    }
                }
            } else {
                match marker_attributes.try_add(attr.name.borrow(), attr.value) {
                    Ok(false) => Ok(log::info!("unrecognized category attribute `{}`", attr.name)),
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

pub type CategoryFlagSet<V = BitVec<u8>> = FlagSet<CategoryFlags, V>;
impl BitFlagForSet for CategoryFlags {
    type Repr = u8;
    const BIT_WIDTH: usize = CategoryFlag::INDEX_MAX as usize + 1;

    fn as_bits(&self) -> &Self::Repr {
        unsafe { &*(self as *const Self as *const u8) }
    }
    fn as_bits_mut(&mut self) -> &mut Self::Repr {
        unsafe { &mut *(self as *mut Self as *mut u8) }
    }
    fn as_bitslice(&self) -> &BitSlice<Self::Repr, BitsNative> {
        unsafe { self.as_bits().view_bits().get_unchecked(..Self::BIT_WIDTH) }
    }
    fn as_bitslice_mut(&mut self) -> &mut BitSlice<Self::Repr, BitsNative> {
        unsafe {
            self.as_bits_mut()
                .view_bits_mut()
                .get_unchecked_mut(..Self::BIT_WIDTH)
        }
    }

    fn range_for(index: usize) -> ops::Range<usize> {
        let start = index << 2;
        let end = start + Self::BIT_WIDTH;
        start..end
    }
}
