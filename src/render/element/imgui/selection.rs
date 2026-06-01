use {
    crate::render::element::prelude::*,
    arcffi::repr::{EnumRepr, EnumReprArrayOf, EnumReprCollection},
};

#[cfg(todo)]
pub struct VerticalStackDraw<D, S, C> {
    pub desc: &'d SelectionListDesc,
    pub state: &'s mut SelectionListState<I>,
    pub scratch: &'s mut L,
}

#[cfg(todo)]
pub struct SelectionList {}
#[cfg(todo)]
pub struct SelectionListDraw<'d, 's, I, L> {
    pub desc: &'d SelectionListDesc,
    pub state: &'s mut SelectionListState<I>,
    pub scratch: &'s mut L,
}
pub struct SelectionEnumState<T: EnumRepr> {
    pub selection: Option<T>,
}

#[derive(Debug, Copy, Clone, Default)]
pub struct SelectionEnumDesc<N = I18nRef<'static>> {
    pub label: N,
    pub none_label: Option<N>,
    pub is_optional: bool,
    pub no_preview: bool,
}

#[derive(Debug, Clone, Default)]
pub struct SelectionScratch<L> {
    pub label_none: String0,
    pub label: String0,
    pub labels: L,
    pub status: ItemStatus,
    pub changed: ItemStatus,
}
pub struct SelectionEnumDraw<'d, 's, T: EnumReprArrayOf<String0>, N = I18nRef<'static>> {
    pub desc: &'d SelectionEnumDesc<N>,
    pub scratch: &'s mut SelectionScratch<SelectionEnumLabels<T>>,
    pub state: &'s mut Option<T>,
}

impl<'d, 's, T, N> SelectionEnumDraw<'d, 's, T, N>
where
    N: fmt::Display,
    T: EnumRepr + EnumReprArrayOf<String0> + Into<&'static str> + Copy,
{
    pub fn draw<'ui, U, C>(&mut self, ui: &mut U, context: &mut C)
    where
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        if self.scratch.label.is_empty() {
            self.scratch.label = String0::format(&self.desc.label);
        }
        let current = *self.state;
        let preview = match current {
            _ if self.desc.no_preview => None,
            None => {
                if let (true, Some(preview)) = (self.scratch.label_none.is_empty(), &self.desc.none_label) {
                    self.scratch.label_none = String0::format(preview);
                }
                let preview = self.scratch.label_none.as_c_str();
                (!preview.is_empty()).then_some(preview)
            },
            Some(current) => Some(self.scratch.labels.label_for(current).as_c_str()),
        };
        let combo = ui.begin_combo_opt(self.scratch.label.as_c_str(), preview);
        let status_prev = self.scratch.status;
        //let open_and_visible = combo.is_some();
        if let Some(_combo) = combo {
            self.scratch.status.insert(ItemStatus::OPEN);
            let current = current.map(|v| v.to_repr());
            for item in T::repr_iter() {
                let is_selected = current == Some(item.to_repr());
                let label = self.scratch.labels.label_for(item);
                if ui.selectable(label.as_c_str(), is_selected) {
                    self.scratch.status.insert(ItemStatus::COMMIT);
                    *self.state = Some(item);
                }
            }
            if self.desc.is_optional {
                if let (true, Some(label)) = (self.scratch.label_none.is_empty(), &self.desc.none_label) {
                    self.scratch.label_none = String0::format(label);
                }
                if ui.selectable(self.scratch.label_none.as_c_str(), current.is_none()) {
                    self.scratch.status.insert(ItemStatus::COMMIT);
                    *self.state = None;
                }
            }
        } else {
            self.scratch.status.remove(ItemStatus::OPEN);
        }
        self.scratch.status.set(ItemStatus::HOVER, ui.is_item_hovered());
        // TODO: context/rightclick?
        context.mask_and_signal_slot(&mut self.scratch.changed, self.scratch.status ^ status_prev);
    }
}

pub struct SelectionEnumLabels<T: EnumReprArrayOf<String0>> {
    pub storage: <T as EnumReprArrayOf<String0>>::EnumArray,
}
impl<T> SelectionEnumLabels<T>
where
    T: EnumReprArrayOf<String0> + Copy,
    T: Into<&'static str>,
{
    pub fn label_for(&mut self, item: T) -> &Str0 {
        let label = self.storage.at_repr_mut(item);
        if label.is_empty() {
            *label = String0::format(I18nRef::new(Into::<&'static str>::into(item)));
        }
        &*label
    }
}
impl<T> Clone for SelectionEnumLabels<T>
where
    T: EnumReprArrayOf<String0>,
    <T as EnumReprArrayOf<String0>>::EnumArray: Clone,
{
    fn clone(&self) -> Self {
        Self { storage: self.storage.clone() }
    }
}
impl<T> Default for SelectionEnumLabels<T>
where
    T: EnumReprArrayOf<String0>,
    <T as EnumReprArrayOf<String0>>::EnumArray: Default,
{
    fn default() -> Self {
        Self { storage: Default::default() }
    }
}
impl<T> fmt::Debug for SelectionEnumLabels<T>
where
    T: EnumReprArrayOf<String0>,
    <T as EnumReprArrayOf<String0>>::EnumArray: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("SelectionEnumLabels").field(&self.storage).finish()
    }
}
