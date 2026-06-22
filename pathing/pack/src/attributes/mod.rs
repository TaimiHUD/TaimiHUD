use {
    self::cell::GetAttrDyn,
    crate::{category::id::IdNameBox, pack::taco_xml_to_guid},
    anyhow::{anyhow, Context},
    glam::{Vec3, Vec4},
    std::{borrow::Cow, fmt, str::FromStr, sync::Arc},
    taimi_hoard::str_opt_ref,
    xml::name::Name,
};

pub use self::{
    festival::{Festival, Festivals},
    mount::{Mount, Mounts},
    profession::{Profession, Professions},
    race::{Race, Races},
};

pub mod cell;
pub mod festival;
pub mod keys;
pub mod mount;
pub mod profession;
pub mod race;

pub type AttrString = Arc<Box<str>>;
pub fn string_into(s: impl Into<Box<str>>) -> AttrString {
    Arc::new(s.into())
}
#[cfg(todo)]
pub type AttrString = Arc<str>;
#[cfg(todo)]
pub fn string_into(s: impl Into<Arc<str>>) -> AttrString {
    s.into()
}
pub type AttrList<T> = Arc<Box<[T]>>;
#[cfg(todo = "unused")]
fn list_into<T>(l: impl Into<Box<[T]>>) -> AttrList<T> {
    Arc::new(l.into())
}

#[derive(Debug, Clone, Default)]
/// Attributes for markers. Inherits up the category stack.
pub struct MarkerAttributes {
    // Common.
    pub edit_tag: Option<i32>,
    pub minimap_visibility: Option<bool>,
    pub map_visibility: Option<bool>,
    pub in_game_visibility: Option<bool>,
    pub tip_name: Option<AttrString>,
    pub tip_description: Option<AttrString>,
    pub render: Option<Arc<RenderAttributes>>,

    pub filters: Option<Box<FilterAttributes>>,

    pub interaction: Option<Arc<InteractionAttributes>>,

    pub script: Option<Box<ScriptAttributes>>,
}

impl MarkerAttributes {
    pub fn merge(&mut self, base: &MarkerAttributes, child: bool) {
        // === Common === //
        match (&base.render, &mut self.render) {
            (Some(base), render @ None) => *render = Some(base.clone()),
            (Some(base), Some(..)) => self.render_mut().merge(base),
            (None, _) => (),
        }
        if self.edit_tag.is_none() {
            self.edit_tag = base.edit_tag;
        }
        if self.minimap_visibility.is_none() {
            self.minimap_visibility = base.minimap_visibility;
        }
        if self.map_visibility.is_none() {
            self.map_visibility = base.map_visibility;
        }
        if self.in_game_visibility.is_none() {
            self.in_game_visibility = base.in_game_visibility;
        }
        if !child && self.tip_name.is_none() {
            self.tip_name = base.tip_name.clone();
        }
        if !child && self.tip_description.is_none() {
            self.tip_description = base.tip_description.clone();
        }
        // === Filters === //
        match (&base.filters, &mut self.filters) {
            (Some(base), filters @ None) => *filters = Some(base.clone()),
            (Some(base), Some(filters)) => filters.merge(base),
            (None, _) => (),
        }
        // === Modifiers === //
        match (&base.interaction, &mut self.interaction) {
            (Some(base), interaction @ None) if !child => *interaction = Some(base.clone()),
            (Some(base), _) => self.interaction_mut().merge(base, child),
            (None, _) => (),
        }
        // === Scripting === //
        match (&base.script, &mut self.script) {
            (Some(base), script @ None) => *script = Some(base.clone()),
            (Some(base), Some(script)) => script.merge(base),
            (None, _) => (),
        }
    }

