use {
    crate::{
        attributes::{AttrString, MarkerAttributes},
        category::id::IdNameBox,
        pack::{taco_safe_name, taco_xml_to_guid, PackBuilderMarkerWarnings},
    },
    anyhow::Context,
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
        warnings: &mut PackBuilderMarkerWarnings,
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
                    Ok(false) => {
                        warnings.attr_warning(&attr.name, &"POI");
                        Ok(())
                    },
                    res => res.map(drop),
                }
            } else {
                match attributes.try_add(attr.name.borrow(), attr.value) {
                    Ok(false) => {
                        warnings.attr_warning(&attr.name, &"POI");
                        Ok(())
                    },
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

        Ok(Poi {
            category: category.into(),
            guid,
            map_id,
            position,
            attributes,
            parent_path: asset_parent.cloned(),
        })
    }

    #[inline]
    pub fn icon_name(&self) -> Option<&str> {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.icon_file.as_ref())
            .map(|s| &s[..])
    }

    #[inline]
    pub fn height_offset(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.height_offset)
            .unwrap_or(1.5)
    }

    #[inline]
    pub fn icon_scale(&self) -> f32 {
        self.attributes
            .get_poi()
            .and_then(|poi| poi.icon_size)
            .unwrap_or(1.0)
    }
}

impl fmt::Display for Poi {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let guid = &self.guid;
        let category = &self.category;
        match &self.parent_path {
            Some(parent) => write!(f, "{parent}{category}/{guid}"),
            None => write!(f, "{category}/{guid}"),
        }
    }
}
