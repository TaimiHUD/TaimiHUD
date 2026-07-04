use {
    crate::{
        controller::{
            api::{ApiAccountInfo, ApiAccountState, ApiMessage},
            Controller,
        },
        exports::runtime::{self as rt},
        render::{
            element::{prelude::*, token::ApiTokenInput},
            i18n::current_game_language,
            RenderState,
        },
        settings::state::{BootstrapState, SaveState, SavedApiToken},
        with_i18n,
    },
    chrono::TimeZone,
    std::collections::{BTreeMap, BTreeSet, HashMap},
    strum::VariantArray,
    taimi_sync::watched::{watch, Watched},
};

#[cfg(feature = "paths")]
use crate::controller::pathing::PathingEnables;

pub struct ApiTabState {
    boot_state: watch::Receiver<BootstrapState>,
    save_state: watch::Receiver<SaveState>,
    account_state: Watched<ApiAccountState>,
    #[cfg(feature = "paths")]
    pathing_enables: Watched<PathingEnables>,
    auto_update: bool,
    tokens: BTreeMap<String, ApiTokenState>,
    add: ApiTokenAdd,
    account_data_last_modified: Option<String>,
}

impl ApiTabState {
    pub fn new() -> Self {
        let mut state = Self {
            boot_state: BootstrapState::get().subscribe(),
            save_state: SaveState::get().subscribe(),
            account_state: Watched::EMPTY,
            #[cfg(feature = "paths")]
            pathing_enables: Watched::EMPTY,
            auto_update: false,
            tokens: BTreeMap::new(),
            add: ApiTokenAdd::new(),
            account_data_last_modified: None,
        };
        state.boot_state.mark_changed();
        state.save_state.mark_changed();
        Controller::with_sender(|s| {
            if let Some(api) = &s.api {
                state.account_state.restart_watching(&api.account_state);
            }
            #[cfg(feature = "paths")]
            if let Some(pathing) = &s.pathing {
                state.pathing_enables.restart_watching(&pathing.enables);
            }
        });
        state
    }

