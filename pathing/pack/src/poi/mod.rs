use {
    crate::{
        attributes::{keys, AttrString, MarkerAttributes},
        category::id::IdNameBox,
        pack::{taco_safe_name, taco_xml_to_guid},
    },
    anyhow::Context,
    glam::{EulerRot, Quat, Vec3},
    glamour::Point3,
    std::fmt,
    uuid::Uuid,
};

#[derive(Debug, Clone)]
pub struct Poi {
    pub category: IdNameBox,
    pub guid: Uuid,
    pub map_id: i32,
    pub position: Point3,
    pub attributes: MarkerAttributes,
    pub parent_path: Option<AttrString>,
}

impl Poi {
    pub fn from_xml(
        asset_parent: Option<&AttrString>,
        attrs: Vec<xml::attribute::OwnedAttribute>,
    ) -> anyhow::Result<Poi> {
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
                if !attr.value.is_empty() {
                    guid = Some(taco_xml_to_guid(&attr.value));
                }
                Ok(())
            } else if attr.name.local_name.starts_with("bh-") {
                match attributes_bh.try_add(attr.name.borrow(), attr.value) {
                    #[cfg(todo = "silence warnings")]
                    Ok(false) => Ok(log::debug!("unrecognized POI attribute `{}`", attr.name)),
                    res => res.map(drop),
                }
            } else {
                match attributes.try_add(attr.name.borrow(), attr.value) {
                    #[cfg(todo = "silence warnings")]
                    Ok(false) => Ok(log::info!("unrecognized POI attribute `{}`", attr.name)),
                    res => res.map(drop),
                }
            }
            .with_context(|| format!("POI attribute '{}'", attr.name));
            #[cfg(todo = "silence warnings")]
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

        Ok(Poi {
            category: category.into(),
            guid,
            map_id,
            position,
            attributes,
            parent_path: asset_parent.cloned(),
        })
    }

    pub fn icon_file(&self) -> Option<&str> {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.icon_file())
            .map(|s| &s[..])
    }
    pub fn height_offset(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.height_offset)
            .unwrap_or(keys::HeightOffset::DEFAULT.into())
    }
    pub fn icon_size(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.icon_size)
            .unwrap_or(keys::IconSize::DEFAULT.into())
    }
    pub fn map_display_size(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.map_display_size)
            .unwrap_or(keys::MapDisplaySize::DEFAULT.into())
    }
    pub fn scale_on_map_with_zoom(&self) -> bool {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.scale_on_map_with_zoom)
            .unwrap_or(keys::ScaleOnMapWithZoom::DEFAULT.into())
    }
    pub fn min_size(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.min_size)
            .unwrap_or(keys::MinSize::DEFAULT.into())
    }
    pub fn max_size(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.max_size)
            .unwrap_or(keys::MaxSize::DEFAULT.into())
    }
    pub fn occlude(&self) -> bool {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.occlude)
            .unwrap_or(keys::Occlude::DEFAULT.into())
    }
    pub fn rotate(&self) -> Option<Vec3> {
        self.attributes.get_poi().and_then(|poi| poi.rotate)
    }
    pub fn rotation(&self) -> Option<Quat> {
        self.rotate()
            .map(|r| Quat::from_euler(EulerRot::XYZ, r.x, r.y, r.z))
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
