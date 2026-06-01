use {
    crate::{
        controller::markers::MarkerSaveEvent,
        exports::runtime as rt,
        marker::format::{MarkerEntry, MarkerFiletype, MarkerSet, MarkerType},
        render::element::prelude::*,
        MarkersController,
        MarkersEvent,
        ACCOUNT_NAME_CELL,
    },
    glam::Vec3,
    std::{f32, mem, path::PathBuf},
    strum::IntoEnumIterator,
};

/*
* To-do:
*  - refactor this to actually take an Option<T> where T is a struct instance of the data
*  associated with a pre-existing marker instance, such that when you are *editing* instead of
*  creating, it becomes
*/
pub struct EditMarkerWindowState {
    pub open: bool,
    pub formatted_name: Option<String>,
    pub name: String,
    pub description: String,
    pub author: String,
    pub category: ComboInput,
    pub trigger: PositionInput,
    pub map_id: i32,
    pub markers: [IndividualMarkerState; 8],
    pub path: Option<String>,
    pub idx: Option<usize>,
    pub filetype: Option<MarkerFiletype>,
    pub save_mode: Option<MarkerSaveMode>,
    pub original_category: Option<String>,
    pub filenames: Vec<PathBuf>,
    pub problems: Vec<I18nRef<'static>>,
}

pub struct IndividualMarkerState {
    pub position: PositionInput,
    pub description: String,
}

impl Default for IndividualMarkerState {
    fn default() -> Self {
        Self {
            position: Default::default(),
            description: "".to_string(),
        }
    }
}
impl IndividualMarkerState {
    pub fn set_position(&mut self, pos: Vec3) {
        self.position.position = Some(pos);
    }
    #[allow(dead_code)]
    pub fn set_description(&mut self, desc: String) {
        self.description = desc;
    }
    pub fn from_marker_entries(mes: Vec<MarkerEntry>) -> [Self; 8] {
        let mut markers: [IndividualMarkerState; 8] = Default::default();
        for me in mes.iter() {
            let position: Vec3 = me.position.clone().into();
            let mut position_input = PositionInput::default();
            position_input.position = Some(position);
            let idx = me.marker as usize;
            if idx != 0 {
                markers[idx - 1] = Self {
                    position: position_input,
                    description: me.id.clone().unwrap_or("".to_string()),
                };
            }
        }
        markers
    }
    #[allow(dead_code)]
    pub fn to_marker_entry(&self, marker: MarkerType) -> Option<MarkerEntry> {
        let id = match self.description.is_empty() {
            true => None,
            false => Some(self.description.clone()),
        };
        Some(MarkerEntry {
            marker,
            id,
            position: self.position.position?.into(),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum MarkerSaveMode {
    Create,
    Append,
    Edit,
}

impl EditMarkerWindowState {
    pub fn new() -> Self {
        Self {
            open: false,
            formatted_name: Default::default(),
            name: Default::default(),
            trigger: Default::default(),
            category: ComboInput::new(fl!("category")),
            description: Default::default(),
            map_id: Default::default(),
            author: Default::default(),
            markers: Default::default(),
            idx: Default::default(),
            save_mode: Default::default(),
            path: Default::default(),
            original_category: Default::default(),
            filetype: Default::default(),
            filenames: Default::default(),
            problems: Default::default(),
        }
    }

    pub fn validate_presave(&self) -> Vec<I18nRef<'static>> {
        let mut conditions = Vec::new();
        if self.name.is_empty() {
            conditions.push(fl!("name-empty"));
        }
        if self.trigger.position.is_none() {
            conditions.push(fl!("no-trigger"));
        }
        // i am too tired to tell if this presents problems
        if self.category.entry.is_none() {
            conditions.push(fl!("no-category"));
        }
        if self.map_id <= 0 {
            conditions.push(fl!("map-id-wrong"));
        }
        let positions: Vec<_> = self.markers.iter().flat_map(|x| x.position.position).collect();
        let pos_count = positions.len();
        if pos_count == 0 {
            conditions.push(fl!("no-positions"));
        }
        conditions
    }

    pub fn validate_save(&self) -> Vec<I18nRef<'static>> {
        let mut conditions = Vec::new();
        if let Some(path) = &self.path {
            if path.is_empty() {
                conditions.push(fl!("filename-empty"))
            }
        } else {
            conditions.push(fl!("filename-empty"))
        }
        conditions
    }

    pub fn draw_validate<'ui, U>(&self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if self.problems.len() > 0 {
            ui.text_wrapped(fl!("validation-fail"));
        }
        for problem in &self.problems {
            ui.bullet();
            ui.text_colored([1.0, 0.0, 0.0, 1.0], problem);
        }
    }