    pub fn try_add(&mut self, name: Name<'_>, value: String) -> anyhow::Result<bool> {
        let attr_name = &name.local_name.trim_start_matches("bh-");
        // === Common === //
        if attr_name.eq_ignore_ascii_case("alpha") {
            self.render_mut().alpha = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("canfade") {
            self.render_mut().can_fade = Some(parse_bool(&value)?);
        } else if attr_name.eq_ignore_ascii_case("color") || attr_name.eq_ignore_ascii_case("tint") {
            if let Some(tint) = opt_str(&value).map(parse_color).transpose()? {
                self.render_mut().tint = Some(tint);
            }
        } else if attr_name.eq_ignore_ascii_case("cull") {
            self.render_mut().cull = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("edittag") {
            self.edit_tag = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("fadenear") {
            self.render_mut().fade_near = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("fadefar") {
            self.render_mut().fade_far = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("minimapvisibility") {
            self.minimap_visibility = Some(parse_bool(&value)?);
        } else if attr_name.eq_ignore_ascii_case("mapvisibility") {
            self.map_visibility = Some(parse_bool(&value)?);
        } else if attr_name.eq_ignore_ascii_case("ingamevisibility") {
            self.in_game_visibility = Some(parse_bool(&value)?);
        // === POI-specific === //
        } else if attr_name.eq_ignore_ascii_case("heightoffset") {
            self.poi_mut().height_offset = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("iconfile") {
            self.poi_mut().icon_file = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("iconsize") {
            self.poi_mut().icon_size = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("mapdisplaysize") {
            self.poi_mut().map_display_size = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("scaleonmapwithzoom") {
            self.poi_mut().scale_on_map_with_zoom = Some(parse_bool(&value)?);
        } else if attr_name.eq_ignore_ascii_case("minsize") {
            self.poi_mut().min_size = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("maxsize") {
            self.poi_mut().max_size = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("occlude") {
            self.poi_mut().occlude = Some(parse_bool(&value)?);
        } else if attr_name.eq_ignore_ascii_case("rotate") {
            // TODO: unclear if unspecified trailing axis should count as "set" or not for inherit reasons?
            // also attr definition order may matter, with this taking priority
            let rotate = &mut self.poi_mut().rotate;
            match value.is_empty() {
                true => *rotate = None,
                false => {
                    let mut axes = rotate.unwrap_or(PoiAttributes::ROTATE_UNSET).to_array();
                    parse_into_array(&mut axes, &value)?;
                    *rotate = Some(Vec3::from_array(axes));
                },
            }
        } else if attr_name.eq_ignore_ascii_case("rotate-x") {
            let x = str_opt_ref(&value).map(f32::from_str).transpose()?;
            self.poi_mut().rotate.get_or_insert(PoiAttributes::ROTATE_UNSET).x =
                x.unwrap_or(PoiAttributes::ROTATE_UNSET_AXIS);
        } else if attr_name.eq_ignore_ascii_case("rotate-y") {
            let y = str_opt_ref(&value).map(f32::from_str).transpose()?;
            self.poi_mut().rotate.get_or_insert(PoiAttributes::ROTATE_UNSET).y =
                y.unwrap_or(PoiAttributes::ROTATE_UNSET_AXIS);
        } else if attr_name.eq_ignore_ascii_case("rotate-z") {
            let z = str_opt_ref(&value).map(f32::from_str).transpose()?;
            self.poi_mut().rotate.get_or_insert(PoiAttributes::ROTATE_UNSET).z =
                z.unwrap_or(PoiAttributes::ROTATE_UNSET_AXIS);
        } else if attr_name.eq_ignore_ascii_case("text") || attr_name.eq_ignore_ascii_case("title") {
            if self.poi().billboard_text.is_none() || !value.is_empty() {
                self.poi_mut().billboard_text = Some(string_into(value));
            }
        } else if attr_name.eq_ignore_ascii_case("title-color") {
            self.poi_mut().billboard_text_color = opt_str(&value).map(parse_color).transpose()?;
        } else if attr_name.eq_ignore_ascii_case("tip-name") {
            self.tip_name = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("tip-description") {
            self.tip_description = Some(string_into(value));
        // === Trail-specific === //
        } else if attr_name.eq_ignore_ascii_case("animspeed") {
            self.trail_mut().anim_speed = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("texture") {
            self.trail_mut().texture = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("trailscale") {
            self.trail_mut().trail_scale = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("iswall") {
            self.trail_mut().is_wall = Some(parse_bool(&value)?);
        // === Filters === //
        } else if attr_name.eq_ignore_ascii_case("festival") {
            self.filters_mut().festivals = Some(parse_into_iter::<Festival, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("mount") {
            self.filters_mut().mounts = Some(parse_into_iter::<Mount, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("profession") {
            self.filters_mut().professions = Some(parse_into_iter::<Profession, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("race") {
            self.filters_mut().races = Some(parse_into_iter::<Race, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("specialization") {
            self.filters_mut().specializations = Some(parse_into_iter::<keys::Specialization, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("maptype") {
            self.filters_mut().map_types = Some(parse_into_iter::<MapType, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("schedule") {
            self.filters_mut().schedule = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("schedule-duration") {
            self.filters_mut().schedule_duration = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("raid") {
            self.filters_mut().raids = Some(parse_into_iter::<keys::Raid, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("achievementid") {
            self.filters_mut().achievement_id = parse_opt(&value)?;
        } else if attr_name.eq_ignore_ascii_case("achievementbit") {
            self.filters_mut().achievement_bit = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("invertbehavior") {
            self.filters_mut().invert_behavior = Some(parse_bool(&value)?);
        // === Taco Behaviors === //
        } else if attr_name.eq_ignore_ascii_case("behavior") {
            self.interaction_mut().taco_behavior = Some(value.parse::<i32>()?.try_into()?);
        } else if attr_name.eq_ignore_ascii_case("resetlength") {
            self.interaction_mut().reset_length = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("autotrigger") {
            self.interaction_mut().auto_trigger = Some(parse_bool(&value)?);
        // === Modifiers === //
        } else if attr_name.eq_ignore_ascii_case("info") {
            self.interaction_mut().info = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("inforange")
            || attr_name.eq_ignore_ascii_case("triggerrange")
        {
            self.interaction_mut().info_range = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("bounce") {
            self.interaction_mut().bounce_behavior = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("bounce-delay") {
            self.interaction_mut().bounce_delay = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("bounce-height") {
            self.interaction_mut().bounce_height = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("bounce-duration") {
            self.interaction_mut().bounce_duration = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("copy") {
            self.interaction_mut().copy_value = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("copy-message") {
            self.interaction_mut().copy_message = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("toggle")
            || attr_name.eq_ignore_ascii_case("togglecategory")
        {
            self.interaction_mut().toggle_category = Some(value.into());
        } else if attr_name.eq_ignore_ascii_case("resetguid") {
            #[cfg(deleteme)]
            let guids = value.split(',').map(|g| taco_xml_to_guid(g.trim_ascii()));
            self.interaction_mut().reset_guids = Some(parse_into_iter::<keys::Guid, _>(&value)?);
        } else if attr_name.eq_ignore_ascii_case("show") {
            self.interaction_mut().show_category = Some(value.into());
        } else if attr_name.eq_ignore_ascii_case("hide") {
            self.interaction_mut().hide_category = Some(value.into());
        // === Scripting === //
        } else if ScriptAttributes::interested_in_key(attr_name) {
            match self.script_mut().try_add(attr_name, value) {
                Ok(()) => (),
                Err(..) => return Ok(false),
            }
        } else {
            return Ok(false)
        }
        Ok(true)
    }

    pub fn script(&self) -> Cow<'_, ScriptAttributes> {
        match &self.script {
            Some(s) => Cow::Borrowed(s),
            None => Cow::Owned(Default::default()),
        }
    }
    pub fn script_mut(&mut self) -> &mut ScriptAttributes {
        self.script.get_or_insert_default()
    }
    pub fn get_interaction_mut(&mut self) -> Option<&mut InteractionAttributes> {
        self.interaction.as_mut().map(Arc::make_mut)
    }
    pub fn interaction(&self) -> Cow<'_, InteractionAttributes> {
        match &self.interaction {
            Some(i) => Cow::Borrowed(i),
            None => Cow::Owned(Default::default()),
        }
    }
    pub fn interaction_mut(&mut self) -> &mut InteractionAttributes {
        Arc::make_mut(self.interaction.get_or_insert_default())
    }
    pub fn filters(&self) -> Cow<'_, FilterAttributes> {
        match &self.filters {
            Some(i) => Cow::Borrowed(i),
            None => Cow::Owned(Default::default()),
        }
    }
    pub fn filters_mut(&mut self) -> &mut FilterAttributes {
        self.filters.get_or_insert_default()
    }
    pub fn render(&self) -> Cow<'_, RenderAttributes> {
        match &self.render {
            Some(i) => Cow::Borrowed(i),
            None => Cow::Owned(Default::default()),
        }
    }
    pub fn get_render_mut(&mut self) -> Option<&mut RenderAttributes> {
        self.render.as_mut().map(Arc::make_mut)
    }
    pub fn render_mut(&mut self) -> &mut RenderAttributes {
        Arc::make_mut(self.render.get_or_insert_default())
    }
    pub fn get_trail(&self) -> Option<&TrailAttributes> {
        self.render
            .as_ref()
            .and_then(|render| render.trail.as_ref())
            .map(|a| &**a)
    }
    pub fn get_trail_mut(&mut self) -> Option<&mut TrailAttributes> {
        self.get_render_mut()
            .and_then(|render| render.trail.as_mut())
            .map(|a| &mut **a)
    }
    pub fn trail(&self) -> Cow<'_, TrailAttributes> {
        match self.get_trail() {
            Some(i) => Cow::Borrowed(i),
            None => Cow::Owned(Default::default()),
        }
    }
    pub fn trail_mut(&mut self) -> &mut TrailAttributes {
        self.render_mut().trail.get_or_insert_default()
    }
    pub fn get_poi(&self) -> Option<&PoiAttributes> {
        self.render
            .as_ref()
            .and_then(|render| render.poi.as_ref())
            .map(|a| &**a)
    }
    pub fn get_poi_mut(&mut self) -> Option<&mut PoiAttributes> {
        self.get_render_mut()
            .and_then(|render| render.poi.as_mut())
            .map(|a| &mut **a)
    }
    pub fn poi(&self) -> Cow<'_, PoiAttributes> {
        match self.get_poi() {
            Some(i) => Cow::Borrowed(i),
            None => Cow::Owned(Default::default()),
        }
    }
    pub fn poi_mut(&mut self) -> &mut PoiAttributes {
        self.render_mut().poi.get_or_insert_default()
    }

    pub fn tip_name(&self) -> Option<&str> {
        self.tip_name.as_ref().and_then(|s| str_opt_ref(&**s))
    }
    pub fn tip_description(&self) -> Option<&str> {
        self.tip_description.as_ref().and_then(|s| str_opt_ref(&**s))
    }
}
impl AsMut<TrailAttributes> for MarkerAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut TrailAttributes {
        self.trail_mut()
    }
}
impl AsMut<PoiAttributes> for MarkerAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut PoiAttributes {
        self.poi_mut()
    }
}
impl AsMut<RenderAttributes> for MarkerAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut RenderAttributes {
        self.render_mut()
    }
}
impl AsMut<FilterAttributes> for MarkerAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut FilterAttributes {
        self.filters_mut()
    }
}
impl AsMut<InteractionAttributes> for MarkerAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut InteractionAttributes {
        self.interaction_mut()
    }
}
impl AsMut<ScriptAttributes> for MarkerAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut ScriptAttributes {
        self.script_mut()
    }
}
impl GetAttrDyn for MarkerAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        RenderAttributes::holds_attr_dyn(key)
            || cell::pack_attr!(=id_is_in(key, [
                keys::EditTag,
                keys::MinimapVisibility,
                keys::MapVisibility,
                keys::InGameVisibility,
                keys::TipName,
                keys::TipDescription,
            ]))
            || FilterAttributes::holds_attr_dyn(key)
            || InteractionAttributes::holds_attr_dyn(key)
            || ScriptAttributes::holds_attr_dyn(key)
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        let has = cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::EditTag,
            keys::MinimapVisibility,
            keys::MapVisibility,
            keys::InGameVisibility,
            keys::TipName,
            keys::TipDescription,
        ] };
        if let Some(has) = has {
            has
        } else if let Some(true) = self.render.as_ref().map(|a| a.has_attr_dyn(key)) {
            true
        } else if RenderAttributes::holds_attr_dyn(key) {
            false
        } else {
            self.filters
                .as_ref()
                .map(|a| a.has_attr_dyn(key))
                .unwrap_or(false)
                || self
                    .interaction
                    .as_ref()
                    .map(|a| a.has_attr_dyn(key))
                    .unwrap_or(false)
                || self.script.as_ref().map(|a| a.has_attr_dyn(key)).unwrap_or(false)
        }
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        let get = cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::EditTag,
            keys::MinimapVisibility,
            keys::MapVisibility,
            keys::InGameVisibility,
            keys::TipName,
            keys::TipDescription,
        ] };
        if let Some(get) = get {
            get
        } else if let Some(Some(render)) = self.render.as_ref().map(|a| a.get_attr_dyn_ref(key)) {
            Some(render)
        } else if RenderAttributes::holds_attr_dyn(key) {
            None
        } else {
            self.filters
                .as_ref()
                .and_then(|a| a.get_attr_dyn_ref(key))
                .or_else(|| self.interaction.as_ref().and_then(|a| a.get_attr_dyn_ref(key)))
                .or_else(|| self.script.as_ref().and_then(|a| a.get_attr_dyn_ref(key)))
        }
    }
    fn get_attr_dyn(&self, key: cell::PackKeyId) -> Option<Cow<'_, dyn cell::AttrKeyValue>> {
        let get = cell::pack_attr! { imp GetAttrDyn::get_attr_dyn(self, key) in [
            keys::EditTag,
            keys::MinimapVisibility,
            keys::MapVisibility,
            keys::InGameVisibility,
            keys::TipName,
            keys::TipDescription,
        ] };
        if let Some(get) = get {
            get
        } else if let Some(Some(render)) = self.render.as_ref().map(|a| a.get_attr_dyn(key)) {
            Some(render)
        } else if RenderAttributes::holds_attr_dyn(key) {
            None
        } else {
            self.filters
                .as_ref()
                .and_then(|a| a.get_attr_dyn(key))
                .or_else(|| self.interaction.as_ref().and_then(|a| a.get_attr_dyn(key)))
                .or_else(|| self.script.as_ref().and_then(|a| a.get_attr_dyn(key)))
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::EditTag,
            keys::MinimapVisibility,
            keys::MapVisibility,
            keys::InGameVisibility,
            keys::TipName,
            keys::TipDescription,
        ] }
        .chain(self.render.as_ref().into_iter().flat_map(|a| a.iter_attrs_dyn()))
        .chain(self.filters.as_ref().into_iter().flat_map(|a| a.iter_attrs_dyn()))
        .chain(
            self.interaction
                .as_ref()
                .into_iter()
                .flat_map(|a| a.iter_attrs_dyn()),
        )
        .chain(self.script.as_ref().into_iter().flat_map(|a| a.iter_attrs_dyn()))
    }
}
impl cell::SetAttrDyn for MarkerAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in
            [
                keys::EditTag,
                keys::MinimapVisibility,
                keys::MapVisibility,
                keys::InGameVisibility,
                keys::TipName,
                keys::TipDescription,
            ],
            _ => {
                if RenderAttributes::holds_attr_dyn(value.id()) {
                    self.render_mut().set_attr_dyn(value)
                } else if FilterAttributes::holds_attr_dyn(value.id()) {
                    self.filters_mut().set_attr_dyn(value)
                } else if ScriptAttributes::holds_attr_dyn(value.id()) {
                    self.script_mut().set_attr_dyn(value)
                } else if self.interaction.is_none() && !InteractionAttributes::holds_attr_dyn(value.id()) {
                    false
                } else {
                    self.interaction_mut().set_attr_dyn(value)
                }
            },
        }
    }
}
cell::pack_attr! {
    impl Attr{keys::EditTag} for &struct{MarkerAttributes}.edit_tag? {}
    impl Attr{keys::MinimapVisibility} for &struct{MarkerAttributes}.minimap_visibility? {}
    impl Attr{keys::MapVisibility} for &struct{MarkerAttributes}.map_visibility? {}
    impl Attr{keys::InGameVisibility} for &struct{MarkerAttributes}.in_game_visibility? {}
    impl Attr{keys::TipName} for &struct{MarkerAttributes}.tip_name? {}
    impl Attr{keys::TipDescription} for &struct{MarkerAttributes}.tip_description? {}

    impl Attr{keys::EditTag} in Internal{MarkerAttributes} {}
    impl Attr{keys::MinimapVisibility} in Internal{MarkerAttributes} {}
    impl Attr{keys::MapVisibility} in Internal{MarkerAttributes} {}
    impl Attr{keys::InGameVisibility} in Internal{MarkerAttributes} {}
    impl Attr{keys::TipName} in Internal{MarkerAttributes} {}
    impl Attr{keys::TipDescription} in Internal{MarkerAttributes} {}
}

/// Scripting.
#[derive(Debug, Clone, Default)]
pub struct ScriptAttributes {
    pub script_tick: Option<AttrString>,
    pub script_focus: Option<AttrString>,
    pub script_trigger: Option<AttrString>,
    pub script_filter: Option<AttrString>,
    pub script_once: Option<AttrString>,
}
impl ScriptAttributes {
    pub fn merge(&mut self, base: &Self) {
        if self.script_tick.is_none() {
            self.script_tick = base.script_tick.clone();
        }
        if self.script_focus.is_none() {
            self.script_focus = base.script_focus.clone();
        }
        if self.script_trigger.is_none() {
            self.script_trigger = base.script_trigger.clone();
        }
        if self.script_filter.is_none() {
            self.script_filter = base.script_filter.clone();
        }
        if self.script_once.is_none() {
            self.script_once = base.script_once.clone();
        }
    }

    const PREFIX: &'static str = "script-";
    pub fn interested_in_key(attr_name: &str) -> bool {
        attr_name
            .get(..Self::PREFIX.len())
            .map(|a| a.eq_ignore_ascii_case(Self::PREFIX))
            .unwrap_or(false)
    }

    pub fn try_add(&mut self, attr_name: &str, value: String) -> Result<(), String> {
        if attr_name.eq_ignore_ascii_case("script-tick") {
            self.script_tick = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("script-focus") {
            self.script_focus = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("script-trigger") {
            self.script_trigger = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("script-filter") {
            self.script_filter = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("script-once") {
            self.script_once = Some(string_into(value));
        } else {
            return Err(value)
        }
        Ok(())
    }
}
impl GetAttrDyn for ScriptAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        cell::pack_attr!(=id_is_in(key, [
            keys::ScriptTick,
            keys::ScriptFocus,
            keys::ScriptTrigger,
            keys::ScriptFilter,
            keys::ScriptOnce,
        ]))
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::ScriptTick,
            keys::ScriptFocus,
            keys::ScriptTrigger,
            keys::ScriptFilter,
            keys::ScriptOnce,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::ScriptTick,
            keys::ScriptFocus,
            keys::ScriptTrigger,
            keys::ScriptFilter,
            keys::ScriptOnce,
        ] }
        .flatten()
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::ScriptTick,
            keys::ScriptFocus,
            keys::ScriptTrigger,
            keys::ScriptFilter,
            keys::ScriptOnce,
        ] }
    }
}
impl cell::SetAttrDyn for ScriptAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::ScriptTick,
            keys::ScriptFocus,
            keys::ScriptTrigger,
            keys::ScriptFilter,
            keys::ScriptOnce,
        ] }
    }
}
cell::pack_attr! {
    impl Attr{keys::ScriptTick} for &struct{ScriptAttributes}.script_tick? {}
    impl Attr{keys::ScriptFocus} for &struct{ScriptAttributes}.script_focus? {	}
    impl Attr{keys::ScriptTrigger} for &struct{ScriptAttributes}.script_trigger? {	}
    impl Attr{keys::ScriptFilter} for &struct{ScriptAttributes}.script_filter? {	}
    impl Attr{keys::ScriptOnce} for &struct{ScriptAttributes}.script_once? {	}

    impl Attr{keys::ScriptTick} in Internal{ScriptAttributes} {}
    impl Attr{keys::ScriptFocus} in Internal{ScriptAttributes} {}
    impl Attr{keys::ScriptTrigger} in Internal{ScriptAttributes} {}
    impl Attr{keys::ScriptFilter} in Internal{ScriptAttributes} {}
    impl Attr{keys::ScriptOnce} in Internal{ScriptAttributes} {}
}

/// Modifiers.
#[derive(Debug, Clone, Default)]
pub struct InteractionAttributes {
    pub info: Option<AttrString>,
    pub info_range: Option<f32>,
    pub bounce_behavior: Option<BounceBehavior>,
    pub bounce_delay: Option<f32>,
    pub bounce_height: Option<f32>,
    pub bounce_duration: Option<f32>,
    pub copy_value: Option<AttrString>,
    pub copy_message: Option<AttrString>,
    pub toggle_category: Option<IdNameBox>,
    pub reset_guids: Option<keys::ResetGuid>,
    pub show_category: Option<IdNameBox>,
    pub hide_category: Option<IdNameBox>,
    /// Taco Behaviors.
    pub taco_behavior: Option<TacoBehavior>,
    pub reset_length: Option<f32>,
    pub auto_trigger: Option<bool>,
}

impl InteractionAttributes {
    pub fn merge(&mut self, base: &Self, child: bool) {
        if self.info.is_none() {
            self.info = base.info.clone();
        }
        if self.info_range.is_none() {
            self.info_range = base.info_range;
        }
        if self.bounce_behavior.is_none() {
            self.bounce_behavior = base.bounce_behavior;
        }
        if self.bounce_delay.is_none() {
            self.bounce_delay = base.bounce_delay;
        }
        if self.bounce_height.is_none() {
            self.bounce_height = base.bounce_height;
        }
        if self.bounce_duration.is_none() {
            self.bounce_duration = base.bounce_duration;
        }
        if !child && self.copy_value.is_none() {
            self.copy_value = base.copy_value.clone();
        }
        if !child && self.copy_message.is_none() {
            self.copy_message = base.copy_message.clone();
        }
        if self.toggle_category.is_none() {
            self.toggle_category = base.toggle_category.clone();
        }
        if self.reset_guids.is_none() {
            self.reset_guids = base.reset_guids.clone();
        }
        if self.show_category.is_none() {
            self.show_category = base.show_category.clone();
        }
        if self.hide_category.is_none() {
            self.hide_category = base.hide_category.clone();
        }
        // === Taco Behaviors === //
        if self.taco_behavior.is_none() {
            self.taco_behavior = base.taco_behavior;
        }
        if self.reset_length.is_none() {
            self.reset_length = base.reset_length;
        }
        if self.auto_trigger.is_none() {
            self.auto_trigger = base.auto_trigger;
        }
    }

    pub fn marker_can_inherit(&self, child: bool) -> bool {
        match (&self.copy_value, &self.copy_message) {
            (Some(..), _) | (_, Some(..)) if child => false,
            _ => true,
        }
    }
}
impl GetAttrDyn for InteractionAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        cell::pack_attr!(=id_is_in(key, [
            keys::Info,
            keys::InfoRange,
            keys::TriggerRange,
            keys::Bounce,
            keys::BounceDelay,
            keys::BounceHeight,
            keys::BounceDuration,
            keys::CopyValue,
            keys::CopyMessage,
            keys::ToggleCategory,
            keys::ResetGuid,
            keys::ShowCategory,
            keys::HideCategory,
            keys::Behaviour,
            keys::ResetLength,
            keys::AutoTrigger,
        ]))
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::Info,
            keys::InfoRange,
            keys::TriggerRange,
            keys::Bounce,
            keys::BounceDelay,
            keys::BounceHeight,
            keys::BounceDuration,
            keys::CopyValue,
            keys::CopyMessage,
            keys::ToggleCategory,
            keys::ResetGuid,
            keys::ShowCategory,
            keys::HideCategory,
            keys::Behaviour,
            keys::ResetLength,
            keys::AutoTrigger,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::Info,
            keys::InfoRange,
            keys::TriggerRange,
            keys::Bounce,
            keys::BounceDelay,
            keys::BounceHeight,
            keys::BounceDuration,
            keys::CopyValue,
            keys::CopyMessage,
            keys::ToggleCategory,
            keys::ResetGuid,
            keys::ShowCategory,
            keys::HideCategory,
            keys::Behaviour,
            keys::ResetLength,
            keys::AutoTrigger,
        ] }
        .flatten()
    }
    fn get_attr_dyn(&self, key: cell::PackKeyId) -> Option<Cow<'_, dyn cell::AttrKeyValue>> {
        let get = cell::pack_attr! { imp GetAttrDyn::get_attr_dyn(self, key) in [
            keys::Behaviour,
            keys::ResetGuid,
        ] };
        if let Some(get) = get {
            get
        } else {
            self.get_attr_dyn_ref(key).map(Cow::Borrowed)
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::Info,
            keys::InfoRange,
            keys::TriggerRange,
            keys::Bounce,
            keys::BounceDelay,
            keys::BounceHeight,
            keys::BounceDuration,
            keys::CopyValue,
            keys::CopyMessage,
            keys::ToggleCategory,
            keys::ResetGuid,
            keys::ShowCategory,
            keys::HideCategory,
            keys::Behaviour,
            keys::ResetLength,
            keys::AutoTrigger,
        ] }
    }
}
impl cell::SetAttrDyn for InteractionAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::Info,
            keys::InfoRange,
            keys::TriggerRange,
            keys::Bounce,
            keys::BounceDelay,
            keys::BounceHeight,
            keys::BounceDuration,
            keys::CopyValue,
            keys::CopyMessage,
            keys::ToggleCategory,
            keys::ResetGuid,
            keys::ShowCategory,
            keys::HideCategory,
            keys::Behaviour,
            keys::ResetLength,
            keys::AutoTrigger,
        ] }
    }
}
cell::pack_attr! {
    impl Attr{keys::Info} for &struct{InteractionAttributes}.info? {}
    impl Attr{keys::InfoRange} for &struct{InteractionAttributes}.info_range? {}
    // TODO: separate field
    impl Attr{keys::TriggerRange} for &struct{InteractionAttributes}.info_range? {}
    impl Attr{keys::Bounce} for &struct{InteractionAttributes}.bounce_behavior? {}
    impl Attr{keys::BounceDelay} for &struct{InteractionAttributes}.bounce_delay? {}
    impl Attr{keys::BounceHeight} for &struct{InteractionAttributes}.bounce_height? {}
    impl Attr{keys::BounceDuration} for &struct{InteractionAttributes}.bounce_duration? {}
    impl Attr{keys::CopyValue} for &struct{InteractionAttributes}.copy_value? {}
    impl Attr{keys::CopyMessage} for &struct{InteractionAttributes}.copy_message? {}
    impl Attr{keys::ToggleCategory} for &struct{InteractionAttributes}.toggle_category? {}
    impl Attr{keys::ShowCategory} for &struct{InteractionAttributes}.show_category? {}
    impl Attr{keys::HideCategory} for &struct{InteractionAttributes}.hide_category? {}
    impl Attr{keys::ResetLength} for &struct{InteractionAttributes}.reset_length? {}
    impl Attr{keys::ResetGuid} for &struct{InteractionAttributes}.reset_guids? {}
    impl Attr{keys::AutoTrigger} for &struct{InteractionAttributes}.auto_trigger? {}

    impl Attr{keys::Info} in Internal{InteractionAttributes} {}
    impl Attr{keys::InfoRange} in Internal{InteractionAttributes} {}
    impl Attr{keys::TriggerRange} in Internal{InteractionAttributes} {}
    impl Attr{keys::Bounce} in Internal{InteractionAttributes} {}
    impl Attr{keys::BounceDelay} in Internal{InteractionAttributes} {}
    impl Attr{keys::BounceHeight} in Internal{InteractionAttributes} {}
    impl Attr{keys::BounceDuration} in Internal{InteractionAttributes} {}
    impl Attr{keys::CopyValue} in Internal{InteractionAttributes} {}
    impl Attr{keys::CopyMessage} in Internal{InteractionAttributes} {}
    impl Attr{keys::ToggleCategory} in Internal{InteractionAttributes} {}
    impl Attr{keys::ShowCategory} in Internal{InteractionAttributes} {}
    impl Attr{keys::HideCategory} in Internal{InteractionAttributes} {}
    impl Attr{keys::Behaviour} in Internal{InteractionAttributes} {}
    impl Attr{keys::ResetGuid} in Internal{InteractionAttributes} {}
    impl Attr{keys::ResetLength} in Internal{InteractionAttributes} {}
    impl Attr{keys::AutoTrigger} in Internal{InteractionAttributes} {}
}
impl keys::GetAttr<keys::Behaviour> for InteractionAttributes {
    fn has_attr(&self) -> bool {
        self.taco_behavior.is_some()
    }
    fn get_attr(&self) -> Option<Cow<'_, keys::Behaviour>> {
        self.taco_behavior.map(keys::Behaviour::from).map(Cow::Owned)
    }
    #[cfg(todo)]
    fn get_attr_ref(&self) -> Option<&keys::Behaviour> {}
}
impl keys::SetAttr<keys::Behaviour> for InteractionAttributes {
    fn set_attr(&mut self, value: keys::Behaviour) {
        self.taco_behavior = Some(value.into());
    }
    fn unset_attr(&mut self) {
        self.taco_behavior = None;
    }
}

/// Filters.
#[derive(Debug, Clone, Default)]
pub struct FilterAttributes {
    pub festivals: Option<Festivals>,
    pub mounts: Option<Mounts>,
    pub professions: Option<Professions>,
    pub races: Option<Races>,
    pub specializations: Option<keys::Specializations>,
    pub map_types: Option<keys::MapTypes>,
    pub schedule: Option<AttrString>,
    pub schedule_duration: Option<f32>,
    pub raids: Option<keys::Raids>,
    pub achievement_id: Option<i32>,
    pub achievement_bit: Option<i32>,
    pub invert_behavior: Option<bool>,
}
impl FilterAttributes {
    pub fn merge(&mut self, base: &Self) {
        if self.festivals.is_none() {
            self.festivals = base.festivals.clone();
        }
        if self.mounts.is_none() {
            self.mounts = base.mounts.clone();
        }
        if self.professions.is_none() {
            self.professions = base.professions.clone();
        }
        if self.races.is_none() {
            self.races = base.races.clone();
        }
        if self.specializations.is_none() {
            self.specializations = base.specializations.clone();
        }
        if self.map_types.is_none() {
            self.map_types = base.map_types.clone();
        }
        if self.schedule.is_none() {
            self.schedule = base.schedule.clone();
        }
        if self.schedule_duration.is_none() {
            self.schedule_duration = base.schedule_duration;
        }
        if self.raids.is_none() {
            self.raids = base.raids.clone();
        }
        if self.achievement_id.is_none() {
            self.achievement_id = base.achievement_id;
        }
        if self.achievement_bit.is_none() {
            self.achievement_bit = base.achievement_bit;
        }
        if self.invert_behavior.is_none() {
            self.invert_behavior = base.invert_behavior;
        }
    }
}
/// TODO: flag sets
impl GetAttrDyn for FilterAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        cell::pack_attr!(=id_is_in(key, [
            keys::AchievementId,
            keys::AchievementBit,
            keys::InvertBehaviour,
            keys::ScheduleStart,
            keys::ScheduleDuration,
            keys::Festivals, keys::Mounts, keys::Races, keys::Professions,
            keys::Specializations, keys::Raids, keys::MapTypes,
            // TODO? Festival, Mount, Race, Profession, keys::Specialization, keys::Raid, MapType,
        ]))
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::AchievementId,
            keys::AchievementBit,
            keys::InvertBehaviour,
            keys::ScheduleStart,
            keys::ScheduleDuration,
            keys::Festivals, keys::Mounts, keys::Races, keys::Professions,
            keys::Specializations, keys::Raids, keys::MapTypes,
            // TODO? Festival, Mount, Race, Profession, keys::Specialization, keys::Raid, MapType,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::AchievementId,
            keys::AchievementBit,
            keys::InvertBehaviour,
            keys::ScheduleStart,
            keys::ScheduleDuration,
            keys::Festivals, keys::Mounts, keys::Races, keys::Professions,
            keys::Specializations, keys::Raids, keys::MapTypes,
            // TODO? Festival, Mount, Race, Profession, keys::Specialization, keys::Raid, MapType,
        ] }
        .flatten()
    }
    /// TODO: flag sets
    #[cfg(todo)]
    fn get_attr_dyn(&self, key: cell::PackKeyId) -> Option<Cow<'_, dyn cell::AttrKeyValue>> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn(self, key) in [
            // TODO? Festival, Mount, Race, Profession, keys::Specialization, keys::Raid, MapType,
        ] }.map(Cow::Owned).or_else(|| self.get_attr_dyn_ref(key).map(Cow::Borrowed))
    }
    /// TODO? chain individual flags?
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::AchievementId,
            keys::AchievementBit,
            keys::InvertBehaviour,
            keys::ScheduleStart,
            keys::ScheduleDuration,
            keys::Festivals, keys::Mounts, keys::Races, keys::Professions,
            keys::Specializations, keys::Raids, keys::MapTypes,
        ] }
    }
}
impl cell::SetAttrDyn for FilterAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in
            [
                keys::AchievementId,
                keys::AchievementBit,
                keys::InvertBehaviour,
                keys::ScheduleStart,
                keys::ScheduleDuration,
                keys::Festivals, keys::Mounts, keys::Races, keys::Professions,
                keys::Specializations, keys::Raids, keys::MapTypes,
                //Festival, Mount, Race, Profession, keys::Specialization, keys::Raid, MapType,
            ],
            _ => {
                #[cfg(taimi_debug)]
                log::debug!("TODO: FilterAttributes::set_attr_dyn({})", value.id());
                false
            },
        }
    }
}
cell::pack_attr! {
    impl Attr{keys::AchievementId} for &struct{FilterAttributes}.achievement_id? {}
    impl Attr{keys::AchievementBit} for &struct{FilterAttributes}.achievement_bit? {}
    impl Attr{keys::InvertBehaviour} for &struct{FilterAttributes}.invert_behavior? {}
    impl Attr{keys::ScheduleStart} for &struct{FilterAttributes}.schedule? {}
    impl Attr{keys::ScheduleDuration} for &struct{FilterAttributes}.schedule_duration? {}
    impl Attr{keys::Festivals} for &struct{FilterAttributes}.festivals? {}
    impl Attr{keys::Mounts} for &struct{FilterAttributes}.mounts? {}
    impl Attr{keys::Races} for &struct{FilterAttributes}.races? {}
    impl Attr{keys::Professions} for &struct{FilterAttributes}.professions? {}
    impl Attr{keys::Specializations} for &struct{FilterAttributes}.specializations? {}
    impl Attr{keys::Raids} for &struct{FilterAttributes}.raids? {}
    impl Attr{keys::MapTypes} for &struct{FilterAttributes}.map_types? {}

    impl Attr{keys::AchievementId} in Internal{FilterAttributes} {}
    impl Attr{keys::AchievementBit} in Internal{FilterAttributes} {}
    impl Attr{keys::InvertBehaviour} in Internal{FilterAttributes} {}
    impl Attr{keys::ScheduleStart} in Internal{FilterAttributes} {}
    impl Attr{keys::ScheduleDuration} in Internal{FilterAttributes} {}
    impl Attr{keys::Festivals} in Internal{FilterAttributes} {}
    impl Attr{keys::Mounts} in Internal{FilterAttributes} {}
    impl Attr{keys::Races} in Internal{FilterAttributes} {}
    impl Attr{keys::Professions} in Internal{FilterAttributes} {}
    impl Attr{keys::Specializations} in Internal{FilterAttributes} {}
    impl Attr{keys::Raids} in Internal{FilterAttributes} {}
    impl Attr{keys::MapTypes} in Internal{FilterAttributes} {}
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RenderAttributes {
    pub alpha: Option<f32>,
    pub can_fade: Option<bool>,
    pub tint: Option<Vec4>,
    pub cull: Option<CullDirection>,
    pub fade_near: Option<f32>,
    pub fade_far: Option<f32>,
    pub trail: Option<Box<TrailAttributes>>,
    pub poi: Option<Box<PoiAttributes>>,
}
impl RenderAttributes {
    pub fn merge(&mut self, base: &Self) {
        if self.alpha.is_none() {
            self.alpha = base.alpha;
        }
        if self.can_fade.is_none() {
            self.can_fade = base.can_fade;
        }
        if self.tint.is_none() {
            self.tint = base.tint;
        }
        if self.cull.is_none() {
            self.cull = base.cull;
        }
        if self.fade_near.is_none() {
            self.fade_near = base.fade_near;
        }
        if self.fade_far.is_none() {
            self.fade_far = base.fade_far;
        }
        // === POI-specific === //
        match (&base.poi, &mut self.poi) {
            (Some(base), poi @ None) => *poi = Some(base.clone()),
            (Some(base), Some(poi)) => poi.merge(base),
            (None, _) => (),
        }
        // === Trail-specific === //
        match (&base.trail, &mut self.trail) {
            (Some(base), trail @ None) => *trail = Some(base.clone()),
            (Some(base), Some(trail)) => trail.merge(base),
            (None, _) => (),
        }
    }

    #[inline]
    pub fn tint(&self) -> Vec4 {
        self.tint.unwrap_or(Vec4::ONE)
    }

    #[inline]
    pub fn alpha(&self) -> f32 {
        self.alpha.unwrap_or(1.0)
    }
}
impl AsMut<TrailAttributes> for RenderAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut TrailAttributes {
        self.trail.get_or_insert_default()
    }
}
impl AsMut<PoiAttributes> for RenderAttributes {
    #[inline]
    fn as_mut(&mut self) -> &mut PoiAttributes {
        self.poi.get_or_insert_default()
    }
}
impl GetAttrDyn for RenderAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        cell::pack_attr!(=id_is_in(key, [
            keys::Alpha,
            keys::CanFade,
            keys::Tint,
            keys::Cull,
            keys::FadeNear,
            keys::FadeFar,
        ])) || PoiAttributes::holds_attr_dyn(key)
            || TrailAttributes::holds_attr_dyn(key)
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::Alpha,
            keys::CanFade,
            keys::Tint,
            keys::Cull,
            keys::FadeNear,
            keys::FadeFar,
        ] }
        .unwrap_or_else(|| {
            self.trail.as_ref().map(|a| a.has_attr_dyn(key)).unwrap_or(false)
                || self.poi.as_ref().map(|a| a.has_attr_dyn(key)).unwrap_or(false)
        })
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::Alpha,
            keys::CanFade,
            keys::Tint,
            keys::Cull,
            keys::FadeNear,
            keys::FadeFar,
        ] }
        .or_else(|| {
            self.poi
                .as_ref()
                .and_then(|poi| PoiAttributes::holds_attr_dyn(key).then(|| poi.get_attr_dyn_ref(key)))
        })
        .or_else(|| {
            self.trail
                .as_ref()
                .and_then(|trail| TrailAttributes::holds_attr_dyn(key).then(|| trail.get_attr_dyn_ref(key)))
        })
        .flatten()
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::Alpha,
            keys::CanFade,
            keys::Tint,
            keys::Cull,
            keys::FadeNear,
            keys::FadeFar,
        ] }
        .chain(self.poi.as_ref().into_iter().flat_map(|a| a.iter_attrs_dyn()))
        .chain(self.trail.as_ref().into_iter().flat_map(|a| a.iter_attrs_dyn()))
    }
}
impl cell::SetAttrDyn for RenderAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in
            [
                keys::Alpha,
                keys::CanFade,
                keys::Tint,
                keys::Cull,
                keys::FadeNear,
                keys::FadeFar,
            ],
            _ => if PoiAttributes::holds_attr_dyn(value.id()) {
                self.poi.get_or_insert_default()
                    .set_attr_dyn(value)
            } else {
                match &mut self.trail {
                    None if !TrailAttributes::holds_attr_dyn(value.id()) =>
                        false,
                    trail => trail.get_or_insert_default()
                        .set_attr_dyn(value),
                }
            },
        }
    }
}
cell::pack_attr! {
    impl Attr{keys::Alpha} for &struct{RenderAttributes}.alpha? {}
    impl Attr{keys::CanFade} for &struct{RenderAttributes}.can_fade? {}
    impl Attr{keys::Tint} for &struct{RenderAttributes}.tint? {}
    impl Attr{keys::Cull} for &struct{RenderAttributes}.cull? {}
    impl Attr{keys::FadeNear} for &struct{RenderAttributes}.fade_near? {}
    impl Attr{keys::FadeFar} for &struct{RenderAttributes}.fade_far? {}

    impl Attr{keys::Alpha} in Internal{RenderAttributes} {}
    impl Attr{keys::CanFade} in Internal{RenderAttributes} {}
    impl Attr{keys::Tint} in Internal{RenderAttributes} {}
    impl Attr{keys::Cull} in Internal{RenderAttributes} {}
    impl Attr{keys::FadeNear} in Internal{RenderAttributes} {}
    impl Attr{keys::FadeFar} in Internal{RenderAttributes} {}
}

