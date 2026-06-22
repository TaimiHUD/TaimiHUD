/*
* Derived from belst; https://github.com/belst/nexus-wingman-uploader/blob/master/src/util.rs
*/

use {
    crate::{exports::runtime as rt, render::element::prelude::*},
    glam::Vec3,
};

pub struct ComboInput {
    label: String,
    make_entry: bool,
    pub entry: Option<String>,
    pub data: Vec<String>,
}

impl ComboInput {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            make_entry: false,
            entry: None,
            data: Default::default(),
        }
    }

    pub fn update(&mut self, data: Vec<String>) {
        log::trace!("Categories updated: {:?}", data);
        self.data = data;
    }

    pub fn draw<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if self.make_entry {
            let entry = self.entry.get_or_insert_default();
            ui.input_text_managed(&self.label, entry, 96, IM_STR_NONE, None);
        } else {
            let combo_box_text = self.entry.as_ref().map(|e| &e[..]);
            let combo = ui.begin_combo_opt(&self.label, combo_box_text);
            let mut selected = None;
            if let Some(_combo) = combo {
                for item in &self.data {
                    if ui.selectable(item, Some(item) == self.entry.as_ref()) {
                        selected = Some(item.clone())
                    }
                }
            };
            if let Some(selection) = selected {
                self.entry = Some(selection);
            }
        }
        let maked = match self.make_entry {
            false => with_i18n!("create-arg", |label| ui.button(label)),
            true => with_i18n!("not-create-arg", |label| ui.button(label)),
        };
        if maked {
            self.make_entry = !self.make_entry;
        }
    }

    pub fn result(&self) -> Option<String> {
        self.entry.clone()
    }
}

#[derive(Debug, Default)]
pub struct PositionInput {
    pub position: Option<Vec3>,
    position_before_edit: Option<Vec3>,
    opened: bool,
}

impl PositionInput {
    pub fn draw_display<'ui, U>(&self, ui: &mut U, trigger: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if let Some(position) = self.position {
            let position = format!("({}, {}, {})", position.x, position.y, position.z);
            if trigger {
                ui.text_wrapped(fl!("trigger", position = &position));
                ui.help_marker(|ui, _click| {
                    ui.tooltip_text(fl!("trigger-explanation"));
                });
            } else {
                ui.text_wrapped(position);
            }
        } else {
            let position = fl!("no-position");
            if trigger {
                ui.text_wrapped(fl!("trigger", [position = position]));
                ui.help_marker(|ui, _click| {
                    ui.tooltip_text(fl!("trigger-explanation"));
                });
            } else {
                ui.text_wrapped(position);
            }
        }
    }
    pub fn draw_take_current<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if ui.button(fl!("position-get")) {
            self.fill_current();
        }
    }
    pub fn fill_current(&mut self) {
        if let Ok(ml) = rt::mumble_link_ptr() {
            self.position = Some(Vec3::from_array(ml.read_avatar().position));
        }
    }
    pub fn draw_edit_manual<'ui, U>(&mut self, ui: &mut U, trigger: bool)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let button_text = match self.opened {
            true => fl!("set-manually-save"),
            false => fl!("set-manually"),
        };
        if ui.button(&button_text) {
            self.opened = !self.opened;
            if self.opened {
                self.position_before_edit = self.position;
            }
        }
        if self.opened {
            let position_as_type = self.position.get_or_insert_default().as_mut();
            let _changed = with_i18n!("manual-position", |label| {
                let _item_width_token = (!trigger).then(|| {
                    let text_width = ui.calc_text_size(&label)[0] + 4.0f32;
                    ui.push_item_width(-text_width)
                });
                ui.inputs_scalar(label, position_as_type, IM_STR_NONE)
            });
            if ui.button(fl!("revert")) {
                self.opened = false;
                self.position = self.position_before_edit;
            }
            ui.same_line();
            if ui.button(fl!("clear")) {
                self.opened = false;
                self.position = None;
            }
        }
    }
}
