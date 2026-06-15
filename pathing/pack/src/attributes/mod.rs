use {
    crate::{category::id::IdNameBox, pack::taco_xml_to_guid},
    anyhow::{anyhow, Context},
    glam::{Vec3, Vec4},
    std::{borrow::Cow, fmt, str::FromStr, sync::Arc},
    uuid::Uuid,
    xml::name::Name,
};

pub use self::{
    festival::{Festival, Festivals},
    mount::{Mount, Mounts},
    profession::{Profession, Professions},
    race::{Race, Races},
};

pub mod festival;
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
            let x = (!value.is_empty())
                .then_some(&value[..])
                .map(f32::from_str)
                .transpose()?;
            self.poi_mut().rotate.get_or_insert(PoiAttributes::ROTATE_UNSET).x =
                x.unwrap_or(PoiAttributes::ROTATE_UNSET_AXIS);
        } else if attr_name.eq_ignore_ascii_case("rotate-y") {
            let y = (!value.is_empty())
                .then_some(&value[..])
                .map(f32::from_str)
                .transpose()?;
            self.poi_mut().rotate.get_or_insert(PoiAttributes::ROTATE_UNSET).y =
                y.unwrap_or(PoiAttributes::ROTATE_UNSET_AXIS);
        } else if attr_name.eq_ignore_ascii_case("rotate-z") {
            let z = (!value.is_empty())
                .then_some(&value[..])
                .map(f32::from_str)
                .transpose()?;
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
            self.filters_mut().festivals = Some(parse_list::<Festival>(&value)?.into_iter().collect());
        } else if attr_name.eq_ignore_ascii_case("mount") {
            self.filters_mut().mounts = Some(parse_list::<Mount>(&value)?.into_iter().collect());
        } else if attr_name.eq_ignore_ascii_case("profession") {
            self.filters_mut().professions = Some(parse_list::<Profession>(&value)?.into_iter().collect());
        } else if attr_name.eq_ignore_ascii_case("race") {
            self.filters_mut().races = Some(parse_list::<Race>(&value)?.into_iter().collect());
        } else if attr_name.eq_ignore_ascii_case("specialization") {
            self.filters_mut().specializations = Some(list_into(parse_list(&value)?));
        } else if attr_name.eq_ignore_ascii_case("maptype") {
            self.filters_mut().map_types = Some(list_into(parse_list(&value)?));
        } else if attr_name.eq_ignore_ascii_case("schedule") {
            self.filters_mut().schedule = Some(string_into(value));
        } else if attr_name.eq_ignore_ascii_case("schedule-duration") {
            self.filters_mut().schedule_duration = Some(value.parse()?);
        } else if attr_name.eq_ignore_ascii_case("raid") {
            self.filters_mut().raids = Some(list_into(parse_list(&value)?));
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
            let guids = value.split(',').map(|g| taco_xml_to_guid(g.trim_ascii()));
            self.interaction_mut().reset_guids = Some(list_into(guids.collect::<Box<[_]>>()));
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
    pub reset_guids: Option<AttrList<Uuid>>,
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

/// Filters.
#[derive(Debug, Clone, Default)]
pub struct FilterAttributes {
    pub festivals: Option<Festivals>,
    pub mounts: Option<Mounts>,
    pub professions: Option<Professions>,
    pub races: Option<Races>,
    pub specializations: Option<AttrList<i32>>,
    pub map_types: Option<AttrList<MapType>>,
    pub schedule: Option<AttrString>,
    pub schedule_duration: Option<f32>,
    pub raids: Option<AttrList<String>>,
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

/// Trail-specific.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TrailAttributes {
    pub anim_speed: Option<f32>,
    pub texture: Option<AttrString>,
    pub trail_scale: Option<f32>,
    pub is_wall: Option<bool>,
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
    }
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

#[derive(Debug, Copy, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CullDirection {
    #[default]
    None = 0,
    Clockwise = 1,
    CounterClockwise = 2,
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
pub enum TacoBehavior {
    AlwaysVisible = 0,
    ReappearOnMapChange = 1,
    ReappearOnDailyReset = 2,
    OnlyVisibleBeforeActivation = 3,
    ReappearAfterTimer = 4,
    ReappearOnMapReset = 5,
    OncePerInstance = 6,
    OnceDailyPerCharacter = 7,
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