    pub fn request_filenames(&self) {
        MarkersController::try_send(MarkersEvent::GetMarkerPaths);
    }

    pub fn save_file(&mut self) {
        let ms = self.to_marker_set();
        if let Some(ms) = ms {
            if let Some(path) = &self.path {
                if let Some(save_mode) = &self.save_mode {
                    let evt = match save_mode {
                        MarkerSaveMode::Create =>
                            MarkerSaveEvent::Create(ms, path.into(), self.filetype.clone().unwrap()),
                        MarkerSaveMode::Append => MarkerSaveEvent::Append(ms, path.into()),
                        MarkerSaveMode::Edit => MarkerSaveEvent::Edit(
                            ms,
                            path.into(),
                            self.original_category.clone(),
                            self.idx.unwrap(),
                        ),
                    };
                    MarkersController::try_send(MarkersEvent::SaveMarker(evt));
                }
            }
        }
    }

    pub fn set_filenames(&mut self, filenames: Vec<PathBuf>) {
        self.filenames = filenames;
    }

    pub fn category_update(&mut self, categories: Vec<String>) {
        self.category.update(categories);
    }

    #[allow(dead_code)]
    pub fn to_marker_set(&self) -> Option<MarkerSet> {
        let marker_types = MarkerType::iter_real_values();
        let enabled = true;
        let markers = marker_types
            .enumerate()
            .flat_map(|(i, k)| self.markers[i].to_marker_entry(k))
            .collect();
        Some(MarkerSet {
            enabled,
            category: self.category.result(),
            markers,
            trigger: self.trigger.position?.into(),
            name: self.name.clone(),
            author: Some(self.author.clone()),
            map_id: self.map_id as u32, // thanks imgui types o.o
            description: self.description.clone(),
            path: None,
            idx: self.idx,
        })
    }

    pub fn open_edit(&mut self, ms: MarkerSet) {
        let prev = mem::replace(self, Self::new());
        let markers = IndividualMarkerState::from_marker_entries(ms.markers);
        let path = if let Some(path) = ms.path {
            Some(path.to_string_lossy().into())
        } else {
            None
        };
        if !self.open {
            let trigger_position: Vec3 = ms.trigger.into();
            self.category.update(prev.category.data);
            self.markers = markers;
            self.original_category = ms.category.clone();
            self.category.entry = ms.category;
            self.name = ms.name;
            self.trigger.position = Some(trigger_position);
            self.description = ms.description;
            self.author = ms.author.unwrap_or("".to_string());
            self.map_id = ms.map_id as i32;
            self.path = path;
            self.idx = ms.idx;
            self.save_mode = Some(MarkerSaveMode::Edit);
            self.open = true;
        }
    }

    pub fn open(&mut self) {
        let prev = mem::replace(self, Self::new());
        if !self.open {
            let author = match ACCOUNT_NAME_CELL.get() {
                Some(a) => a.clone(),
                None => match rt::rtapi() {
                    #[cfg(feature = "extension-nexus")]
                    Ok(Some(rtapi)) =>
                        if let Some(player_data) = rtapi.read_player() {
                            player_data.account_name
                        } else {
                            "".to_string()
                        },
                    _ => "".to_string(),
                },
            };
            let map_id = if let Ok(ml) = rt::mumble_link_ptr() {
                ml.read_map_id() as i32
            } else {
                Default::default()
            };
            self.category.update(prev.category.data);
            self.author = author;
            self.map_id = map_id;
            self.request_filenames();
            self.open = true;
        }
    }

