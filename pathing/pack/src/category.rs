use {
    anyhow::Context,
    crate::{
        attributes::{parse_bool, MarkerAttributes},
        pack::{taco_safe_name, PartialItem},
    },
    bitvec::vec::BitVec,
    indexmap::IndexMap,
    std::{
        collections::HashMap,
        sync::Arc,
    },
};

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

        let mut id = String::new();
        let mut display_name = None;
        let mut is_separator = false;
        let mut is_hidden = false;
        let mut default_toggle = true;

        for attr in attrs {
            let attr_name = attr.name.local_name.trim_start_matches("bh-");
            let res = if attr_name.eq_ignore_ascii_case("name") {
                id = taco_safe_name(&attr.value, false);
                Ok(())
            } else if attr_name.eq_ignore_ascii_case("displayname") {
                display_name = Some(attr.value);
                Ok(())
            } else if attr_name.eq_ignore_ascii_case("isseparator") {
                parse_bool(&attr.value)
                    .map(|val| is_separator = val)
                    .map_err(From::from)
            } else if attr_name.eq_ignore_ascii_case("ishidden") {
                parse_bool(&attr.value)
                    .map(|val| is_hidden = val)
                    .map_err(From::from)
            } else if attr_name.eq_ignore_ascii_case("defaulttoggle") {
                parse_bool(&attr.value)
                    .map(|val| default_toggle = val)
                    .map_err(From::from)
            } else {
                marker_attributes.try_add(attr.name.borrow(), attr.value)
            }.with_context(|| format!("parsing category attribute '{}'", attr.name));
            if let Err(e) = res {
                log::warn!("{e:#}");
            }
        }

        let full_id = if let Some(PartialItem::MarkerCategory(cat)) = parse_stack.last() {
            format!("{}.{id}", cat.full_id)
        } else {
            id.clone()
        };

        let marker_attributes = Arc::new(marker_attributes);

        Ok(Category {
            display_name: display_name.unwrap_or(id.clone()),
            id,
            full_id,
            is_separator,
            is_hidden,
            default_toggle,
            sub_categories: Default::default(),
            marker_attributes,
        })
    }

    pub fn recompute_enabled(&self, all_categories: &IndexMap<String, Category>, enabled_categories: &mut BitVec, user_category_state: &BitVec, parent: bool) {
        if let Some(idx) = all_categories.get_index_of(&self.full_id) {
            if let Some(cur) = user_category_state.get(idx) {
                let res = parent && *cur;
                if let Some(mut cat) = enabled_categories.get_mut(idx) {
                    *cat = res;
                }
                for (_local, global) in self.sub_categories.iter() {
                    all_categories[global].recompute_enabled(all_categories, enabled_categories, user_category_state, res);
                }
            }
        }
    }

    pub fn attain_state(
        &self,
        all_categories: &IndexMap<String, Category>,
        state: &mut HashMap<String, bool>,
    ) {
        let _ = state
            .entry(self.full_id.clone())
            .or_insert(self.default_toggle);
        for (_local, global) in self.sub_categories.iter() {
            all_categories[global].attain_state(all_categories, state);
        }
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
        Arc::make_mut(&mut new.marker_attributes).merge(&self.marker_attributes);
        self.marker_attributes = new.marker_attributes;
        let self_subs = Arc::make_mut(&mut self.sub_categories);
        for (local_id, full_id) in Arc::make_mut(&mut new.sub_categories).drain(..) {
            if !self_subs.contains_key(&local_id) {
                self_subs.insert(local_id, full_id);
            }
        }
    }
}