/// Trail-specific.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrailAttributes {
    pub anim_speed: Option<f32>,
    pub texture: Option<AttrString>,
    pub trail_scale: Option<f32>,
    pub is_wall: Option<bool>,
    pub tint_map: Option<Vec4>,
}
impl TrailAttributes {
    pub fn merge(&mut self, base: &Self) {
        if self.anim_speed.is_none() {
            self.anim_speed = base.anim_speed;
        }
        if self.texture.is_none() {
            self.texture = base.texture.clone();
        }
        if self.trail_scale.is_none() {
            self.trail_scale = base.trail_scale;
        }
        if self.is_wall.is_none() {
            self.is_wall = base.is_wall;
        }
        if self.tint_map.is_none() {
            self.tint_map = base.tint_map;
        }
    }
}
impl GetAttrDyn for TrailAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        cell::pack_attr!(=id_is_in(key, [
            keys::AnimSpeed,
            keys::TextureFile,
            keys::TrailScale,
            keys::IsWall,
            keys::MapTint,
        ]))
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::AnimSpeed,
            keys::TextureFile,
            keys::TrailScale,
            keys::IsWall,
            keys::MapTint,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::AnimSpeed,
            keys::TextureFile,
            keys::TrailScale,
            keys::IsWall,
            keys::MapTint,
        ] }
        .flatten()
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::AnimSpeed,
            keys::TextureFile,
            keys::TrailScale,
            keys::IsWall,
            keys::MapTint,
        ] }
    }
}
impl cell::SetAttrDyn for TrailAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::AnimSpeed,
            keys::TextureFile,
            keys::TrailScale,
            keys::IsWall,
            keys::MapTint,
        ] }
    }
}
cell::pack_attr! {
    impl Attr{keys::AnimSpeed} for &struct{TrailAttributes}.anim_speed? {}
    impl Attr{keys::TextureFile} for &struct{TrailAttributes}.texture? {}
    impl Attr{keys::TrailScale} for &struct{TrailAttributes}.trail_scale? {}
    impl Attr{keys::IsWall} for &struct{TrailAttributes}.is_wall? {}
    impl Attr{keys::MapTint} for &struct{TrailAttributes}.tint_map? {}

    impl Attr{keys::AnimSpeed} in Internal{TrailAttributes} {}
    impl Attr{keys::TextureFile} in Internal{TrailAttributes} {}
    impl Attr{keys::TrailScale} in Internal{TrailAttributes} {}
    impl Attr{keys::IsWall} in Internal{TrailAttributes} {}
    impl Attr{keys::MapTint} in Internal{TrailAttributes} {}
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PoiAttributes {
    pub height_offset: Option<f32>,
    pub icon_file: Option<AttrString>,
    pub icon_size: Option<f32>,
    pub map_display_size: Option<f32>,
    pub scale_on_map_with_zoom: Option<bool>,
    pub min_size: Option<f32>,
    pub max_size: Option<f32>,
    pub occlude: Option<bool>,
    pub rotate: Option<Vec3>,
    pub billboard_text: Option<AttrString>,
    pub billboard_text_color: Option<Vec4>,
}
impl PoiAttributes {
    pub fn merge(&mut self, base: &Self) {
        if self.height_offset.is_none() {
            self.height_offset = base.height_offset;
        }
        if self.icon_file.is_none() {
            self.icon_file = base.icon_file.clone();
        }
        if self.icon_size.is_none() {
            self.icon_size = base.icon_size;
        }
        if self.map_display_size.is_none() {
            self.map_display_size = base.map_display_size;
        }
        if self.scale_on_map_with_zoom.is_none() {
            self.scale_on_map_with_zoom = base.scale_on_map_with_zoom;
        }
        if self.min_size.is_none() {
            self.min_size = base.min_size;
        }
        if self.max_size.is_none() {
            self.max_size = base.max_size;
        }
        if self.occlude.is_none() {
            self.occlude = base.occlude;
        }
        if let Some(rotate) = base.rotate {
            let dest = self.rotate.get_or_insert(Self::ROTATE_UNSET);
            let dest_axis = [&mut dest.x, &mut dest.y, &mut dest.z];
            for (rotate, dest) in rotate.to_array().iter().zip(dest_axis) {
                if dest.to_bits() == Self::ROTATE_UNSET_AXIS.to_bits() {
                    *dest = *rotate;
                }
            }
        }
        if self.billboard_text.is_none() {
            self.billboard_text = base.billboard_text.clone();
        }
        if self.billboard_text_color.is_none() {
            self.billboard_text_color = base.billboard_text_color;
        }
    }

    pub const ROTATE_UNSET_AXIS: f32 = -0.0;
    pub const ROTATE_UNSET: Vec3 = Vec3::new(
        Self::ROTATE_UNSET_AXIS,
        Self::ROTATE_UNSET_AXIS,
        Self::ROTATE_UNSET_AXIS,
    );
}
impl GetAttrDyn for PoiAttributes {
    fn holds_attr_dyn(key: cell::PackKeyId) -> bool {
        cell::pack_attr!(=id_is_in(key, [
            keys::HeightOffset,
            keys::IconFile,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::ScaleOnMapWithZoom,
            keys::MinSize,
            keys::MaxSize,
            keys::Occlude,
            keys::Rotate,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Title, //keys::Text,
            keys::TitleColour,
        ]))
    }
    fn has_attr_dyn(&self, key: cell::PackKeyId) -> bool {
        cell::pack_attr! { imp GetAttrDyn::has_attr_dyn(self, key) in [
            keys::HeightOffset,
            keys::IconFile,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::ScaleOnMapWithZoom,
            keys::MinSize,
            keys::MaxSize,
            keys::Occlude,
            keys::Rotate,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Title, //keys::Text,
            keys::TitleColour,
        ] }
        .unwrap_or(false)
    }
    fn get_attr_dyn_ref(&self, key: cell::PackKeyId) -> Option<&dyn cell::AttrKeyValue> {
        cell::pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::HeightOffset,
            keys::IconFile,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::ScaleOnMapWithZoom,
            keys::MinSize,
            keys::MaxSize,
            keys::Occlude,
            keys::Rotate,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Title, //keys::Text,
            keys::TitleColour,
        ] }
        .flatten()
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn cell::AttrKeyValue>> + '_ {
        cell::pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::HeightOffset,
            keys::IconFile,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::ScaleOnMapWithZoom,
            keys::MinSize,
            keys::MaxSize,
            keys::Occlude,
            keys::Rotate,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Title, //keys::Text,
            keys::TitleColour,
        ] }
    }
}
impl cell::SetAttrDyn for PoiAttributes {
    fn set_attr_dyn(&mut self, value: cell::PackValueCell) -> bool {
        cell::pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in [
            keys::HeightOffset,
            keys::IconFile,
            keys::IconSize,
            keys::MapDisplaySize,
            keys::ScaleOnMapWithZoom,
            keys::MinSize,
            keys::MaxSize,
            keys::Occlude,
            keys::Rotate,
            keys::RotateX, keys::RotateY, keys::RotateZ,
            keys::Title, //keys::Text,
            keys::TitleColour,
        ] }
    }
}
cell::pack_attr! {
    impl Attr{keys::HeightOffset} for &struct{PoiAttributes}.height_offset? {}
    impl Attr{keys::IconFile} for &struct{PoiAttributes}.icon_file? {}
    impl Attr{keys::IconSize} for &struct{PoiAttributes}.icon_size? {}
    impl Attr{keys::MapDisplaySize} for &struct{PoiAttributes}.map_display_size? {}
    impl Attr{keys::ScaleOnMapWithZoom} for &struct{PoiAttributes}.scale_on_map_with_zoom? {}
    impl Attr{keys::MinSize} for &struct{PoiAttributes}.min_size? {}
    impl Attr{keys::MaxSize} for &struct{PoiAttributes}.max_size? {}
    impl Attr{keys::Occlude} for &struct{PoiAttributes}.occlude? {}
    impl Attr{keys::Rotate} for &struct{PoiAttributes}.rotate? {}
    impl Attr{keys::Title} for &struct{PoiAttributes}.billboard_text? {}
    impl Attr{keys::TitleColour} for &struct{PoiAttributes}.billboard_text_color? {}

    impl Attr{keys::HeightOffset} in Internal{PoiAttributes} {}
    impl Attr{keys::IconFile} in Internal{PoiAttributes} {}
    impl Attr{keys::IconSize} in Internal{PoiAttributes} {}
    impl Attr{keys::MapDisplaySize} in Internal{PoiAttributes} {}
    impl Attr{keys::ScaleOnMapWithZoom} in Internal{PoiAttributes} {}
    impl Attr{keys::MinSize} in Internal{PoiAttributes} {}
    impl Attr{keys::MaxSize} in Internal{PoiAttributes} {}
    impl Attr{keys::Occlude} in Internal{PoiAttributes} {}
    impl Attr{keys::Rotate} in Internal{PoiAttributes} {}
    impl Attr{keys::RotateX} in Internal{PoiAttributes} {}
    impl Attr{keys::RotateY} in Internal{PoiAttributes} {}
    impl Attr{keys::RotateZ} in Internal{PoiAttributes} {}
    impl Attr{keys::Title} in Internal{PoiAttributes} {}
    impl Attr{keys::TitleColour} in Internal{PoiAttributes} {}
}
impl keys::GetAttr<keys::RotateX> for PoiAttributes {
    fn has_attr(&self) -> bool {
        self.rotate.is_some()
    }
    fn get_attr_ref(&self) -> Option<&keys::RotateX> {
        self.rotate.as_ref().map(|rot| keys::RotateX::from_ref(&rot.x))
    }
}
impl keys::SetAttr<keys::RotateX> for PoiAttributes {
    fn set_attr(&mut self, value: keys::RotateX) {
        self.rotate.get_or_insert_default().x = value.into();
    }
    fn unset_attr(&mut self) {
        let is_empty = if let Some(rot) = &mut self.rotate {
            rot.x = Default::default();
            *rot == Vec3::ZERO
        } else {
            false
        };
        if is_empty {
            self.rotate = None
        }
    }
}
impl keys::GetAttr<keys::RotateY> for PoiAttributes {
    fn has_attr(&self) -> bool {
        self.rotate.is_some()
    }
    fn get_attr_ref(&self) -> Option<&keys::RotateY> {
        self.rotate.as_ref().map(|rot| keys::RotateY::from_ref(&rot.y))
    }
}
impl keys::SetAttr<keys::RotateY> for PoiAttributes {
    fn set_attr(&mut self, value: keys::RotateY) {
        self.rotate.get_or_insert_default().y = value.into();
    }
    fn unset_attr(&mut self) {
        let is_empty = if let Some(rot) = &mut self.rotate {
            rot.y = Default::default();
            *rot == Vec3::ZERO
        } else {
            false
        };
        if is_empty {
            self.rotate = None
        }
    }
}
impl keys::GetAttr<keys::RotateZ> for PoiAttributes {
    fn has_attr(&self) -> bool {
        self.rotate.is_some()
    }
    fn get_attr_ref(&self) -> Option<&keys::RotateZ> {
        self.rotate.as_ref().map(|rot| keys::RotateZ::from_ref(&rot.z))
    }
}
impl keys::SetAttr<keys::RotateZ> for PoiAttributes {
    fn set_attr(&mut self, value: keys::RotateZ) {
        self.rotate.get_or_insert_default().z = value.into();
    }
    fn unset_attr(&mut self) {
        let is_empty = if let Some(rot) = &mut self.rotate {
            rot.z = Default::default();
            *rot == Vec3::ZERO
        } else {
            false
        };
        if is_empty {
            self.rotate = None
        }
    }
}

