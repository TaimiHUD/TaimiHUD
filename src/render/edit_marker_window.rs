use {
    crate::{
        controller::MarkerSaveEvent,
        fl,
        marker::{
            atomic::MarkerInputData,
            format::{MarkerEntry, MarkerFiletype, MarkerSet, MarkerType},
        },
        util::{ComboInput, PositionInput, UiExt},
        ControllerEvent, Controller, ACCOUNT_NAME_CELL,
    },
    glam::Vec3,
    nexus::{
        imgui::{
            ComboBox, Id, PopupModal, Selectable, TableColumnFlags, TableColumnSetup, TableFlags,
            Ui, Window,
        },
        rtapi::{GroupType, RealTimeApi},
    },
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
    pub formatted_name: String,
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
    pub problems: Vec<String>,
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
        for (i, me) in mes.iter().enumerate() {
            let position: Vec3 = me.position.clone().into();
            let mut position_input = PositionInput::default();
            position_input.position = Some(position);
            markers[i] = Self {
                position: position_input,
                description: me.id.clone().unwrap_or("".to_string()),
            };
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
            category: ComboInput::new(&fl!("category")),
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

    pub fn validate_presave(&self) -> Vec<String> {
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
        let positions: Vec<_> = self
            .markers
            .iter()
            .flat_map(|x| x.position.position)
            .collect();
        let pos_count = positions.len();
        if pos_count == 0 {
            conditions.push(fl!("no-positions"));
        }
        conditions
    }

    pub fn validate_save(&self) -> Vec<String> {
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

    pub fn draw_validate(&self, ui: &Ui) {
        if self.problems.len() > 0 {
            ui.text_wrapped(fl!("validation-fail"));
        }
        for problem in &self.problems {
            ui.bullet();
            ui.text_colored([1.0, 0.0, 0.0, 1.0], problem);
        }
    }

    pub fn request_filenames(&self) {
        Controller::try_send(ControllerEvent::GetMarkerPaths);
    }

    pub fn save_file(&mut self) {
        let ms = self.to_marker_set();
        if let Some(ms) = ms {
            if let Some(path) = &self.path {
                if let Some(save_mode) = &self.save_mode {
                    let evt = match save_mode {
                        MarkerSaveMode::Create => {
                            MarkerSaveEvent::Create(ms, path.into(), self.filetype.clone().unwrap())
                        }
                        MarkerSaveMode::Append => MarkerSaveEvent::Append(ms, path.into()),
                        MarkerSaveMode::Edit => MarkerSaveEvent::Edit(
                            ms,
                            path.into(),
                            self.original_category.clone(),
                            self.idx.unwrap(),
                        ),
                    };
                    Controller::try_send(ControllerEvent::SaveMarker(evt));
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
            Some(path.to_string_lossy().to_string())
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
                Some(a) => (a[1..]).to_string(),
                None => match RealTimeApi::get() {
                    Some(rtapi) => {
                        if let Some(player_data) = rtapi.read_player() {
                            player_data.account_name
                        } else {
                            "".to_string()
                        }
                    }
                    None => "".to_string(),
                },
            };
            let map_id = if let Some(mid) = MarkerInputData::read() {
                mid.map_id as i32
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

    pub fn draw(&mut self, ui: &Ui) {
        let mut open = self.open;
        if open {
            let closed = Window::new(&fl!("edit-markers"))
                .size([300.0, 200.0], nexus::imgui::Condition::FirstUseEver)
                .opened(&mut open)
                .build(ui, || {
                    let name_name = fl!("name");
                    let name_input = ui.input_text(&name_name, &mut self.name);
                    name_input.build();
                    let author_name = fl!("author");
                    let author_input = ui.input_text(&author_name, &mut self.author);
                    author_input.build();
                    self.category.draw(ui);
                    ui.dummy([4.0; 2]);
                    let map_id_name = fl!("map-id");
                    let map_id_input = ui.input_int(&map_id_name, &mut self.map_id);
                    map_id_input.build();
                    if ui.button(&fl!("set-map-id")) {
                        if let Some(mid) = MarkerInputData::read() {
                            self.map_id = mid.map_id as i32;
                        }
                    }
                    ui.dummy([4.0; 2]);
                    let description_name = fl!("description");
                    let description_input = ui.input_text_multiline(
                        &description_name,
                        &mut self.description,
                        [0.0, 0.0],
                    );
                    description_input.build();
                    self.trigger.draw_display(ui, true);
                    self.trigger.draw_take_current(ui);
                    self.trigger.draw_edit_manual(ui, true);
                    ui.dummy([4.0; 2]);
                    if let Some(rtapi) = RealTimeApi::get() {
                        if let Some(group) = rtapi.read_group() {
                            let is_squad = matches!(
                                group.group_type,
                                Ok(GroupType::Squad | GroupType::RaidSquad)
                            );
                            if is_squad {
                                if ui.button(&fl!("take-squad-markers")) {
                                    for (i, marker) in group.squad_markers.iter().enumerate() {
                                        if *marker != [f32::INFINITY; 3] {
                                            self.markers[i].set_position(Vec3::from_array(*marker));
                                        }
                                    }
                                }
                            } else {
                                ui.text_colored(
                                    [1.0, 1.0, 0.0, 1.0],
                                    &fl!("cannot-take-squad-markers"),
                                );
                            }
                        } else {
                            ui.text_colored(
                                [1.0, 1.0, 0.0, 1.0],
                                &fl!("cannot-take-squad-markers"),
                            );
                        }
                    } else {
                        ui.text_colored(
                            [1.0, 1.0, 0.0, 1.0],
                            &fl!("rt-api-required-squad-markers"),
                        );
                    }
                    ui.dummy([4.0; 2]);
                    let table_flags =
                        TableFlags::RESIZABLE | TableFlags::ROW_BG | TableFlags::BORDERS;
                    let table = ui.begin_table_header_with_flags(
                        "edit_markers",
                        [
                            TableColumnSetup {
                                name: &fl!("icon"),
                                flags: TableColumnFlags::WIDTH_FIXED,
                                init_width_or_weight: 0.0,
                                user_id: Id::Str("marker_icon"),
                            },
                            TableColumnSetup {
                                name: &fl!("description"),
                                flags: TableColumnFlags::WIDTH_STRETCH,
                                init_width_or_weight: 0.0,
                                user_id: Id::Str("marker_desc"),
                            },
                            TableColumnSetup {
                                name: &fl!("local-header"),
                                flags: TableColumnFlags::WIDTH_STRETCH,
                                init_width_or_weight: 0.0,
                                user_id: Id::Str("marker_pos"),
                            },
                            TableColumnSetup {
                                name: &fl!("controls"),
                                flags: TableColumnFlags::WIDTH_STRETCH,
                                init_width_or_weight: 0.0,
                                user_id: Id::Str("marker_pos"),
                            },
                        ],
                        table_flags,
                    );
                    ui.table_next_column();
                    for (i, value) in MarkerType::iter_real_values().enumerate() {
                        let pushy = ui.push_id(Id::Str(&format!("{}", value)));
                        if let Some(mt) = MarkerType::from_repr(i as u8+1) {
                            mt.icon(ui);
                        }
                        ui.table_next_column();
                        let label_size = ui.push_item_width(-1.0);
                        let label = format!("##Marker Description {value}");
                        let meep = ui.push_id(&label);
                        let description_input =
                            ui.input_text(&label, &mut self.markers[i].description);
                        description_input.hint(&fl!("no-description")).build();
                        label_size.pop(ui);
                        meep.pop();
                        ui.table_next_column();
                        self.markers[i].position.draw_display(ui, false);
                        ui.table_next_column();
                        self.markers[i].position.draw_take_current(ui);
                        self.markers[i].position.draw_edit_manual(ui, false);
                        ui.table_next_column();
                        pushy.pop();
                    }
                    if let Some(token) = table {
                        token.end();
                    }
                    ui.dummy([4.0; 2]);
                    self.draw_validate(ui);
                    ui.dummy([4.0; 2]);
                    if self.save_mode == Some(MarkerSaveMode::Edit) {
                        if ui.button(&fl!("save-edit")) {
                            self.problems = self.validate_presave();
                            if self.problems.is_empty() {
                                self.formatted_name =
                                    fl!("save-edit-item", item = self.name.clone());
                                ui.open_popup(&self.formatted_name);
                            }
                        }
                    } else {
                        if ui.button(&fl!("save")) {
                            self.problems = self.validate_presave();
                            if self.problems.len() == 0 {
                                self.formatted_name = fl!("save-item", item = self.name.clone());
                                ui.open_popup(&self.formatted_name);
                            }
                        }
                    }
                    if let Some(_token) = PopupModal::new(&self.formatted_name)
                        .always_auto_resize(true)
                        .begin_popup(ui)
                    {
                        if self.save_mode == Some(MarkerSaveMode::Edit) {
                            ui.text_colored([1.0, 1.0, 0.0, 1.0], fl!("overwrite-markerset"));
                            if ui.button(fl!("save")) {
                                self.save_file();
                                return true;
                            }
                            ui.same_line();
                        } else {
                            self.draw_validate(ui);
                            let msm_name = |item: &MarkerSaveMode| match item {
                                MarkerSaveMode::Create => fl!("save-standalone"),
                                MarkerSaveMode::Append => fl!("save-append"),
                                _ => "".to_string(),
                            };
                            let save_mode_closure = || {
                                let mut selected = self.save_mode.clone();
                                for item in [MarkerSaveMode::Create, MarkerSaveMode::Append].iter()
                                {
                                    if Selectable::new(msm_name(item))
                                        .selected(Some(item) == self.save_mode.as_ref())
                                        .build(ui)
                                    {
                                        selected = Some(item.clone());
                                        // standalone paths are relative
                                        // append paths are absolute
                                        // pls dont mix them :(
                                        self.path = None;
                                    }
                                }
                                selected
                            };
                            let combo_box_text = match &self.save_mode {
                                Some(s) => format!("{}", msm_name(s)),
                                None => "".to_string(),
                            };
                            if let Some(Some(selection)) = ComboBox::new(fl!("save-mode"))
                                .preview_value(combo_box_text)
                                .build(ui, save_mode_closure)
                            {
                                self.save_mode = Some(selection);
                            }
                        }
                        match self.save_mode {
                            Some(MarkerSaveMode::Create) => {
                                let filetype_closure = || {
                                    let mut selected = self.filetype.clone();
                                    for item in MarkerFiletype::iter() {
                                        if Selectable::new(item.to_string())
                                            .selected(Some(&item) == self.filetype.as_ref())
                                            .build(ui)
                                        {
                                            selected = Some(item.clone());
                                        }
                                    }
                                    selected
                                };
                                let combo_box_text = match &self.filetype {
                                    Some(s) => format!("{}", s),
                                    None => "".to_string(),
                                };
                                if let Some(Some(selection)) = ComboBox::new(fl!("filetype"))
                                    .preview_value(combo_box_text)
                                    .build(ui, filetype_closure)
                                {
                                    self.filetype = Some(selection);
                                }
                                ui.help_marker(|| {
                                    ui.tooltip_text(fl!("marker-filetype-explanation"));
                                });
                                let filename = self.path.get_or_insert_default();
                                let filename_text = fl!("filename");
                                ui.input_text(filename_text, filename).build();
                                if ui.button(fl!("save")) {
                                    self.problems = self.validate_save();
                                    if self.problems.len() == 0 {
                                        self.save_file();
                                        return true;
                                    }
                                }
                                ui.same_line();
                            }
                            Some(MarkerSaveMode::Append) => {
                                let filename_closure = || {
                                    let mut selected = self.path.clone();
                                    for item in &self.filenames {
                                        let path_name = format!("{}", item.display());
                                        if Selectable::new(&path_name)
                                            .selected(Some(&path_name) == self.path.as_ref())
                                            .build(ui)
                                        {
                                            selected = Some(path_name);
                                        }
                                    }
                                    selected
                                };
                                let combo_box_text = match &self.path {
                                    Some(s) => format!("{}", s),
                                    None => "".to_string(),
                                };
                                if let Some(Some(selection)) = ComboBox::new(fl!("filename"))
                                    .preview_value(combo_box_text)
                                    .build(ui, filename_closure)
                                {
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
                                        return true;
                                    }
                                }
                                ui.same_line();
                            }
                            _ => (),
                        }
                        if ui.button(fl!("close")) {
                            ui.close_current_popup();
                            return true;
                        }
                        ui.same_line();
                        if ui.button(fl!("cancel")) {
                            ui.close_current_popup();
                        }
                    }
                    false
                });
            self.open = match closed {
                Some(true) => false,
                _ => open,
            };
        }
    }
}
