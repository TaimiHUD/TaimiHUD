/*
* Derived from belst; https://github.com/belst/nexus-wingman-uploader/blob/master/src/util.rs
*/

use {
    crate::{exports::runtime as rt, fl},
    glam::Vec3,
    nexus::imgui::{ComboBox, InputFloat3, Selectable, StyleColor, Ui},
};

#[allow(dead_code)]
pub trait UiExt {
    fn help_marker<F: FnOnce()>(&self, f: F) -> bool;
    fn attention_marker<F: FnOnce()>(&self, f: F) -> bool;
    fn link(&self, label: impl AsRef<str>, url: impl AsRef<str>);
}

impl UiExt for Ui<'_> {
    fn help_marker<F: FnOnce()>(&self, f: F) -> bool {
        let mut clicked = false;
        self.same_line();
        self.text_disabled("(?)");
        if self.is_item_hovered() && self.is_item_clicked() {
            clicked = true;
        }
        if self.is_item_hovered() {
            f();
        }
        clicked
    }
    fn attention_marker<F: FnOnce()>(&self, f: F) -> bool {
        let mut clicked = false;
        self.same_line();
        self.text_disabled("(!)");
        if self.is_item_hovered() && self.is_item_clicked() {
            clicked = true;
        }
        if self.is_item_hovered() {
            f();
        }
        clicked
    }
    fn link(&self, label: impl AsRef<str>, url: impl AsRef<str>) {
        let blue = self.push_style_color(StyleColor::Text, [0.0, 0.0, 1.0, 1.0]);
        self.text(label);
        blue.pop();
        let mut min = self.item_rect_min();
        let max = self.item_rect_max();
        min[1] = max[1];
        self.get_window_draw_list()
            .add_line(min, max, [0.0, 0.0, 1.0, 1.0])
            .build();
        if self.is_item_hovered() {
            if self.is_item_clicked() {
                if let Err(e) = open::that_detached(url.as_ref()) {
                    log::error!("Failed to open {}: {e}", url.as_ref());
                }
            }
            self.tooltip_text(fl!("open-button", kind = url.as_ref()));
        }
    }
}

pub struct ComboInput {
    label: String,
    make_entry: bool,
    pub entry: Option<String>,
    pub data: Vec<String>,
}

impl ComboInput {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            make_entry: false,
            entry: None,
            data: Default::default(),
        }
    }

    pub fn update(&mut self, data: Vec<String>) {
        log::trace!("Categories updated: {:?}", data);
        self.data = data;
    }

    pub fn draw(&mut self, ui: &Ui) {
        if self.make_entry {
            let entry = self.entry.get_or_insert_default();
            ui.input_text(&self.label, entry).build();
        } else {
            let closure = || {
                let mut selected = self.entry.clone();
                for item in &self.data {
                    if Selectable::new(item)
                        .selected(Some(item) == self.entry.as_ref())
                        .build(ui)
                    {
                        selected = Some(item.clone())
                    }
                }
                selected
            };
            let combo_box_text = match &self.entry {
                Some(s) => s,
                None => "",
            };
            if let Some(Some(selection)) = ComboBox::new(self.label.clone())
                .preview_value(combo_box_text)
                .build(ui, closure)
            {
                self.entry = Some(selection);
            }
        }
        let button_text = match self.make_entry {
            false => fl!("create-arg", arg = self.label.clone()),
            true => fl!("not-create-arg", arg = self.label.clone()),
        };
        if ui.button(button_text) {
            self.make_entry = !self.make_entry;
        }
    }

    pub fn result(&self) -> Option<String> {
        self.entry.clone()
    }
}

#[derive(Default)]
pub struct PositionInput {
    pub position: Option<Vec3>,
    position_before_edit: Option<Vec3>,
    opened: bool,
}

impl PositionInput {
    pub fn draw_display(&self, ui: &Ui, trigger: bool) {
        if let Some(position) = self.position {
            let position = format!("({}, {}, {})", position.x, position.y, position.z);
            if trigger {
                ui.text_wrapped(&fl!("trigger", position = position));
                ui.help_marker(|| {
                    ui.tooltip_text(fl!("trigger-explanation"));
                });
            } else {
                ui.text_wrapped(position);
            }
        } else {
            let position = fl!("no-position");
            if trigger {
                ui.text_wrapped(&fl!("trigger", position = position));
                ui.help_marker(|| {
                    ui.tooltip_text(fl!("trigger-explanation"));
                });
            } else {
                ui.text_wrapped(position);
            }
        }
    }
    pub fn draw_take_current(&mut self, ui: &Ui) {
        if ui.button(&fl!("position-get")) {
            if let Ok(ml) = rt::mumble_link_ptr() {
                self.position = Some(Vec3::from_array(ml.read_avatar().position));
            }
        }
    }
    pub fn draw_edit_manual(&mut self, ui: &Ui, trigger: bool) {
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
            if !trigger {
                let text = fl!("manual-position");
                let text_width = ui.calc_text_size(&text)[0] + 4.0f32;
                let item_width_token = ui.push_item_width(-text_width);
                let position_input = InputFloat3::new(ui, &text, position_as_type);
                position_input.build();
                item_width_token.pop(ui);
            } else {
                let text = fl!("manual-position");
                let position_input = InputFloat3::new(ui, &text, position_as_type);
                position_input.build();
            }
            if ui.button(&fl!("revert")) {
                self.opened = false;
                self.position = self.position_before_edit;
            }
            ui.same_line();
            if ui.button(&fl!("clear")) {
                self.opened = false;
                self.position = None;
            }
        }
    }
}