// TODO: move parse helpers into a separate file and make pub

pub fn parse_bool(value: &str) -> anyhow::Result<bool> {
    match value {
        value if value.eq_ignore_ascii_case("true") => Ok(true),
        value if value.eq_ignore_ascii_case("false") => Ok(false),
        value => value
            .parse::<i32>()
            .map(|i| i != 0)
            .map_err(|_| anyhow!("unexpected bool `{value}`")),
    }
}

pub fn parse_opt<T: FromStr>(value: &str) -> Result<Option<T>, T::Err> {
    opt_str(value).map(T::from_str).transpose()
}

fn opt_str(value: &str) -> Option<&str> {
    match value.is_empty() {
        false => Some(value),
        true => None,
    }
}

fn parse_color(value: &str) -> anyhow::Result<Vec4> {
    let val = value.trim_start_matches('#');
    let mut itint = u32::from_str_radix(val, 16)?;
    if val.len() == 6 {
        itint |= 0xFF000000;
    }
    Ok(Vec4::new(
        ((itint >> 16) & 0xFF) as f32 / 255.0,
        ((itint >> 8) & 0xFF) as f32 / 255.0,
        ((itint >> 0) & 0xFF) as f32 / 255.0,
        ((itint >> 24) & 0xFF) as f32 / 255.0,
    ))
}