    pub fn draw<'ui, U>(&mut self, ui: &mut U, _state_errors: &mut HashMap<String, anyhow::Error>)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        if self.boot_state.has_changed().ok() == Some(true) {
            self.sync_boot();
        }
        if self.save_state.has_changed().ok() == Some(true) {
            self.sync_save();
        }
        if let Some(account_state) = self.account_state.try_read_if_changed() {
            log::trace!("ACC UPDATED: {account_state:?}");
            self.account_data_last_modified = account_state
                .last_updated_achievements()
                .map(|time| TimeZone::from_utc_datetime(&chrono::Local, &time.naive_utc()).to_rfc2822());
        }
        let account_state = self.account_state.borrow_mut();
        let tree_token = (!self.tokens.is_empty() || !account_state.is_empty()).then(|| {
            with_i18n!("data", |label| ui.begin_tree_node_framed(
                ImCondition::initial(true),
                c"data",
                label,
                true,
            ))
        });
        let has_account_section = tree_token.is_some();
        if let Some(Some(_tree)) = &tree_token {
            fn id_text<'ui, U: ?Sized + ImDrawWindow<'ui>>(
                ui: &mut U,
                id_name: &mut Option<String>,
                label: impl ImStrExt,
            ) {
                if id_name.take().is_some() {
                    ui.same_line();
                }
                ui.text(label);
            }
            let mut id_name = None;
            if let Some(id) = account_state.account_id() {
                id_name = ApiTokenState::id_name(id);
                if let Some(id) = &id_name {
                    ui.text(&id);
                }
            }
            if let Some(last_modified) = &self.account_data_last_modified {
                id_text(
                    ui,
                    &mut id_name,
                    fl!("checked-for-updates-last", time = last_modified),
                );
            }
            let update_avail = match (
                account_state.data_update_available,
                account_state.update_available,
            ) {
                (Some(false), Some(false)) => {
                    with_i18n!("update-not-required", |msg| id_text(ui, &mut id_name, msg));
                    false
                },
                (data_update, None | Some(true)) if self.account_data_last_modified.is_none() => {
                    with_i18n!("update-unknown", |msg| id_text(ui, &mut id_name, msg));
                    data_update.unwrap_or(true)
                },
                (data_update, _) => data_update.unwrap_or(false),
            };
            if ui.is_item_clicked() {
                // hidden button whee :3
                ApiMessage::AccountInfoRefresh(Some(ApiAccountInfo::Account)).try_send();
            }

            if with_i18n!("reload-data-sources", |label| ui.button(label)) {
                ApiMessage::account_reload_all().try_send();
            }
            ui.same_line();
            let refresh = with_i18n!("refresh", |label| match update_avail {
                false => ui.small_button(&label),
                true => ui.button(&label),
            });
            if refresh {
                ApiMessage::AccountInfoRefresh(None).try_send();
            }

            if with_i18n!("api-auto-update", |label| ui
                .checkbox(label, &mut self.auto_update))
            {
                ApiMessage::SetAutoUpdate(self.auto_update).try_send();
            }
        }
        drop(tree_token);

        #[cfg(feature = "paths")]
        {
            let enables = self.pathing_enables.get_mut();
            if enables.contains(PathingEnables::KATRENDER) && enables.contains(PathingEnables::API_BYPASS) {
                with_i18n!("pathing-config-api-bypass", |label| ui.text(label));
                let hovered = ui.is_item_hovered();
                ui.same_line();
                with_i18n!("enabled", |label| ui.text(label));

                if hovered {
                    with_i18n!("pathing-config", |label| ui.tooltip_text(label));
                }
            }
        }
        if has_account_section {
            ui.separator();
        }

        let account_name = crate::ACCOUNT_NAME_CELL.get().map(|s| &s[..]);
        let display_account = self
            .tokens
            .values()
            .any(|token| Some(&token.account_name[..]) != account_name);
        let account_name = (!display_account).then_some(account_name).flatten().unwrap_or("");

        for (id, token) in &mut self.tokens {
            token.draw(ui, id, account_name);
            ui.separator();
        }
        self.add.draw(ui);
        let url = || with_i18n!("api-link", |msg| msg.into_owned());
        with_i18n!("api-setup-open", |label| RenderState::draw_open_button(
            ui, label, url, url
        ));
        if let _font = NexusLinkFont::Ui.push_font(ui) {
            with_i18n!("api-notice", |msg| ui.text_wrapped(msg))
        }
        #[cfg(todo = "unnecessary")]
        if with_i18n!("reset", |msg| ui.small_button(msg)) {
            // TODO: confirmation popup
            let _ = rt::send_alert(ui, "clearing all tokens...");
            ApiMessage::TokenClear.try_send();
        }

        ui.spacing();
        ui.separator();
        ui.spacing();
        ui.text_wrapped(fl!("experimental-notice"));
    }

    pub fn sync_boot(&mut self) {
        let boot = self.boot_state.borrow_and_update();
        self.add.clear();

        self.tokens
            .retain(|id, _| boot.anet_api_token.iter().any(|token| &token.id == id));
        for token in &boot.anet_api_token {
            if self.tokens.contains_key(&token.id) {
                continue
            }
            self.tokens.insert(token.id.clone(), ApiTokenState::new(token));
        }
    }

    pub fn sync_save(&mut self) {
        let save = self.save_state.borrow_and_update();
        self.auto_update = save.api_auto_update;
    }
}

pub struct ApiTokenAdd {
    token: ApiTokenInput,
    status: Option<TokenStatus>,
}
impl ApiTokenAdd {
    pub fn new() -> Self {
        Self {
            token: ApiTokenInput::new(),
            status: None,
        }
    }

