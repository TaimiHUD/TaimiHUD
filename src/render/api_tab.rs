use {
    crate::{
        controller::api::{ApiAccountInfo, ApiMessage},
        exports::runtime::{
            self as rt,
            imgui::Ui,
        },
        render::{element::token::ApiTokenInput, RenderState},
        settings::state::{BootstrapState, SavedApiToken},
        with_i18n,
    }, glam::Vec2, std::collections::{BTreeMap, BTreeSet, HashMap}, strum::VariantArray, tokio::sync::watch,
};

pub struct ApiTabState {
    boot_state: watch::Receiver<BootstrapState>,
    tokens: BTreeMap<String, ApiTokenState>,
    add: ApiTokenAdd,
}

impl ApiTabState {
    pub fn new() -> Self {
        let mut state = Self {
            boot_state: BootstrapState::get().subscribe(),
            tokens: BTreeMap::new(),
            add: ApiTokenAdd::new(),
        };
        state.boot_state.mark_changed();
        state
    }

    pub fn draw(&mut self, ui: &Ui, _state_errors: &mut HashMap<String, anyhow::Error>) {
        if self.boot_state.has_changed().ok() == Some(true) {
            self.sync_boot();
        }
        let account_name = crate::ACCOUNT_NAME_CELL.get().map(|s| &s[..]);
        let display_account = self.tokens.values().any(|token| Some(&token.account_name[..]) != account_name);
        let account_name = (!display_account).then_some(account_name).flatten()
            .unwrap_or("");
        for (id, token) in &mut self.tokens {
            token.draw(ui, id, account_name);
            ui.separator();
        }
        self.add.draw(ui);
        let url = || with_i18n!("api-link", |msg| msg.into_owned());
        with_i18n!("api-setup-open", |label|
            RenderState::draw_open_button(ui, label, url, url)
        );
        if let _font = RenderState::push_font("ui", ui) {
            with_i18n!("api-notice", |msg| ui.text_wrapped(msg))
        }
        #[cfg(todo = "unnecessary")]
        if with_i18n!("reset", |msg| ui.small_button(msg)) {
            // TODO: confirmation popup
            let _ = rt::send_alert(ui, "clearing all tokens...");
            ApiMessage::TokenClear.try_send();
        }
    }

    pub fn sync_boot(&mut self) {
        let boot = self.boot_state.borrow_and_update();
        self.add.clear();

        self.tokens.retain(|id, _| boot.anet_api_token.iter().any(|token| &token.id == id));
        for token in &boot.anet_api_token {
            if self.tokens.contains_key(&token.id) {
                continue
            }
            self.tokens.insert(token.id.clone(), ApiTokenState::new(token));
        }
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

    pub fn draw(&mut self, ui: &Ui) {
        match self.status {
            None => {
                self.draw_input(ui);
            },
            Some(TokenStatus::Setup) => {
                let _font = RenderState::push_font("big", ui);
                with_i18n!("api-status-setup", |msg| ui.text(&msg));
            },
        }
    }

    pub fn draw_input(&mut self, ui: &Ui) {
        match self.token.draw(ui, "api-token-label") {
            None => (),
            Some(token) if token.is_empty() => (),
            Some(token) => {
                let mut token = SavedApiToken::new(token);
                if let Some(lang) = rt::game_language() {
                    token.locale = crate::game_language_id(lang).into();
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
        let name = token.name()
            .map(ToOwned::to_owned)
            .or_else(|| token.id().and_then(|id| id.get(..6).map(|id| format!("{id}..."))))
            .unwrap_or_else(|| with_i18n!("api-token-label", |label| label.into_owned()));
        Self {
            name,
            account_name: token.account_name.clone(),
            permissions: token.permissions.clone(),
        }
    }

    pub fn draw(&mut self, ui: &Ui, id: &str, account_name: &str) {
        RenderState::font_text("big", ui, &self.name);
        let subheader = Vec2::from(ui.item_rect_max());
        if !self.account_name.is_empty() && self.account_name != account_name {
            let is_suffix = |c: char| c == '.' || c.is_ascii_digit();
            let name = match self.account_name.split_once(is_suffix) {
                Some((prefix, rest)) if !rest.is_empty() =>
                    Some(prefix),
                _ => {
                    self.name.get(..5)
                },
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
        match ui.item_rect_max()[1] {
            text_y if text_y >= subheader.y => (),
            text_y => {
                ui.set_cursor_screen_pos(subheader.with_y(text_y).to_array());
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

        if let _font = RenderState::push_font("ui", ui) {
            with_i18n!("api-refresh", |msg| ui.text(msg));

            for (i, &endpoint) in ApiAccountInfo::VARIANTS.iter().enumerate() {
                if i > 0 || true {
                    ui.same_line();
                }
                if with_i18n(&format!("api-refresh-{endpoint}"), |msg| ui.button(msg)) {
                    ApiMessage::RefreshAccount {
                        endpoint,
                        token_id: Some(id.into()),
                    }.try_send();
                }
            }
        }
    }

    pub fn sync(&mut self, token: &SavedApiToken) {
        self.account_name = token.account_name.clone();
        self.permissions = token.permissions.clone();
        self.name = token.name()
            .map(ToOwned::to_owned)
            .or_else(|| token.id().and_then(|id| id.get(..6).map(|id| format!("{id}..."))))
            .unwrap_or_else(|| with_i18n!("api-token-label", |label| label.into_owned()));
        #[cfg(todo)] {
            self.token.preview = self.token_name.clone();
        }
    }
}