#[cfg(todo = "unused")]
fn parse_list<T: FromStr>(value: &str) -> anyhow::Result<Vec<T>>
where
    <T as FromStr>::Err: fmt::Display + Into<anyhow::Error>,
{
    let mut err = None;
    let list: Vec<T> = value
        .split(',')
        .map(|f| f.trim_ascii())
        .filter_map(|f| match f.parse() {
            Ok(v) => Some(v),
            Err(e) => {
                if let Some(e) = err.replace(e) {
                    log::error!("unrecognized item `{f}` in list `{value}`: {e}");
                }
                None
            },
        })
        .collect();

    match err {
        Some(e) if list.is_empty() => Err(e.into()),
        _ => Ok(list),
    }
}

#[cfg(todo = "unused")]
fn parse_array<const N: usize, T: FromStr>(value: &str) -> anyhow::Result<[T; N]>
where
    T: Default + Copy,
    <T as FromStr>::Err: Into<anyhow::Error>,
{
    let mut list = [T::default(); N];
    parse_into_array(&mut list, value).map(move |()| list)
}
fn parse_into_array<const N: usize, T: FromStr>(list: &mut [T; N], value: &str) -> anyhow::Result<()>
where
    <T as FromStr>::Err: Into<anyhow::Error>,
{
    let values = value.split(',').map(|f| f.trim_ascii()).map(FromStr::from_str);
    for (dest, item) in list.iter_mut().zip(values) {
        *dest = item
            .map_err(Into::into)
            .with_context(|| format!("parsing list `{value}`"))?;
    }

    Ok(())
}
fn parse_into_iter<T, O>(value: &str) -> anyhow::Result<O>
where
    T: FromStr,
    <T as FromStr>::Err: Into<anyhow::Error>,
    O: FromIterator<T>,
{
    value.split(',').map(|f| f.trim_ascii()).map(|f|
        f.parse::<T>()
            .map_err(Into::into)
    ).collect::<anyhow::Result<O>>()
        .with_context(|| format!("parsing list `{value}`"))
}

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CullDirection {
    #[default]
    None = 0,
    Clockwise = 1,
    CounterClockwise = 2,
}
impl CullDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Clockwise => "clockwise",
            Self::CounterClockwise => "counterclockwise",
        }
    }
}