    pub fn draw<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        match self.status {
            None => {
                self.draw_input(ui);
            },
            Some(TokenStatus::Setup) => {
                with_i18n!("api-status-setup", |msg| ui
                    .text_with_font(NexusLinkFont::Big, msg));
            },
        }
    }

    pub fn draw_input<'ui, U>(&mut self, ui: &mut U)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        match self.token.draw(ui, "api-token-label") {
            None => (),
            Some(token) if token.is_empty() => (),
            Some(token) => {
                let mut token = SavedApiToken::new(token);
                if let Some(lang) = current_game_language() {
                    token.locale = lang.language.as_str().into();
                }
                ApiMessage::TokenAdd(token).try_send();
                self.status = Some(TokenStatus::Setup);
            },
        }
    }

    pub fn clear(&mut self) {
        self.token.update_preview(false);
        self.status = None;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TokenStatus {
    Setup,
}

pub struct ApiTokenState {
    name: String,
    account_name: String,
    permissions: BTreeSet<String>,
}
impl ApiTokenState {
    pub fn new(token: &SavedApiToken) -> Self {
        let name = token
            .name()
            .map(ToOwned::to_owned)
            .or_else(|| token.id().and_then(|id| id.get(..6).map(|id| format!("{id}..."))))
            .unwrap_or_else(|| with_i18n!("api-token-label", |label| label.into_owned()));
        Self {
            name,
            account_name: token.account_name.clone(),
            permissions: token.permissions.clone(),
        }
    }

    pub fn draw<'ui, U>(&mut self, ui: &mut U, id: &str, account_name: &str)
    where
        U: ?Sized + ImDrawWindow<'ui>,
    {
        ui.text_with_font(NexusLinkFont::Big, &self.name);
        let subheader = ui.item_rect_max();
        if !self.account_name.is_empty() && self.account_name != account_name {
            let is_suffix = |c: char| c == '.' || c.is_ascii_digit();
            let name = match self.account_name.split_once(is_suffix) {
                Some((prefix, rest)) if !rest.is_empty() => Some(prefix),
                _ => self.name.get(..5),
            };
            if let Some(name) = name {
                ui.same_line();
                ui.text(name);
            }
        }
        ui.same_line();
        if with_i18n!("delete", |label| ui.small_button(label)) {
            ApiMessage::TokenRemove(id.into()).try_send();
            let _ = rt::send_alert(ui, "token removed");
        }
        match ui.item_rect_max().y {
            text_y if text_y >= subheader.y => (),
            text_y => {
                ui.set_cursor_screen_pos(subheader.with_y(text_y));
            },
        }
        // TODO: this could just be a hover tooltip...
        #[cfg(todo = "unnecessary")]
        {
            with_i18n!("api-permissions", |msg| ui.text(msg));
            ui.same_line();
        }
        for (i, perm) in self.permissions.iter().enumerate() {
            if i > 0 {
                ui.same_line();
            }
            ui.text(perm);
        }
        #[cfg(todo = "unnecessary")]
        if self.permissions.is_empty() {
            with_i18n!("unset", |msg| ui.text(msg));
        }
        ui.dummy([4.0, 0.0]);

        if let _font = NexusLinkFont::Ui.push_font(ui) {
            with_i18n!("api-refresh", |msg| ui.text(msg));

            for (i, &endpoint) in ApiAccountInfo::VARIANTS.iter().enumerate() {
                if i > 0 || true {
                    ui.same_line();
                }
                if with_i18n(&format!("api-refresh-{endpoint}"), |msg| ui.button(msg)) {
                    ApiMessage::RefreshAccount { endpoint, token_id: Some(id.into()) }.try_send();
                }
            }
        }
    }

    pub fn sync(&mut self, token: &SavedApiToken) {
        self.account_name = token.account_name.clone();
        self.permissions = token.permissions.clone();
        self.name = token
            .name()
            .map(ToOwned::to_owned)
            .or_else(|| token.id().and_then(Self::id_name))
            .unwrap_or_else(|| with_i18n!("api-token-label", |label| label.into_owned()));
        #[cfg(todo)]
        {
            self.token.preview = self.token_name.clone();
        }
    }

    pub fn id_name(id: &str) -> Option<String> {
        id.get(..6).map(|id| format!("{id}..."))
    }
}
