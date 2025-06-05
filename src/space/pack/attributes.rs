use {
    super::{taco_xml_to_guid, Pack, PackTextureHandle},
    std::str::FromStr,
    uuid::Uuid,
    xml::attribute::OwnedAttribute,
};

#[derive(Default, Clone)]
/// Attributes for markers. Inherits up the category stack.
pub struct MarkerAttributes {
    // Common.
    pub alpha: Option<f32>,
    pub can_fade: Option<bool>,
    pub tint: Option<glam::Vec4>,
    pub cull: Option<CullDirection>,
    pub edit_tag: Option<i32>,
    pub fade_near: Option<f32>,
    pub fade_far: Option<f32>,
    pub minimap_visibility: Option<bool>,
    pub map_visibility: Option<bool>,
    pub in_game_visibility: Option<bool>,

    // POI-specific.
    pub height_offset: Option<f32>,
    pub icon_file: Option<String>,
    pub icon_size: Option<f32>,
    pub invert_behavior: Option<bool>,
    pub map_display_size: Option<f32>,
    pub scale_on_map_with_zoom: Option<bool>,
    pub min_size: Option<f32>,
    pub max_size: Option<f32>,
    pub occlude: Option<bool>,
    pub rotate: Option<glam::Vec3>,
    pub billboard_text: Option<String>,
    pub billboard_text_color: Option<glam::Vec4>,
    pub tip_name: Option<String>,
    pub tip_description: Option<String>,

    // Trail-specific.
    pub anim_speed: Option<f32>,
    pub texture: Option<PackTextureHandle>,
    pub trail_scale: Option<f32>,
    pub is_wall: Option<bool>,

    // Filters.
    pub festivals: Option<Vec<Festival>>,
    pub mounts: Option<Vec<Mount>>,
    pub professions: Option<Vec<Profession>>,
    pub races: Option<Vec<Race>>,
    pub specializations: Option<Vec<i32>>,
    pub map_types: Option<Vec<MapType>>,
    pub schedule: Option<croner::Cron>,
    pub schedule_duration: Option<f32>,
    pub raids: Option<Vec<String>>,

    /// Taco Behaviors.
    pub taco_behavior: Option<TacoBehavior>,
    pub achievement_id: Option<i32>,
    pub achievement_bit: Option<i32>,
    pub reset_length: Option<f32>,
    pub auto_trigger: Option<bool>,

    /// Modifiers.
    pub info: Option<String>,
    pub info_range: Option<f32>,
    pub bounce_behavior: Option<BounceBehavior>,
    pub bounce_delay: Option<f32>,
    pub bounce_height: Option<f32>,
    pub bounce_duration: Option<f32>,
    pub copy_value: Option<String>,
    pub copy_message: Option<String>,
    pub toggle_category: Option<String>,
    pub reset_guids: Option<Vec<Uuid>>,
    pub show_category: Option<String>,
    pub hide_category: Option<String>,

    /// Scripting.
    pub script_tick: Option<String>,
    pub script_focus: Option<String>,
    pub script_trigger: Option<String>,
    pub script_filter: Option<String>,
    pub script_once: Option<String>,
}

