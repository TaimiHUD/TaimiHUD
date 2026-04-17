use {
    crate::{
        exports::runtime::{
            bindings::{self, Control, KeyIntercept},
            imgui,
        },
        fl,
        settings::{state::SaveState, ArcVk},
        with_i18n,
    },
    anyhow::Context,
    std::{
        borrow::Cow,
        collections::{btree_map, BTreeMap},
    },
    taimi_input::win::keyboard::KeyInput,
};

#[derive(Debug, Clone, Default)]
pub struct KeyBindSelection {
    pub binding_buffers: BTreeMap<usize, KeyBindState>,
}

impl KeyBindSelection {
    pub fn draw_gamebind<'a>(
        &'a mut self,
        ui: &imgui::Ui,
        control: Control,
    ) -> Option<&'a mut KeyBindState> {
        let default_key = bindings::DEFAULT_GAMEBINDS
            .get(&control.into())
            .copied()
            .unwrap_or(KeyInput::EMPTY);

        let current_key = SaveState::read_with(|state| state.game_binds.get_slot(control, None));
        let current_key = match current_key {
            Some(((vk, mods), idx)) => Some(KeyInput::new(vk, mods, true)),
            None => None,
        }
        .unwrap_or(KeyInput::EMPTY);
        let key = i32::from(control) as usize;

        let keyname = match control {
            control => format!("{control}"),
        };
        let _id_token = ui.push_id(&keyname);

        log::debug!(
            "Looking up translation for {}",
            keyname.to_lowercase().replace(" ", "-").as_str()
        );

        with_i18n!(keyname.as_str(), |msg| self.draw_bind(
            ui,
            key,
            current_key,
            default_key,
            &msg
        ))
    }

    pub fn draw_keybind<'a, F: FnOnce(&ArcVk)>(
        &'a mut self,
        ui: &imgui::Ui,
        vk: &'static ArcVk,
        action: Option<F>,
    ) -> Option<&'a mut KeyBindState> {
        let _id_token = ui.push_id(vk.id);
        let name = vk.get_name();
        match action {
            Some(action) =>
                if ui.button(name) {
                    action(vk)
                },
            None => ui.text(name),
        }
        ui.same_line();

        let default_vk = vk.vkeycode_default();
        let default_key = default_vk.map(KeyInput::from).unwrap_or(KeyInput::EMPTY);
        let current_vk = vk.get_setting_vkeycode();
        let current_key = current_vk.map(KeyInput::from).unwrap_or(KeyInput::EMPTY);
        let key = vk.id.as_ptr() as usize;

        with_i18n!("keybind", |msg| self.draw_bind(
            ui,
            key,
            current_key,
            default_key,
            &msg
        ))
    }

    pub fn draw_bind<'a>(
        &'a mut self,
        ui: &imgui::Ui,
        key: usize,
        current: KeyInput,
        default: KeyInput,
        label: &str,
    ) -> Option<&'a mut KeyBindState> {
        let any_configuring = self.binding_buffers.values().any(|b| b.configuring);

        let binding_buffer =
            KeyBindState::unwrap_entry_btreemap(self.binding_buffers.entry(key), key, current, default);
        let mut changed = binding_buffer.draw_input(ui, key, label);
        if !changed {
            ui.same_line();
            changed |= binding_buffer.draw_bind(ui, any_configuring);
        }
        changed.then_some(binding_buffer)
    }

    pub fn do_keybind<F: FnOnce(&ArcVk)>(&mut self, ui: &imgui::Ui, vk: &'static ArcVk, action: Option<F>) {
        let changed = self.draw_keybind(ui, vk, action);

        if let Some(new) = changed {
            let new_vk = match new.take_pending() {
                Some(Ok(new)) => {
                    if !new.mods.is_empty() {
                        log::warn!("TODO: save keybind modifiers: {}", new.mods);
                    }
                    new.vk
                },
                Some(Err(..)) | None => return,
            };

            let res = vk
                .set_vkeycode(new_vk)
                .with_context(|| format!("saving keybind {} failed", vk.id));
            if let Err(e) = res {
                log::error!("{e:#}");
            }
        }
    }

    pub fn do_gamebind(&mut self, ui: &imgui::Ui, control: Control) {
        let changed = self.draw_gamebind(ui, control);

        if let Some(new) = changed {
            let mut saved = true;
            SaveState::write_with(|state| {
                let game_binds = state.game_binds_mut();
                match new.take_pending() {
                    Some(Ok(new)) if new.is_empty() => game_binds.remove((control.into(), i8::MIN)),
                    Some(Ok(new)) => game_binds.set(control, i8::MIN, (new.vk.0, new.mods)),
                    Some(Err(..)) | None => {
                        saved = false;
                        return
                    },
                }
            });
            bindings::populate_bind_controls();
        }
    }
    pub fn do_gamebinds<I: IntoIterator>(&mut self, ui: &imgui::Ui, controls: I)
    where
        I::Item: Into<Control>,
    {
        for control in controls {
            self.do_gamebind(ui, control.into());
        }
    }

    pub fn clear_dirty(&mut self) {
        for bind in self.binding_buffers.values_mut() {
            bind.reset_configuring();
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeyBindState {
    pub name_buffer: String,
    pub current_name: Cow<'static, str>,
    pub default_name: Option<Cow<'static, str>>,
    pub configuring: bool,
    pub filtered: bool,
    pub pending: Option<Result<KeyInput, ()>>,
}

impl KeyBindState {
    pub const EMPTY: Self = Self {
        name_buffer: String::new(),
        current_name: Cow::Owned(String::new()),
        default_name: None,
        configuring: false,
        filtered: false,
        pending: None,
    };

    pub fn clear(&mut self) {
        self.name_buffer.clear();
        self.clear_cache();
    }

    pub fn clear_cache(&mut self) {
        match &mut self.current_name {
            Cow::Owned(name) => name.clear(),
            name => *name = Cow::Owned(String::new()),
        }
    }

    pub fn reset_configuring(&mut self) {
        let _ = self.pending.take();
        self.clear();
    }

    pub fn take_pending(&mut self) -> Option<Result<KeyInput, &str>> {
        self.pending.take().map(|p| p.map_err(|()| &self.name_buffer[..]))
    }

    pub fn binding_from_input(input: &str) -> anyhow::Result<KeyInput> {
        if input.is_empty() {
            return Ok(KeyInput::EMPTY)
        }

        let input = KeyInput::parse_ascii(input.as_bytes())
            .with_context(|| format!("Invalid keybind {input:?}"))?;

        Ok(input)
    }

    pub fn unwrap_entry_btreemap<'b>(
        binding_buffer: btree_map::Entry<'b, usize, Self>,
        key: usize,
        current: KeyInput,
        default: KeyInput,
    ) -> &'b mut Self {
        let is_fresh = match &binding_buffer {
            btree_map::Entry::Vacant(..) => true,
            btree_map::Entry::Occupied(b) => b.get().name_buffer.is_empty(),
        };
        let binding_buffer = binding_buffer.or_insert_with(|| {
            let mut binding = Self::default();
            binding.filtered = match Self::key_is_shortcut(key) {
                #[cfg(feature = "extension-arcdps")]
                true if crate::exports::arcdps::loaded() => true,
                _ => false,
            };
            binding.default_name = default.any().map(Self::vk_name);
            binding
        });

        if binding_buffer.current_name.is_empty() {
            Self::vk_name_into(current, &mut binding_buffer.current_name);
        }
        if is_fresh {
            binding_buffer.name_buffer = binding_buffer.current_name.clone().into_owned();
        }
        binding_buffer
    }

    pub fn draw_input(&mut self, ui: &imgui::Ui, key: usize, label: &str) -> bool {
        let input = ui
            .input_text(label, &mut self.name_buffer)
            .read_only(self.configuring)
            .auto_select_all(true)
            .always_insert_mode(true)
            .enter_returns_true(true)
            .no_undo_redo(true)
            .no_horizontal_scroll(true);
        let input = match &self.default_name {
            Some(name) => input.hint(&name[..]),
            _ => input,
        };
        let mut changed = input.build();

        if changed {
            self.pending = Some(Self::binding_from_input(&self.name_buffer).map_err(|e| {
                log::warn!("{e:#}");
            }));
            self.clear_cache();
        }
        if !changed && ui.is_item_clicked_with_button(imgui::MouseButton::Right) {
            changed = true;
            self.clear();
            self.pending.get_or_insert(Ok(KeyInput::EMPTY));
        }
        if Self::key_is_shortcut(key) {
            ui.same_line();
            if ui.checkbox("mod-filtered", &mut self.filtered) {
                changed |= true;
                self.pending.get_or_insert(Err(()));
            }
        }
        changed
    }

    pub fn draw_bind(&mut self, ui: &imgui::Ui, any_configuring: bool) -> bool {
        match (self.configuring, any_configuring) {
            (true, _) => self.draw_bind_prompt(ui),
            (false, false) => {
                self.draw_bind_button(ui);
                false
            },
            (false, true) => {
                ui.dummy([0.0; 2]);
                false
            },
        }
    }

    pub fn draw_bind_prompt(&mut self, ui: &imgui::Ui) -> bool {
        let press_key_translation = fl!("press-key");
        match bindings::held_mods() {
            mods if mods.is_empty() => ui.text_disabled(press_key_translation),
            mods => ui.text(format!("{press_key_translation}: {mods}+")),
        };
        match KeyIntercept::intercept_take() {
            None => {
                log::info!("key bind cancelled");
                self.configuring = false;
                self.pending = None;
            },
            Some(KeyIntercept::Pending) => (),
            Some(KeyIntercept::Intercepted { key }) => {
                log::debug!("got key bind: {key:?}");
                match key.down {
                    false => {
                        KeyIntercept::intercept_restart();
                    },
                    true => {
                        self.configuring = false;
                        self.clear();
                        self.pending = Some(Ok(key));
                        return true
                    },
                }
            },
        }
        false
    }

    pub fn draw_bind_button(&mut self, ui: &imgui::Ui) {
        debug_assert!(!KeyIntercept::intercept_ready());
        if ui.button(fl!("bind")) {
            KeyIntercept::intercept_restart();
            self.configuring = true;
            self.pending = None;
        }
    }

    fn vk_name(key: KeyInput) -> Cow<'static, str> {
        let mut out = Cow::Owned(String::new());
        Self::vk_name_into(key, &mut out);
        out
    }

    fn vk_name_into(key: KeyInput, out: &mut Cow<'static, str>) {
        match key.any() {
            Some(key) => {
                use std::fmt::Write;
                let mut out = out.to_mut();
                out.clear();

                let _ = write!(&mut out, "{}", key.display_addonapi());
            },
            None => {
                *out = Cow::Borrowed("(unbound)");
            },
        }
    }

    fn key_is_shortcut(key: usize) -> bool {
        key > 0x100
    }
}

impl Default for KeyBindState {
    fn default() -> Self {
        Self::EMPTY
    }
}
