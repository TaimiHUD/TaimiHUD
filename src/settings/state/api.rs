use {
    crate::{
        exports::runtime as rt,
        settings::state::{install::IvId, BootstrapState},
    },
    anyhow::Context,
    serde::{Deserialize, Serialize},
    std::{borrow::Cow, collections::BTreeSet, fmt},
    taimi_hoard::{str_opt, str_opt_ref},
};

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SavedApiToken {
    #[serde(default, skip_serializing_if = "SavedApiSecret::is_empty")]
    pub token: SavedApiSecret,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub base_url: String,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub id: String,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub name: String,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub account_id: String,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub account_name: String,
    #[serde(default, skip_serializing_if = "str::is_empty")]
    pub locale: String,
    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    pub permissions: BTreeSet<String>,
    #[serde(default, skip_serializing_if = "taimi_hoard::is_false_ref")]
    pub is_subtoken: bool,
}

impl SavedApiToken {
    pub const UNAUTHENTICATED: Self = Self {
        token: SavedApiSecret::Empty,
        base_url: String::new(),
        id: String::new(),
        name: String::new(),
        account_id: String::new(),
        account_name: String::new(),
        locale: String::new(),
        permissions: BTreeSet::new(),
        is_subtoken: false,
    };

    pub fn new<T: Into<SavedApiSecret>>(token: T) -> Self {
        Self {
            token: token.into(),
            ..Self::UNAUTHENTICATED
        }
    }

    pub const ANET_BASE_URL: &'static str = "https://api.guildwars2.com/";
    pub fn base_url(&self) -> &str {
        str_opt_ref(&self.base_url).unwrap_or(Self::ANET_BASE_URL)
    }
    pub fn token(&self) -> Option<Cow<'_, str>> {
        rt::log::error_ok(self.token.get())
    }
    pub fn locale(&self) -> Option<&str> {
        str_opt_ref(&self.locale)
    }
    pub fn id(&self) -> Option<&str> {
        str_opt_ref(&self.id)
    }
    pub fn name(&self) -> Option<&str> {
        str_opt_ref(&self.name)
    }
    pub fn account_id(&self) -> Option<&str> {
        str_opt_ref(&self.account_id)
    }
    pub fn account_name(&self) -> Option<&str> {
        str_opt_ref(&self.account_name)
    }

    pub(super) fn token_by_account_name<'a>(tokens: &'a [Self], acc: &str) -> Option<&'a SavedApiToken> {
        tokens
            .iter()
            .find(|token| token.account_name == acc)
            .or_else(|| match tokens[..] {
                [ref token] if token.account_name.is_empty() || acc.is_empty() => Some(token),
                _ => None,
            })
    }
    pub(super) fn get_token_mut<'a, F: FnMut(&SavedApiToken) -> bool>(
        tokens: &'a mut Vec<Self>,
        criteria: F,
    ) -> &'a mut SavedApiToken {
        let idx = tokens.iter().position(criteria);
        let idx = match idx {
            Some(i) => i,
            None => {
                let i = tokens.len();
                tokens.push(SavedApiToken::UNAUTHENTICATED);
                i
            },
        };
        unsafe { tokens.get_unchecked_mut(idx) }
    }
}

#[derive(Clone, Default, Deserialize, Serialize)]
#[cfg_attr(todo, serde(tag = "secret", content = "saved"))]
pub enum SavedApiSecret {
    #[default]
    Empty,
    Plain(Box<str>),
    Scrambled(ScrambledApiSecret),
}

impl SavedApiSecret {
    pub fn try_scramble(&mut self) -> anyhow::Result<()> {
        let scrambled = {
            let secret = match self {
                Self::Plain(secret) if !secret.is_empty() => &secret[..],
                Self::Empty | Self::Scrambled(..) | Self::Plain(_) => return Ok(()),
            };

            let install = BootstrapState::installation();
            let (initial, random) = install
                .iv
                .initial()
                .and_then(|i| install.iv.random().map(|r| (i, r)))
                .context("random IV data required")?;
            let iv_id = install.iv_id;
            ScrambledApiSecret::with_iv_parts(secret.as_bytes(), iv_id, initial, random)
        };

        *self = Self::Scrambled(scrambled);
        Ok(())
    }

    pub fn get(&self) -> anyhow::Result<Cow<'_, str>> {
        match self {
            Self::Empty => Ok(Cow::Borrowed("")),
            Self::Plain(s) => Ok(Cow::Borrowed(&s[..])),
            Self::Scrambled(scrambled) => {
                let install = BootstrapState::installation();
                let (initial, random) = install
                    .iv
                    .initial()
                    .and_then(|i| install.iv.random().map(|r| (i, r)))
                    .context("random IV data lost")?;
                let iv_id = install.iv_id;
                if iv_id != scrambled.iv_id {
                    anyhow::bail!("IV#{iv_id} mismatched expected ID {}", scrambled.iv_id)
                }
                String::from_utf8(scrambled.decrypt_with(initial, random).into())
                    .context("scrambled data corrupted")
                    .map(Cow::Owned)
            },
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Plain(s) if s.is_empty() => true,
            _ => false,
        }
    }
    pub fn needs_scramble(&self) -> bool {
        match self {
            Self::Plain(..) => true,
            _ => false,
        }
    }
}
impl From<String> for SavedApiSecret {
    fn from(s: String) -> Self {
        str_opt(s)
            .map(String::into_boxed_str)
            .map(Self::Plain)
            .unwrap_or(Self::Empty)
    }
}
impl From<ScrambledApiSecret> for SavedApiSecret {
    fn from(s: ScrambledApiSecret) -> Self {
        Self::Scrambled(s)
    }
}
impl fmt::Debug for SavedApiSecret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let variant = match self {
            Self::Scrambled(..) => "ScrambledApiSecret",
            Self::Plain(s) if !s.is_empty() => "Plain",
            Self::Empty | Self::Plain(_) => "Empty",
        };
        f.debug_tuple("SavedApiSecret").field(&"<sensitive>").finish()
    }
}

pub type ScrambleVersion = u32;
#[derive(Clone, Default, Deserialize, Serialize)]
pub struct ScrambledApiSecret {
    pub iv_id: IvId,
    pub version: ScrambleVersion,
    pub data: Box<[u8]>,
}

impl ScrambledApiSecret {
    pub const VERSION_1: ScrambleVersion = 1;

    pub fn with_iv_parts(secret: &[u8], iv_id: IvId, initial: &[u8], random: &[u8]) -> Self {
        let mut secret = Self {
            iv_id,
            version: Self::VERSION_1,
            data: secret.into(),
        };
        secret.encrypt_data_with(initial, random);
        secret
    }

    pub fn decrypt_with(&self, initial: &[u8], random: &[u8]) -> Box<[u8]> {
        let mut out = self.clone();
        out.decrypt_data_with(initial, random);
        out.data
    }

    /// please grab a crypto library thanks
    pub fn encrypt_data_with(&mut self, initial: &[u8], random: &[u8]) {
        for (out, iv) in self.data.iter_mut().zip(random) {
            *out ^= iv;
        }
    }
    pub fn decrypt_data_with(&mut self, initial: &[u8], random: &[u8]) {
        self.encrypt_data_with(initial, random)
    }
}

impl fmt::Debug for ScrambledApiSecret {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("ScrambledApiSecret").field(&"<sensitive>").finish()
    }
}
