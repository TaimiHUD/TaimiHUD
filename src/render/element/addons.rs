use crate::{render::element::prelude::*, settings::state::AddonHostName};

#[derive(Debug, Clone, Default)]
#[cfg(todo)]
pub struct AddonHostSelection {
    pub host: Option<AddonHostName>,
}

#[cfg(todo)]
impl AddonHostSelection {
    pub const fn new_minimal(host: Option<AddonHostName>) -> Self {
        Self { host, without_i18n: true }
    }

    pub fn draw<'ui, U>(&mut self, ui: &mut U, label_id: &str) -> Option<AddonHostName>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.draw_inner(ui, label_id, false).flatten()
    }

    pub fn draw_opt<'ui, U>(&mut self, ui: &mut U, label_id: &str) -> Option<Option<AddonHostName>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        self.draw_inner(ui, label_id, true)
    }

    fn draw_inner<'ui, U>(
        &mut self,
        ui: &mut U,
        label_id: &str,
        optional: bool,
    ) -> Option<Option<AddonHostName>>
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut new_selection = None;
        let none = match self.without_i18n {
            false => "disable",
            true => "Disable",
        };
        let combo = match self.without_i18n {
            false => {
                let host_id = self.host.map(|h| h.id()).unwrap_or(none);
                // TODO: ImStrId::new(label_id, label),
                with_i18n!((label_id, host_id), |(label, selected)| ui
                    .begin_combo(label, selected))
            },
            true => {
                let host_name = self.host.map(|h| h.name()).unwrap_or(none);
                ui.begin_combo(label_id, host_name)
            },
        };
        if let Some(_token) = combo {
            let all = AddonHostName::ALL
                .into_iter()
                .map(Some)
                .chain(optional.then_some(None));
            for loader in all {
                let is_selected = self.host == loader;
                let selected = match self.without_i18n {
                    false => {
                        let id = loader.map(|l| l.id()).unwrap_or(none);
                        with_i18n!(id, |label| ui.selectable(label, is_selected))
                    },
                    true => {
                        let name = loader.map(|l| l.name()).unwrap_or(none);
                        ui.selectable(name, is_selected)
                    },
                };
                if selected {
                    new_selection = Some(loader);
                }
            }
        }

        new_selection
    }
}

#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct AddonHostSelectionCache {
    pub label: String0,
    pub status: ItemStatus,
    pub changed: ItemStatus,
}
#[cfg(todo)]
#[derive(Debug, Clone, Default)]
pub struct AddonHostSelectionDesc {
    pub label_id: &'a str,
    pub is_optional: bool,
    /// disable i18n for use even if rest of addon is disabled
    pub without_i18n: bool,
}
#[cfg(todo)]
impl<'a, 's, 'ui, W, C> Drawable<W, C> for DrawAddonHostSelection<'a, 's>
where
    W: ?Sized + ImDrawWindow<'ui>,
    C: ?Sized + DrawContext<'ui>,
{
    fn draw_on_window(&mut self, window: &mut W, context: &mut C) {
        if let Some(v) = self.state.draw_inner(window, self.label_id, self.is_optional) {
            self.new_selection = Some(v);
            context.raise_signal(InteractSignal::TRIGGER);
        } else if context.signal_interest().contains(InteractSignal::HOVER) && window.item_is_hovered() {
            context.raise_signal(InteractSignal::HOVER);
        }
    }
}
#[derive(Debug, Clone, Default)]
pub struct AddonHostSelection<L = I18nRef<'static>> {
    pub desc: im::SelectionEnumDesc<L>,
    pub scratch: im::SelectionScratch<im::SelectionEnumLabels<AddonHostName>>,
    pub host: Option<AddonHostName>,
}
impl<L> AddonHostSelection<L> {
    pub fn new(label: L, opt: Result<L, Option<L>>) -> Self {
        Self::with_host(label, opt, None)
    }
    pub fn with_host(label: L, opt: Result<L, Option<L>>, host: Option<AddonHostName>) -> Self {
        let (none_label, is_optional) = match opt {
            Ok(none_label) => (Some(none_label), true),
            Err(none_label) => (none_label, false),
        };
        let desc = im::SelectionEnumDesc {
            label,
            none_label,
            is_optional,
            no_preview: false,
        };
        Self {
            desc,
            scratch: Default::default(),
            host,
        }
    }
    pub fn draw<'ui, U, C>(&mut self, ui: &mut U, context: &mut C) -> bool
    where
        L: fmt::Display,
        U: ?Sized + ImDrawWindow<'ui>,
        C: ?Sized + DrawContext<'ui>,
    {
        if self.host.is_none() && self.desc.none_label.is_none() {
            self.desc.no_preview = true;
        }
        im::SelectionEnumDraw {
            desc: &self.desc,
            scratch: &mut self.scratch,
            state: &mut self.host,
        }
        .draw(ui, context);
        self.scratch.changed.contains(ItemStatus::COMMIT)
    }
}
impl<'ui, W, C, L> Drawable<W, C> for AddonHostSelection<L>
where
    W: ?Sized + ImDrawWindow<'ui>,
    C: ?Sized + DrawContext<'ui>,
    L: fmt::Display,
{
    fn draw_on_window(&mut self, window: &mut W, context: &mut C) {
        self.draw(window, context);
    }
}
