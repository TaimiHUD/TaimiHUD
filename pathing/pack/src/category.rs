use {
    crate::{
        attributes::{parse_bool, MarkerAttributes},
        pack::{taco_safe_name, PartialItem},
    },
    anyhow::{anyhow, Context},
    indexmap::IndexMap,
    std::sync::Arc,
};

#[derive(Debug, Clone, Default)]
pub struct Category {
    pub id: String,
    pub full_id: String,
    pub display_name: String,
    pub is_separator: bool,
    pub is_hidden: bool,
    pub default_toggle: bool,
    // Map of local to global name.
    pub sub_categories: Arc<IndexMap<String, String>>,
    /// Attributes for markers attached to this category.
    pub marker_attributes: Arc<MarkerAttributes>,
}

impl Category {
    pub fn from_xml(
        parse_stack: &[PartialItem],
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

        let name = id.or(bh_id).ok_or_else(|| anyhow!("category missing name"))?;
        let id = taco_safe_name(&name, false);

        let full_id = if let Some(PartialItem::MarkerCategory(cat)) = parse_stack.last() {
            format!("{}.{id}", cat.full_id)
        } else {
            id.clone()
        };

        // TODO: support bh features properly...
        marker_attributes.merge(&attributes_bh, false);

        let marker_attributes = Arc::new(marker_attributes);
        let display_name = display_name.or(bh_display_name).unwrap_or(id.clone());
        let is_separator = is_separator.or(bh_is_separator).unwrap_or(false);
        let is_hidden = is_hidden.or(bh_is_hidden).unwrap_or(false);
        let default_toggle = default_toggle.or(bh_default_toggle).unwrap_or(true);

        Ok(Category {
            display_name,
            id,
            full_id,
            is_separator,
            is_hidden,
            default_toggle,
            sub_categories: Default::default(),
            marker_attributes,
        })
    }

    pub fn merge(&mut self, mut new: Category) {
        if self.id != new.id || self.full_id != new.full_id {
            log::error!(
                "Invalid category state. Attempted to merge {} onto {}",
                new.full_id,
                self.full_id
            );
            return;
        }
        // This should not result in a clone because nobody else should own the Arc.
        if Arc::strong_count(&new.marker_attributes) > 1 {
            log::warn!("Multiple owners for category attributes.");
        }
        Arc::make_mut(&mut new.marker_attributes).merge(&self.marker_attributes, false);
        self.marker_attributes = new.marker_attributes;
        let self_subs = Arc::make_mut(&mut self.sub_categories);
        for (local_id, full_id) in Arc::make_mut(&mut new.sub_categories).drain(..) {
            if !self_subs.contains_key(&local_id) {
                self_subs.insert(local_id, full_id);
            }
        }
    }
}