impl MarkerAttributes {
    pub fn merge(&mut self, base: &MarkerAttributes) {
        // === Common === //
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
        if self.edit_tag.is_none() {
            self.edit_tag = base.edit_tag;
        }
        if self.fade_near.is_none() {
            self.fade_near = base.fade_near;
        }
        if self.fade_far.is_none() {
            self.fade_far = base.fade_far;
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
        // === POI-specific === //
        if self.height_offset.is_none() {
            self.height_offset = base.height_offset;
        }
        if self.icon_file.is_none() {
            self.icon_file = base.icon_file.clone();
        }
        if self.icon_size.is_none() {
            self.icon_size = base.icon_size;
        }
        if self.invert_behavior.is_none() {
            self.invert_behavior = base.invert_behavior;
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
        if self.rotate.is_none() {
            self.rotate = base.rotate;
        }
        if self.billboard_text.is_none() {
            self.billboard_text = base.billboard_text.clone();
        }
        if self.billboard_text_color.is_none() {
            self.billboard_text_color = base.billboard_text_color;
        }
        if self.tip_name.is_none() {
            self.tip_name = base.tip_name.clone();
        }
        if self.tip_description.is_none() {
            self.tip_description = base.tip_description.clone();
        }
        // === Trail-specific === //
        if self.anim_speed.is_none() {
            self.anim_speed = base.anim_speed;
        }
        if self.texture.is_none() {
            self.texture = base.texture;
        }
        if self.trail_scale.is_none() {
            self.trail_scale = base.trail_scale;
        }
        if self.is_wall.is_none() {
            self.is_wall = base.is_wall;
        }
        // === Filters === //
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
        // === Taco Behaviors === //
        if self.taco_behavior.is_none() {
            self.taco_behavior = base.taco_behavior;
        }
        if self.achievement_id.is_none() {
            self.achievement_id = base.achievement_id;
        }
        if self.achievement_bit.is_none() {
            self.achievement_bit = base.achievement_bit;
        }
        if self.reset_length.is_none() {
            self.reset_length = base.reset_length;
        }
        if self.auto_trigger.is_none() {
            self.auto_trigger = base.auto_trigger;
        }
        // === Modifiers === //
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
        if self.copy_value.is_none() {
            self.copy_value = base.copy_value.clone();
        }
        if self.copy_message.is_none() {
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
        // === Scripting === //
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

    pub fn try_add(&mut self, pack: &mut Pack, attr: &OwnedAttribute) -> bool {
        let attr_name = &attr.name.local_name.trim_start_matches("bh-");
        // === Common === //
        if attr_name.eq_ignore_ascii_case("alpha") {
            self.alpha = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("canfade") {
            self.can_fade = parse_bool(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("color") || attr_name.eq_ignore_ascii_case("tint")
        {
            self.tint = parse_color(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("cull") {
            self.cull = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("edittag") {
            self.edit_tag = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("fadenear") {
            self.fade_near = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("fadefar") {
            self.fade_far = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("minimapvisibility") {
            self.minimap_visibility = parse_bool(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("mapvisibility") {
            self.map_visibility = parse_bool(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("ingamevisibility") {
            self.in_game_visibility = parse_bool(&attr.value);
        // === POI-specific === //
        } else if attr_name.eq_ignore_ascii_case("heightoffset") {
            self.height_offset = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("iconfile") {
            self.icon_file = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("iconsize") {
            self.icon_size = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("invertbehavior") {
            self.invert_behavior = parse_bool(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("mapdisplaysize") {
            self.map_display_size = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("scaleonmapwithzoom") {
            self.scale_on_map_with_zoom = parse_bool(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("minsize") {
            self.min_size = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("maxsize") {
            self.max_size = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("occlude") {
            self.occlude = parse_bool(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("rotate") {
            let mut split = attr.value.split(',');
            let (Some(x), Some(y), Some(z)) = (split.next(), split.next(), split.next()) else {
                return false;
            };
            if let (Some(x), Some(y), Some(z)) = (x.parse().ok(), y.parse().ok(), z.parse().ok()) {
                self.rotate = Some(glam::Vec3::new(x, y, z));
            }
        } else if attr_name.eq_ignore_ascii_case("rotate-x") {
            let mut vec = self.rotate.unwrap_or_default();
            if let Ok(x) = attr.value.parse() {
                vec.x = x;
                self.rotate = Some(vec);
            }
        } else if attr_name.eq_ignore_ascii_case("rotate-y") {
            let mut vec = self.rotate.unwrap_or_default();
            if let Ok(y) = attr.value.parse() {
                vec.y = y;
                self.rotate = Some(vec);
            }
        } else if attr_name.eq_ignore_ascii_case("rotate-z") {
            let mut vec = self.rotate.unwrap_or_default();
            if let Ok(z) = attr.value.parse() {
                vec.z = z;
                self.rotate = Some(vec);
            }
        } else if attr_name.eq_ignore_ascii_case("text") || attr_name.eq_ignore_ascii_case("title")
        {
            self.billboard_text = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("title-color") {
            self.billboard_text_color = parse_color(&attr.value);
        } else if attr_name.eq_ignore_ascii_case("tip-name") {
            self.tip_name = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("tip-description") {
            self.tip_description = Some(attr.value.clone());
        // === Trail-specific === //
        } else if attr_name.eq_ignore_ascii_case("animspeed") {
            self.anim_speed = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("texture") {
            self.texture = Some(pack.register_texture(&attr.value));
        } else if attr_name.eq_ignore_ascii_case("trailscale") {
            self.trail_scale = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("iswall") {
            self.is_wall = parse_bool(&attr.value);
        // === Filters === //
        } else if attr_name.eq_ignore_ascii_case("festival") {
            self.festivals = Some(
                attr.value
                    .split(',')
                    .filter_map(|f| f.parse().ok())
                    .collect(),
            );
        } else if attr_name.eq_ignore_ascii_case("mount") {
            self.mounts = Some(
                attr.value
                    .split(',')
                    .filter_map(|f| f.parse().ok())
                    .collect(),
            );
        } else if attr_name.eq_ignore_ascii_case("profession") {
            self.professions = Some(
                attr.value
                    .split(',')
                    .filter_map(|f| f.parse().ok())
                    .collect(),
            );
        } else if attr_name.eq_ignore_ascii_case("race") {
            self.races = Some(
                attr.value
                    .split(',')
                    .filter_map(|f| f.parse().ok())
                    .collect(),
            );
        } else if attr_name.eq_ignore_ascii_case("specialization") {
            self.specializations = Some(
                attr.value
                    .split(',')
                    .filter_map(|f| f.parse().ok())
                    .collect(),
            );
        } else if attr_name.eq_ignore_ascii_case("maptype") {
            self.map_types = Some(
                attr.value
                    .split(',')
                    .filter_map(|f| f.parse().ok())
                    .collect(),
            );
        } else if attr_name.eq_ignore_ascii_case("schedule") {
            self.schedule = croner::Cron::new(&attr.value).parse().ok();
        } else if attr_name.eq_ignore_ascii_case("schedule-duration") {
            self.schedule_duration = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("raid") {
            self.raids = Some(attr.value.split(',').map(String::from).collect());
        // === Taco Behaviors === //
        } else if attr_name.eq_ignore_ascii_case("behavior") {
            self.taco_behavior = attr
                .value
                .parse::<i32>()
                .ok()
                .and_then(|i| i.try_into().ok());
        } else if attr_name.eq_ignore_ascii_case("achievementid") {
            self.achievement_id = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("achievementbit") {
            self.achievement_bit = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("resetlength") {
            self.reset_length = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("autotrigger") {
            self.auto_trigger = parse_bool(&attr.value);
        // === Modifiers === //
        } else if attr_name.eq_ignore_ascii_case("info") {
            self.info = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("inforange")
            || attr_name.eq_ignore_ascii_case("triggerrange")
        {
            self.info_range = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("bounce") {
            self.bounce_behavior = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("bounce-delay") {
            self.bounce_delay = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("bounce-height") {
            self.bounce_height = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("bounce-duration") {
            self.bounce_duration = attr.value.parse().ok();
        } else if attr_name.eq_ignore_ascii_case("copy") {
            self.copy_value = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("copy-message") {
            self.copy_message = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("toggle")
            || attr_name.eq_ignore_ascii_case("togglecategory")
        {
            self.toggle_category = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("resetguid") {
            self.reset_guids = Some(attr.value.split(',').map(|g| taco_xml_to_guid(g)).collect());
        } else if attr_name.eq_ignore_ascii_case("show") {
            self.show_category = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("hide") {
            self.hide_category = Some(attr.value.clone());
        // === Scripting === //
        } else if attr_name.eq_ignore_ascii_case("script-tick") {
            self.script_tick = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("script-focus") {
            self.script_focus = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("script-trigger") {
            self.script_trigger = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("script-filter") {
            self.script_filter = Some(attr.value.clone());
        } else if attr_name.eq_ignore_ascii_case("script-once") {
            self.script_once = Some(attr.value.clone());
        } else {
            return false;
        }
        true
    }
}


// TODO: move parse helpers into a separate file and make pub

pub fn parse_bool(value: &str) -> Option<bool> {
    value
        .parse()
        .ok()
        .or_else(|| value.parse::<i32>().ok().map(|i| i != 0))
}

fn parse_color(value: &str) -> Option<glam::Vec4> {
    let val = value.trim_start_matches('#');
    if let Ok(mut itint) = u32::from_str_radix(val, 16) {
        if val.len() == 6 {
            itint |= 0xFF000000;
        }
        return Some(glam::Vec4::new(
            ((itint >> 16) & 0xFF) as f32 / 255.0,
            ((itint >> 8) & 0xFF) as f32 / 255.0,
            ((itint >> 0) & 0xFF) as f32 / 255.0,
            ((itint >> 24) & 0xFF) as f32 / 255.0,
        ));
    }
    None
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum CullDirection {
    None = 0,
    Clockwise = 1,
    CounterClockwise = 2,
}

impl FromStr for CullDirection {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("none") {
            Ok(CullDirection::None)
        } else if s.eq_ignore_ascii_case("clockwise") {
            Ok(CullDirection::Clockwise)
        } else if s.eq_ignore_ascii_case("counterclockwise") {
            Ok(CullDirection::CounterClockwise)
        } else {
            Err(())
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Festival {
    Unknown,
    Halloween,
    Wintersday,
    SuperAdventureBox,
    LunarNewYear,
    FestivalOfTheFourWinds,
    DragonBash,
}

impl FromStr for Festival {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Festival::*;
        if s.eq_ignore_ascii_case("halloween") {
            Ok(Halloween)
        } else if s.eq_ignore_ascii_case("wintersday") {
            Ok(Wintersday)
        } else if s.eq_ignore_ascii_case("superadventurefestival") {
            Ok(SuperAdventureBox)
        } else if s.eq_ignore_ascii_case("lunarnewyear") {
            Ok(LunarNewYear)
        } else if s.eq_ignore_ascii_case("festivalofthefourwinds") {
            Ok(FestivalOfTheFourWinds)
        } else if s.eq_ignore_ascii_case("dragonbash") {
            Ok(DragonBash)
        } else {
            log::warn!("Unknown festival `{s}`");
            Ok(Unknown)
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mount {
    None = 0,
    Jackal = 1,
    Griffon = 2,
    Springer = 3,
    Skimmer = 4,
    Raptor = 5,
    RollerBeetle = 6,
    Warclaw = 7,
    Skyscale = 8,
    Skiff = 9,
    SiegeTurtle = 10,
}

impl TryFrom<i32> for Mount {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use Mount::*;
        Ok(match value {
            0 => None,
            1 => Jackal,
            2 => Griffon,
            3 => Springer,
            4 => Skimmer,
            5 => Raptor,
            6 => RollerBeetle,
            7 => Warclaw,
            8 => Skyscale,
            9 => Skiff,
            10 => SiegeTurtle,
            _ => {
                log::warn!("Unknown mount `{value}`");
                return Err(());
            }
        })
    }
}

impl FromStr for Mount {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Mount::*;
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("none") {
            Ok(None)
        } else if s.eq_ignore_ascii_case("jackal") {
            Ok(Jackal)
        } else if s.eq_ignore_ascii_case("griffon") {
            Ok(Griffon)
        } else if s.eq_ignore_ascii_case("springer") {
            Ok(Springer)
        } else if s.eq_ignore_ascii_case("skimmer") {
            Ok(Skimmer)
        } else if s.eq_ignore_ascii_case("raptor") {
            Ok(Raptor)
        } else if s.eq_ignore_ascii_case("rollerbeetle") {
            Ok(RollerBeetle)
        } else if s.eq_ignore_ascii_case("warclaw") {
            Ok(Warclaw)
        } else if s.eq_ignore_ascii_case("skyscale") {
            Ok(Skyscale)
        } else if s.eq_ignore_ascii_case("skiff") {
            Ok(Skiff)
        } else if s.eq_ignore_ascii_case("siegeturtle") {
            Ok(SiegeTurtle)
        } else {
            log::warn!("Unknown mount `{s}`");
            Err(())
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Profession {
    Guardian = 1,
    Warrior = 2,
    Engineer = 3,
    Ranger = 4,
    Thief = 5,
    Elementalist = 6,
    Mesmer = 7,
    Necromancer = 8,
    Revenant = 9,
}

impl TryFrom<i32> for Profession {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use Profession::*;
        Ok(match value {
            1 => Guardian,
            2 => Warrior,
            3 => Engineer,
            4 => Ranger,
            5 => Thief,
            6 => Elementalist,
            7 => Mesmer,
            8 => Necromancer,
            9 => Revenant,
            _ => {
                log::warn!("Unknown profession `{value}`");
                return Err(());
            }
        })
    }
}

impl FromStr for Profession {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Profession::*;
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("guardian") {
            Ok(Guardian)
        } else if s.eq_ignore_ascii_case("warrior") {
            Ok(Warrior)
        } else if s.eq_ignore_ascii_case("Engineer") {
            Ok(Engineer)
        } else if s.eq_ignore_ascii_case("ranger") {
            Ok(Ranger)
        } else if s.eq_ignore_ascii_case("thief") {
            Ok(Thief)
        } else if s.eq_ignore_ascii_case("elementalist") {
            Ok(Elementalist)
        } else if s.eq_ignore_ascii_case("mesmer") {
            Ok(Mesmer)
        } else if s.eq_ignore_ascii_case("necromancer") {
            Ok(Necromancer)
        } else if s.eq_ignore_ascii_case("revenant") {
            Ok(Revenant)
        } else {
            log::warn!("Unknown profession `{s}`");
            Err(())
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Race {
    Asura = 0,
    Charr = 1,
    Human = 2,
    Norn = 3,
    Sylvari = 4,
}

impl TryFrom<i32> for Race {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        use Race::*;
        Ok(match value {
            0 => Asura,
            1 => Charr,
            2 => Human,
            3 => Norn,
            4 => Sylvari,
            _ => {
                log::warn!("Unknown race `{value}`");
                return Err(());
            }
        })
    }
}

impl FromStr for Race {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use Race::*;
        if let Ok(i) = s.parse::<i32>() {
            i.try_into()
        } else if s.eq_ignore_ascii_case("asura") {
            Ok(Asura)
        } else if s.eq_ignore_ascii_case("charr") {
            Ok(Charr)
        } else if s.eq_ignore_ascii_case("human") {
            Ok(Human)
        } else if s.eq_ignore_ascii_case("norn") {
            Ok(Norn)
        } else if s.eq_ignore_ascii_case("sylvari") {
            Ok(Sylvari)
        } else {
            log::warn!("Unknown race `{s}`");
            Err(())
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
    type Error = ();

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
                log::warn!("Unknown map type `{value}`");
                Unknown
            }
        })
    }
}

impl FromStr for MapType {
    type Err = ();

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
        } else if s.eq_ignore_ascii_case("center") || s.eq_ignore_ascii_case("eternalbattlegrounds")
        {
            Ok(EternalBattlegrounds)
        } else if s.eq_ignore_ascii_case("bluehome") || s.eq_ignore_ascii_case("blueborderlands") {
            Ok(BlueHome)
        } else if s.eq_ignore_ascii_case("greenhome") || s.eq_ignore_ascii_case("greenborderlands")
        {
            Ok(GreenHome)
        } else if s.eq_ignore_ascii_case("redhome") || s.eq_ignore_ascii_case("redborderlands") {
            Ok(RedHome)
        } else if s.eq_ignore_ascii_case("fortunesvale") {
            Ok(FortunesVale)
        } else if s.eq_ignore_ascii_case("jumppuzzle") || s.eq_ignore_ascii_case("obsidiansanctum")
        {
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
            log::warn!("Unknown MapType `{s}`");
            Ok(Unknown)
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
    type Error = ();

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
                log::warn!("Unknown taco behavior `{value}`");
                return Err(());
            }
        })
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum BounceBehavior {
    Bounce,
    Rise,
}

impl FromStr for BounceBehavior {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.eq_ignore_ascii_case("bounce") {
            Ok(BounceBehavior::Bounce)
        } else if s.eq_ignore_ascii_case("rise") {
            Ok(BounceBehavior::Rise)
        } else {
            log::warn!("Unknown BounceBehavior `{}`", s);
            Err(())
        }
    }
}
