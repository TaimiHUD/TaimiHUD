use {
    crate::{
        attributes::MarkerAttributes,
        pack::{taco_safe_name, taco_xml_to_guid},
    },
    anyhow::Context,
    glam::Vec4,
    glamour::Point3,
    std::path::Path,
    uuid::Uuid,
};

#[derive(Clone)]
pub struct Poi {
    pub category: String,
    pub guid: Uuid,
    pub map_id: i32,
    pub position: Point3,
    pub attributes: MarkerAttributes,
    pub parent_path: Option<String>,
}

impl Poi {
    pub fn from_xml(
        asset: &str,
        attrs: Vec<xml::attribute::OwnedAttribute>,
    ) -> anyhow::Result<Poi> {
        let mut category = String::new();
        let mut map_id = None;
        let mut pos_x = None;
        let mut pos_y = None;
        let mut pos_z = None;
        let mut guid = None;
        let mut attributes = MarkerAttributes::default();

        for attr in attrs {
            if attr.name.local_name.eq_ignore_ascii_case("type") {
                category = taco_safe_name(&attr.value, true);
            } else if attr.name.local_name.eq_ignore_ascii_case("MapID") {
                map_id = Some(attr.value.parse().context("Parse POI MapID")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("xpos") {
                pos_x = Some(attr.value.parse().context("Parse POI xpos")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("ypos") {
                pos_y = Some(attr.value.parse().context("Parse POI ypos")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("zpos") {
                pos_z = Some(attr.value.parse().context("Parse POI zpos")?);
            } else if attr.name.local_name.eq_ignore_ascii_case("guid") {
                guid = Some(taco_xml_to_guid(&attr.value));
            } else if let Err(..) = attributes.try_add(attr.name.borrow(), attr.value) {
                log::warn!("Unknown POI attribute '{}'", attr.name.local_name);
            }
        }

        let Some(map_id) = map_id else {
            anyhow::bail!("POI must have MapID");
        };

        let (Some(pos_x), Some(pos_y), Some(pos_z)) = (pos_x, pos_y, pos_z) else {
            anyhow::bail!("POI must have xpos, ypos, and zpos");
        };
        let position = Point3::new(pos_x, pos_y, pos_z);

        let guid = guid.unwrap_or_default();

        let parent_path = Path::new(asset).parent()
            .map(|p| p.to_string_lossy().into());
        Ok(Poi {
            category,
            guid,
            map_id,
            position,
            attributes,
            parent_path,
        })
    }

    #[inline]
    pub fn icon_name(&self) -> Option<&str> {
        self.attributes.icon_file.as_ref().map(|s| &s[..])
    }

    #[inline]
    pub fn height_offset(&self) -> f32 {
        self.attributes.height_offset.unwrap_or(0.0)
    }

    #[inline]
    pub fn icon_scale(&self) -> f32 {
        self.attributes.icon_size.unwrap_or(1.0)
    }

    #[inline]
    pub fn tint(&self) -> Vec4 {
        self.attributes.tint.unwrap_or(Vec4::ONE)
    }

    #[inline]
    pub fn alpha(&self) -> f32 {
        self.attributes.alpha.unwrap_or(1.0)
    }
}