    pub fn draw<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        let mut open = self.open;
        if open {
            let window = with_i18n!("edit-markers", |label| ui.begin_taimi_window(
                "edit-markers",
                label,
                ImCondition::initial(ImSize2::new(300.0, 200.0)),
                &mut open,
            ));
            if let Some(_window) = window {
                let _name_input = with_i18n!("name", |label| ui.input_text_managed(
                    label,
                    &mut self.name,
                    128,
                    IM_STR_NONE,
                    None
                ));
                let _author_input = with_i18n!("author", |label| ui.input_text_managed(
                    label,
                    &mut self.author,
                    128,
                    IM_STR_NONE,
                    None,
                ));
                self.category.draw(ui);
                ui.dummy([4.0; 2]);
                let _map_id_changed = with_i18n!("map-id", |label| ui.input_int(label, &mut self.map_id));
                if with_i18n!("set-map-id", |label| ui.button(label)) {
                    if let Ok(ml) = rt::mumble_link_ptr() {
                        self.map_id = ml.read_map_id() as i32
                    }
                }
                ui.dummy([4.0; 2]);
                let _description_input = with_i18n!("description", |label| ui
                    .input_text_managed_multiline(label, &mut self.description, 256, None));
                self.trigger.draw_display(ui, true);
                self.trigger.draw_take_current(ui);
                self.trigger.draw_edit_manual(ui, true);
                ui.dummy([4.0; 2]);
                #[cfg(feature = "extension-nexus")]
                if let Ok(Some(rtapi)) = rt::rtapi() {
                    use nexus::rtapi::GroupType;

                    if let Some(group) = rtapi.read_group() {
                        let is_squad =
                            matches!(group.group_type, Ok(GroupType::Squad | GroupType::RaidSquad));
                        if is_squad {
                            if ui.button(fl!("take-squad-markers")) {
                                for (i, marker) in group.squad_markers.iter().enumerate() {
                                    if *marker != [f32::INFINITY; 3] {
                                        self.markers[i].set_position(Vec3::from_array(*marker));
                                    }
                                }
                            }
                        } else {
                            ui.text_colored([1.0, 1.0, 0.0, 1.0], fl!("cannot-take-squad-markers"));
                        }
                    } else {
                        ui.text_colored([1.0, 1.0, 0.0, 1.0], fl!("cannot-take-squad-markers"));
                    }
                } else {
                    ui.text_colored([1.0, 1.0, 0.0, 1.0], fl!("rt-api-required-squad-markers"));
                }
                ui.dummy([4.0; 2]);
                let cols = ["icon", "description", "local-header", "controls"];
                let (table_flags, column_flags, col0_flags) = match ui.imgui_version_num() {
                    #[cfg(taimi_imgui = "180")]
                    Some(im180::VERSION_NUM) => (
                        imw::DynArgsTable::new(Some(imw::Table::IM180_FLAGS_PRESET)),
                        Some(imw::TableColumn::IM180_WIDTH_STRETCH),
                        Some(imw::TableColumn::IM180_WIDTH_FIXED),
                    ),
                    #[cfg(taimi_imgui = "192")]
                    Some(im192::VERSION_NUM) => (
                        imw::DynArgsTable::new(Some(imw::Table::IM192_FLAGS_PRESET)),
                        Some(imw::TableColumn::IM192_WIDTH_STRETCH),
                        Some(imw::TableColumn::IM192_WIDTH_FIXED),
                    ),
                    _ => (Default::default(), None, None),
                };
                let table_token = ui.begin_table_with_flags(c"edit_markers", cols.len(), table_flags);
                if let Some(_table) = table_token {
                    for (i, id) in cols.into_iter().enumerate() {
                        let user_id = 0;
                        let flags = match i {
                            0 => col0_flags,
                            _ => column_flags,
                        };
                        with_i18n(id, |label| {
                            ui.table_column_setup_untyped(Some(label), flags, None, user_id)
                        });
                    }
                    ui.table_header_row();
                    ui.table_next_column();
                    for (i, value) in MarkerType::iter_real_values().enumerate() {
                        let pushy = ui.push_id_hash(value);
                        if let Some(mt) = MarkerType::from_repr(i as u8 + 1) {
                            mt.icon(ui);
                        }
                        ui.table_next_column();
                        let Some(marker) = self.markers.get_mut(i) else {
                            for _ in 0..3 {
                                ui.table_next_column();
                            }
                            continue
                        };
                        {
                            let _label_size = ui.item_prepare_push_width_dyn(-1.0);
                            let label = c"##Marker Description";
                            let _changed = with_i18n!("no-description", |hint| ui.input_text_managed(
                                label,
                                &mut marker.description,
                                128,
                                Some(hint),
                                None,
                            ));
                        }
                        ui.table_next_column();
                        marker.position.draw_display(ui, false);
                        ui.table_next_column();
                        marker.position.draw_take_current(ui);
                        marker.position.draw_edit_manual(ui, false);
                        ui.table_next_column();
                    }
                }
                ui.dummy([4.0; 2]);
                self.draw_validate(ui);
                ui.dummy([4.0; 2]);
                if self.save_mode == Some(MarkerSaveMode::Edit) {
                    if ui.button(fl!("save-edit")) {
                        self.problems = self.validate_presave();
                        if self.problems.is_empty() {
                            let name = self
                                .formatted_name
                                .insert(fl!("save-edit-item", item = &self.name).into());
                            ui.open_popup(&*name);
                        }
                    }
                } else {
                    if ui.button(fl!("save")) {
                        self.problems = self.validate_presave();
                        if self.problems.len() == 0 {
                            let name = self
                                .formatted_name
                                .insert(fl!("save-item", item = &self.name).into());
                            ui.open_popup(&*name);
                        }
                    }
                }
                let popup = self
                    .formatted_name
                    .as_ref()
                    .map(|name| ui.begin_popup_modal(name, Default::default(), None));
                if let Some(_token) = popup {
                    if self.save_mode == Some(MarkerSaveMode::Edit) {
                        ui.text_colored([1.0, 1.0, 0.0, 1.0], fl!("overwrite-markerset"));
                        if ui.button(fl!("save")) {
                            self.save_file();
                            open = false;
                        }
                        ui.same_line();
                    } else {
                        self.draw_validate(ui);
                        let msm_name = |item: &MarkerSaveMode| match item {
                            MarkerSaveMode::Create => Some(fl!("save-standalone")),
                            MarkerSaveMode::Append => Some(fl!("save-append")),
                            _ => None,
                        };
                        let combo_box_text = self.save_mode.as_ref().and_then(msm_name);
                        let combo =
                            with_i18n!("save-mode", |label| ui.begin_combo_opt(label, combo_box_text,));
                        let mut selected = None;
                        if let Some(_combo) = combo {
                            for item in [MarkerSaveMode::Create, MarkerSaveMode::Append].iter() {
                                if ui.selectable(
                                    msm_name(item).unwrap(),
                                    Some(item) == self.save_mode.as_ref(),
                                ) {
                                    selected = Some(item.clone());
                                    // standalone paths are relative
                                    // append paths are absolute
                                    // pls dont mix them :(
                                    self.path = None;
                                }
                            }
                        }
                        if let Some(selection) = selected {
                            self.save_mode = Some(selection);
                        }
                    }
                    match self.save_mode {
                        Some(MarkerSaveMode::Create) => {
                            let combo_box_text = match &self.filetype {
                                Some(s) => Some(s.to_string()),
                                None => None,
                            };
                            let combo =
                                with_i18n!("filetype", |label| ui.begin_combo_opt(label, combo_box_text,));
                            let mut selected = None;
                            if let Some(_combo) = combo {
                                for item in MarkerFiletype::iter() {
                                    if ui.selectable(im_to_s!(item), Some(&item) == self.filetype.as_ref())
                                    {
                                        selected = Some(item.clone());
                                    }
                                }
                            }
                            if let Some(selection) = selected {
                                self.filetype = Some(selection);
                            }
                            ui.help_marker(|ui, _click| {
                                ui.tooltip_text(fl!("marker-filetype-explanation"));
                            });
                            let filename = self.path.get_or_insert_default();
                            with_i18n!("filename", |label| ui.input_text_managed(
                                label,
                                filename,
                                64,
                                IM_STR_NONE,
                                None
                            ));
                            if ui.button(fl!("save")) {
                                self.problems = self.validate_save();
                                if self.problems.len() == 0 {
                                    self.save_file();
                                    open = false;
                                }
                            }
                            ui.same_line();
                        },
                        Some(MarkerSaveMode::Append) => {
                            let mut selected = None;
                            let combo_box_text = match &self.path {
                                Some(s) => Some(&s[..]),
                                None => None,
                            };
                            let combo =
                                with_i18n!("filename", |label| ui.begin_combo_opt(label, combo_box_text,));
                            if let Some(_combo) = combo {
                                for item in &self.filenames {
                                    let path_name = format!("{}", item.display());
                                    if ui.selectable(
                                        im_to_s!(item.display()),
                                        Some(&path_name) == self.path.as_ref(),
                                    ) {
                                        selected = Some(path_name);
                                    }
                                }
                            }
                            if let Some(selection) = selected {
                                self.path = Some(selection).clone();
                            }
                            if ui.button(fl!("refresh-files")) {
                                self.request_filenames();
                            }
                            ui.same_line();
                            if ui.button(fl!("save")) {
                                self.problems = self.validate_save();
                                if self.problems.len() == 0 {
                                    self.save_file();
                                    open = false;
                                }
                            }
                            ui.same_line();
                        },
                        _ => (),
                    }
                    if ui.button(fl!("close")) {
                        ui.close_current_popup();
                        open = false;
                    }
                    ui.same_line();
                    if ui.button(fl!("cancel")) {
                        ui.close_current_popup();
                    }
                }
            }
            self.open = open;
        }
    }
}
