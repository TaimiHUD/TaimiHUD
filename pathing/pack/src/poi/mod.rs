use {
    crate::{
        attributes::MarkerAttributes,
        category::id::IdNameBox,
        pack::{taco_safe_name, taco_xml_to_guid},
    },
    anyhow::Context,
    glam::Vec4,
    glamour::Point3,
    std::{fmt, path::Path},
    uuid::Uuid,
};

#[derive(Debug, Clone)]
pub struct Poi {
    pub category: IdNameBox,
    pub guid: Uuid,
    pub map_id: i32,
    pub position: Point3,
    pub attributes: MarkerAttributes,
    pub parent_path: Option<String>,
}

impl Poi {
    pub fn from_xml(asset: &str, attrs: Vec<xml::attribute::OwnedAttribute>) -> anyhow::Result<Poi> {
        let mut category = String::new();
        let mut map_id = None;
        let mut pos_x = None;
        let mut pos_y = None;
        let mut pos_z = None;
        let mut guid = None;
        let mut attributes = MarkerAttributes::default();
        let mut attributes_bh = MarkerAttributes::default();

        for attr in attrs {
            let res = if attr.name.local_name.eq_ignore_ascii_case("type") {
                category = taco_safe_name(&attr.value, true);
                Ok(())
            } else if attr.name.local_name.eq_ignore_ascii_case("mapid") {
                attr.value.parse().map(|v| map_id = Some(v)).map_err(From::from)
            } else if attr.name.local_name.eq_ignore_ascii_case("xpos") {
                attr.value.parse().map(|v| pos_x = Some(v)).map_err(From::from)
            } else if attr.name.local_name.eq_ignore_ascii_case("ypos") {
                attr.value.parse().map(|v| pos_y = Some(v)).map_err(From::from)
            } else if attr.name.local_name.eq_ignore_ascii_case("zpos") {
                attr.value.parse().map(|v| pos_z = Some(v)).map_err(From::from)
            } else if attr.name.local_name.eq_ignore_ascii_case("guid") {
                guid = Some(taco_xml_to_guid(&attr.value));
                Ok(())
            } else if attr.name.local_name.starts_with("bh-") {
                match attributes_bh.try_add(attr.name.borrow(), attr.value) {
                    Ok(false) => Ok(log::debug!("unrecognized POI attribute `{}`", attr.name)),
                    res => res.map(drop),
                }
            } else {
                match attributes.try_add(attr.name.borrow(), attr.value) {
                    Ok(false) => Ok(log::info!("unrecognized POI attribute `{}`", attr.name)),
                    res => res.map(drop),
                }
            }
            .with_context(|| format!("POI attribute '{}'", attr.name));
            if let Err(e) = res {
                log::warn!("{e:#}");
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

        // TODO: support bh features properly...
        attributes.merge(&attributes_bh, false);

        let parent_path = Path::new(asset).parent().map(|p| p.to_string_lossy().into());
        Ok(Poi {
            category: category.into(),
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
        self.attributes.height_offset.unwrap_or(1.5)
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

impl fmt::Display for Poi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let guid = &self.guid;
        match &self.parent_path {
            Some(parent) => write!(f, "{parent}/{guid}"),
            None => write!(f, "{guid}"),
        }
    }
}