impl FromStr for CullDirection {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() || s.eq_ignore_ascii_case("none") {
            Ok(CullDirection::None)
        } else if s.eq_ignore_ascii_case("clockwise") {
            Ok(CullDirection::Clockwise)
        } else if s.eq_ignore_ascii_case("counterclockwise") {
            Ok(CullDirection::CounterClockwise)
        } else {
            Err(anyhow!("unexpected cull direction `{s}`"))
        }
    }
}
impl fmt::Display for CullDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
#[cfg(todo)]
impl TryFrom<i32> for MapType {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        Ok(match value {
            0 => Self::None,
            1 => Self::Clockwise,
            2 => Self::CounterClockwise,
            _ => {
                anyhow::bail!("unknown cull direction `{value}`");
            },
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MapType {
    Unknown = -1,
    Redirect = 0,
    CharacterCreate = 1,
    Pvp = 2,
    Gvg = 3,
    Instance = 4,
    Public = 5,
    Tournament = 6,
    Tutorial = 7,
    UserTournament = 8,
    EternalBattlegrounds = 9,
    BlueHome = 10,
    GreenHome = 11,
    RedHome = 12,
    FortunesVale = 13,
    ObsidianSanctum = 14,
    EdgeOfTheMists = 15,
    PublicMini = 16,
    BigBattle = 17,
    WvwLounge = 18,
}

impl TryFrom<i32> for MapType {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use MapType::*;
        Ok(match value {
            -1 => Unknown,
            0 => Redirect,
            1 => CharacterCreate,
            2 => Pvp,
            3 => Gvg,
            4 => Instance,
            5 => Public,
            6 => Tournament,
            7 => Tutorial,
            8 => UserTournament,
            9 => EternalBattlegrounds,
            10 => BlueHome,
            11 => GreenHome,
            12 => RedHome,
            13 => FortunesVale,
            14 => ObsidianSanctum,
            15 => EdgeOfTheMists,
            16 => PublicMini,
            17 => BigBattle,
            18 => WvwLounge,
            _ => {
                anyhow::bail!("unknown map type `{value}`");
            },
        })
    }
}

impl FromStr for MapType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use MapType::*;
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("unknown") {
            Ok(Unknown)
        } else if s.eq_ignore_ascii_case("redirect") {
            Ok(Redirect)
        } else if s.eq_ignore_ascii_case("charactercreate") {
            Ok(CharacterCreate)
        } else if s.eq_ignore_ascii_case("pvp") {
            Ok(Pvp)
        } else if s.eq_ignore_ascii_case("Gvg") {
            Ok(Gvg)
        } else if s.eq_ignore_ascii_case("instance") {
            Ok(Instance)
        } else if s.eq_ignore_ascii_case("public") {
            Ok(Public)
        } else if s.eq_ignore_ascii_case("tournament") {
            Ok(Tournament)
        } else if s.eq_ignore_ascii_case("tutorial") {
            Ok(Tutorial)
        } else if s.eq_ignore_ascii_case("usertournament") {
            Ok(UserTournament)
        } else if s.eq_ignore_ascii_case("center") || s.eq_ignore_ascii_case("eternalbattlegrounds") {
            Ok(EternalBattlegrounds)
        } else if s.eq_ignore_ascii_case("bluehome") || s.eq_ignore_ascii_case("blueborderlands") {
            Ok(BlueHome)
        } else if s.eq_ignore_ascii_case("greenhome") || s.eq_ignore_ascii_case("greenborderlands") {
            Ok(GreenHome)
        } else if s.eq_ignore_ascii_case("redhome") || s.eq_ignore_ascii_case("redborderlands") {
            Ok(RedHome)
        } else if s.eq_ignore_ascii_case("fortunesvale") {
            Ok(FortunesVale)
        } else if s.eq_ignore_ascii_case("jumppuzzle") || s.eq_ignore_ascii_case("obsidiansanctum") {
            Ok(ObsidianSanctum)
        } else if s.eq_ignore_ascii_case("edgeofthemists") {
            Ok(EdgeOfTheMists)
        } else if s.eq_ignore_ascii_case("publicmini") {
            Ok(PublicMini)
        } else if s.eq_ignore_ascii_case("bigbattle") {
            Ok(BigBattle)
        } else if s.eq_ignore_ascii_case("wvwlounge") {
            Ok(WvwLounge)
        } else {
            Err(anyhow!("unknown map type `{s}`"))
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TacoBehavior {
    AlwaysVisible = 0,
    ReappearOnMapChange = 1,
    ReappearOnDailyReset = 2,
    OnlyVisibleBeforeActivation = 3,
    ReappearAfterTimer = 4,
    ReappearOnMapReset = 5,
    OncePerInstance = 6,
    OnceDailyPerCharacter = 7,
    /// internal extension
    TaimiAchievement = 33,
    /// BlishHUD extension.
    ReappearOnWeeklyReset = 101,
}

impl TryFrom<i32> for TacoBehavior {
    type Error = anyhow::Error;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use TacoBehavior::*;
        Ok(match value {
            0 => AlwaysVisible,
            1 => ReappearOnMapChange,
            2 => ReappearOnDailyReset,
            3 => OnlyVisibleBeforeActivation,
            4 => ReappearAfterTimer,
            5 => ReappearOnMapReset,
            6 => OncePerInstance,
            7 => OnceDailyPerCharacter,
            101 => ReappearOnWeeklyReset,
            _ => {
                anyhow::bail!("unknown taco behavior `{value}`");
            },
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BounceBehavior {
    Bounce,
    Rise,
}
impl BounceBehavior {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bounce => "bounce",
            Self::Rise => "rise",
        }
    }
}
impl FromStr for BounceBehavior {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("bounce") {
            Ok(BounceBehavior::Bounce)
        } else if s.eq_ignore_ascii_case("rise") {
            Ok(BounceBehavior::Rise)
        } else {
            Err(anyhow!("unknown bounce behavior `{s}`"))
        }
    }
}
