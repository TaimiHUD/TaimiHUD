use {
    super::{
        attributes::{parse_bool, MarkerAttributes},
        taco_safe_name, Pack, PartialItem,
    }, crate::{marker::atomic::MarkerInputData, render::pathing_window::PathingFilterState}, bitvec::vec::BitVec, indexmap::IndexMap, nexus::imgui::{Condition, TreeNode, Ui}, std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    }
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
        pack: &mut Pack,
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
            if attr_name.eq_ignore_ascii_case("name") {
                id = taco_safe_name(&attr.value, false);
            } else if attr_name.eq_ignore_ascii_case("displayname") {
                display_name = Some(attr.value);
            } else if attr_name.eq_ignore_ascii_case("isseparator") {
                if let Some(val) = parse_bool(&attr.value) {
                    is_separator = val;
                }
            } else if attr_name.eq_ignore_ascii_case("ishidden") {
                if let Some(val) = parse_bool(&attr.value) {
                    is_hidden = val;
                }
            } else if attr_name.eq_ignore_ascii_case("defaulttoggle") {
                if let Ok(val) = attr.value.parse() {
                    default_toggle = val;
                }
            } else if !marker_attributes.try_add(pack, &attr) {
                log::warn!(
                    "Unknown MarkerCategory attribute '{}'",
                    attr.name.local_name
                );
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

    pub fn draw(&self, ui: &Ui, all_categories: &IndexMap<String, Category>, state: &mut BitVec, filter_state: PathingFilterState, open_items: &mut HashSet<String>, is_root: bool, recompute: &mut bool) {
        let push_token = ui.push_id(&self.full_id);
        if self.is_hidden {
            push_token.pop();
            return;
        }
        let mut display = true;
        if let Some(idx) = all_categories.get_index_of(&self.full_id) {
            if let Some(substate) = state.get(idx) {
                let enabled_filter = *substate && filter_state.contains(PathingFilterState::Enabled);
                let disabled_filter = !*substate && filter_state.contains(PathingFilterState::Disabled);
                let is_root_filter = is_root && filter_state.contains(PathingFilterState::IgnoreRoot);
                let is_leaf = self.sub_categories.is_empty();
                let is_branch = !is_leaf;
                let is_leaf_filter = is_leaf && filter_state.contains(PathingFilterState::IgnoreLeaves);
                let is_branch_filter = is_branch && filter_state.contains(PathingFilterState::IgnoreBranches);
                display = enabled_filter | disabled_filter | is_root_filter | is_leaf_filter | is_branch_filter;
            }
        }
        if display {
            let mut unbuilt = TreeNode::new(&self.display_name)
                .frame_padding(true)
                .tree_push_on_open(false)
                .opened(open_items.contains(&self.full_id), Condition::Always);
            if self.is_separator {
                unbuilt = unbuilt.leaf(true);
            } else if self.sub_categories.is_empty() {
                unbuilt = unbuilt.bullet(true);
            } else {
                unbuilt = unbuilt.framed(true);
            }
            let tree_token = unbuilt.push(ui);
            ui.table_next_column();
            if !self.is_separator {
                if let Some(idx) = all_categories.get_index_of(&self.full_id) {
                    if let Some(mut substate) = state.get_mut(idx) {
                        if ui.checkbox("", &mut substate) {
                            *recompute = true;
                        };
                    }
                }
            }
            let mut internal_closure = || {
                if !open_items.contains(&self.full_id) {
                    open_items.insert(self.full_id.clone());
                }
                if !self.sub_categories.is_empty() {
                    ui.indent(); //_by(1.0);
                }
                for (_local, global) in self.sub_categories.iter() {
                    all_categories[global].draw(ui, all_categories, state, filter_state, open_items, false, recompute);
                }
                if !self.sub_categories.is_empty() {
                    ui.unindent(); //_by(1.0);
                }
            };
            ui.table_next_column();
            if let Some(token) = tree_token {
                internal_closure();
                token.pop();
            } else {
                if open_items.contains(&self.full_id) {
                    open_items.remove(&self.full_id);
                }
            }
        }
        push_token.pop();
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
