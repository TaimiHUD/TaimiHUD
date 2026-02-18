use {
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
            AttrString,
            MarkerAttributes,
        },
        category::id::IdNameBox,
        pack::{taco_safe_name, taco_xml_to_guid, PackBuilderMarkerWarnings},
    },
    anyhow::Context,
    glamour::Point3,
    std::{borrow::Cow, fmt},
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

        let map_id = map_id.unwrap_or(0i32);
        #[cfg(todo)]
        let Some(map_id) = map_id
        else {
            anyhow::bail!("POI must have MapID");
        };

        #[cfg(todo)]
        let (Some(pos_x), Some(pos_y), Some(pos_z)) = (pos_x, pos_y, pos_z) else {
            anyhow::bail!("POI must have xpos, ypos, and zpos");
        };
        let position = Point3::new(
            pos_x.unwrap_or_default(),
            pos_y.unwrap_or_default(),
            pos_z.unwrap_or_default(),
        );

        let guid = guid.unwrap_or_default();

        // TODO: support bh features properly...
        //attributes.merge(&attributes_bh, false);
        attributes_bh.merge(&attributes, false);
        attributes = attributes_bh;

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
        self.attributes.get_poi().and_then(|poi| poi.rotate())
    }
    pub fn rotation(&self) -> Option<Quat> {
        self.attributes.get_poi().and_then(|poi| poi.rotation())
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
impl Poi {
    fn holds_attr_dyn_inherent(key: PackKeyId) -> bool {
        pack_attr!(=id_is_in(key, [
            keys::CategoryRef,
            keys::Guid,
            keys::GameMap,
            keys::PositionX,
            keys::PositionY,
            keys::PositionZ,
            // keys::Position,
        ]))
    }
}

impl GetAttrDyn for Poi {
    fn holds_attr_dyn(key: PackKeyId) -> bool {
        Self::holds_attr_dyn_inherent(key) || MarkerAttributes::holds_attr_dyn(key)
    }
    fn has_attr_dyn(&self, key: PackKeyId) -> bool {
        Self::holds_attr_dyn_inherent(key) || self.attributes.has_attr_dyn(key)
    }
    #[inline]
    fn get_attr_dyn_ref(&self, key: PackKeyId) -> Option<&dyn AttrKeyValue> {
        let v = pack_attr! { imp GetAttrDyn::get_attr_dyn_ref(self, key) in [
            keys::CategoryRef,
            keys::Guid,
            keys::GameMap,
            keys::PositionX,
            keys::PositionY,
            keys::PositionZ,
            // keys::Position,
        ] };
        if let Some(v) = v {
            v
        } else {
            self.attributes.get_attr_dyn_ref(key)
        }
    }
    #[inline]
    fn get_attr_dyn(&self, key: PackKeyId) -> Option<Cow<'_, dyn AttrKeyValue>> {
        if Self::holds_attr_dyn_inherent(key) {
            self.get_attr_dyn_ref(key).map(Cow::Borrowed)
        } else {
            self.attributes.get_attr_dyn(key)
        }
    }
    #[inline]
    fn clone_attr_dyn(&self, key: PackKeyId) -> Option<PackValueDyn> {
        let v = pack_attr! { imp GetAttrDyn::clone_attr_dyn(self, key) in [
            keys::CategoryRef,
            keys::Guid,
            keys::GameMap,
            keys::PositionX,
            keys::PositionY,
            keys::PositionZ,
            // keys::Position,
        ] };
        if let Some(v) = v {
            v
        } else {
            self.attributes.clone_attr_dyn(key)
        }
    }
    fn iter_attrs_dyn(&self) -> impl Iterator<Item = Cow<'_, dyn AttrKeyValue>> + '_ {
        pack_attr! { imp GetAttrDyn::iter_attrs_dyn(self) in [
            keys::CategoryRef,
            keys::Guid,
            keys::GameMap,
            keys::PositionX,
            keys::PositionY,
            keys::PositionZ,
            // keys::Position,
        ] }
        .chain(self.attributes.iter_attrs_dyn())
    }
}
impl SetAttrDyn for Poi {
    #[inline]
    fn set_attr_dyn(&mut self, value: PackValueCell) -> bool {
        pack_attr! { imp SetAttrDyn::set_attr_dyn(self, value) in
            [
                keys::CategoryRef,
                keys::Guid,
                keys::GameMap,
                keys::PositionX,
                keys::PositionY,
                keys::PositionZ,
                // keys::Position,
            ],
            _ =>
                self.attributes.set_attr_dyn(value),
        }
    }
}
pack_attr! {
    impl Attr{keys::CategoryRef} for &struct{Poi}.category {}
    impl Attr{keys::Guid} for &struct{Poi}.guid {}
    impl Attr{keys::GameMap} for &struct{Poi}.map_id {}
    //impl Attr{keys::PositionX} for &struct{Poi}.position.x {}
}
impl GetAttr<keys::PositionX> for Poi {
    fn has_attr(&self) -> bool {
        true
    }
    fn get_attr_ref(&self) -> Option<&keys::PositionX> {
        Some(keys::PositionX::from_ref(&self.position.x))
    }
}
impl GetAttr<keys::PositionY> for Poi {
    fn has_attr(&self) -> bool {
        true
    }
    fn get_attr_ref(&self) -> Option<&keys::PositionY> {
        Some(keys::PositionY::from_ref(&self.position.y))
    }
}
impl GetAttr<keys::PositionZ> for Poi {
    fn has_attr(&self) -> bool {
        true
    }
    fn get_attr_ref(&self) -> Option<&keys::PositionZ> {
        Some(keys::PositionZ::from_ref(&self.position.z))
    }
}
impl SetAttr<keys::PositionX> for Poi {
    fn set_attr(&mut self, value: keys::PositionX) {
        self.position.x = value.0;
    }
}
impl SetAttr<keys::PositionY> for Poi {
    fn set_attr(&mut self, value: keys::PositionY) {
        self.position.y = value.0;
    }
}
impl SetAttr<keys::PositionZ> for Poi {
    fn set_attr(&mut self, value: keys::PositionZ) {
        self.position.z = value.0;
    }
}
