use {
    crate::{
        exports::runtime::{self as rt, log::DeferredLogger},
        LocalizationsEmbed,
    },
    anyhow::Context,
    core::cell::RefCell,
    i18n_embed::{
        fluent::{fluent_language_loader, FluentLanguageLoader},
        I18nAssets,
        LanguageLoader,
        Localizer,
    },
    std::{
        borrow::Cow,
        collections::BTreeSet,
        path::PathBuf,
        slice,
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
            LazyLock,
            Mutex,
        },
    },
};

mod r#ref;

pub use i18n_embed::unic_langid::{subtags as unic_subtags, LanguageIdentifier};

pub(crate) use self::r#ref::{i18n_fmt, i18n_ref, I18nRef};

#[inline(always)]
pub fn assets() -> &'static impl I18nAssets {
    #[cfg(taimi_dev = "debug")]
    static LOCALIZATIONS: LazyLock<i18n_embed::RustEmbedNotifyAssets<LocalizationsEmbed>> =
        LazyLock::new(|| {
            i18n_embed::RustEmbedNotifyAssets::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("i18n/"))
        });
    #[cfg(not(taimi_dev = "debug"))]
    static LOCALIZATIONS: LocalizationsEmbed = LocalizationsEmbed;

    match () {
        #[cfg(taimi_dev = "debug")]
        _ => &*LOCALIZATIONS,
        #[cfg(not(taimi_dev = "debug"))]
        _ => &LOCALIZATIONS,
    }
}

pub(crate) static LOADER: LazyLock<FluentLanguageLoader> = LazyLock::new(|| fluent_language_loader!());
pub(crate) fn language_loader_setup(loader: &FluentLanguageLoader) {
    loader.set_use_isolating(false);
    #[cfg(todo)]
    loader.with_bundles_mut(|b| {
        // might be needed for fluent 0.17..
        let res = b.add_builtins().context("Failed to add i18n/fluent builtins");
        if let Err(e) = res {
            log::warn!("{e:#}");
        }
    });
}

#[macro_export]
macro_rules! fl {
    (@compile_error; $message_id:literal) => {
        {
            #[cfg(debug_assertions)]
            let _ = $crate::fl!(@String; $message_id);
        }
    };
    (@compile_error; $message_id:literal, $($args:expr),*) => {
        {
            #[cfg(debug_assertions)]
            let _ = $crate::fl!(@String; $message_id, $($args),*);
        }
    };
    (@String; $message_id:literal) => {{
        i18n_embed_fl::fl!($crate::render::i18n::LOADER, $message_id)
    }};
    (@String; $message_id:literal, $($args:expr),*) => {{
        i18n_embed_fl::fl!($crate::render::i18n::LOADER, $message_id, $($args), *)
    }};
    ($id:literal) => {
        $crate::render::i18n::i18n_fmt! {
            $id
        }
    };
    ($id:literal, $($args:tt)*) => {
        $crate::render::i18n::i18n_fmt! {
            $id => $($args)*
        }
    };
    ($id:expr, $($args:tt)*) => {
        $crate::render::i18n::i18n_fmt! {
            $id => $($args)*
        }
    };
    ($id:expr) => {
        $crate::render::i18n::i18n_fmt! {
            $id
        }
    };
}

static I18N_WARN_ONCE: Mutex<BTreeSet<String>> = Mutex::new(BTreeSet::new());
static I18N_INIT_LOADED: AtomicBool = AtomicBool::new(false);
pub fn with_i18n<R, F>(message_id: &str, f: F) -> R
where
    F: FnOnce(Cow<str>) -> R,
{
    with_i18n_message(message_id, |m, errors| {
        let msg = m.and_then(|(m, b)| m.value().map(|p| b.format_pattern(p, None, errors)));

        match msg {
            Some(m) => f(m),
            None => f(Cow::Borrowed(message_id)),
        }
    })
}
pub type FluentErrors = Vec<fluent::FluentError>;
pub type FluentBundle = fluent::concurrent::FluentBundle<Arc<fluent::FluentResource>>;
pub fn with_i18n_message<R, F>(message_id: &str, f: F) -> R
where
    F: FnOnce(Option<(fluent::FluentMessage, &FluentBundle)>, &mut FluentErrors) -> R,
{
    // XXX: why does this not take an FnOnce...
    let mut errors = Vec::new();
    let f = RefCell::new((Some(f), &mut errors));
    let res = {
        LOADER.with_fluent_message_and_bundle(message_id, |m, b| {
            let mut f = f.try_borrow_mut();
            let (f, errors) = match &mut f {
                #[cfg(debug_assertions)]
                f => f.as_mut().ok().and_then(|f| {
                    let (f, e) = &mut **f;
                    f.take().map(move |f| (f, e))
                })?,
                #[cfg(not(debug_assertions))]
                Ok(f) => unsafe {
                    let (f, e) = &mut **f;
                    (f.take().unwrap_unchecked(), e)
                },
                #[cfg(not(debug_assertions))]
                Err(..) => unsafe { core::hint::unreachable_unchecked() },
            };
            Some(f(Some((m, b)), errors))
        })
    };
    let f = f.into_inner().0;
    let (res, is_ok) = match (res, f) {
        (Some(Some(r)), _) => (r, errors.is_empty()),
        (None | Some(None), Some(f)) => (f(None, &mut errors), false),
        (_, None) => unreachable!("with_message calls once"),
    };
    let warn_once = match is_ok {
        true => false,
        false if !I18N_INIT_LOADED.load(Ordering::Relaxed) => false,
        false => match I18N_WARN_ONCE.try_lock() {
            Ok(once) if once.contains(message_id) => false,
            Ok(mut once) => {
                once.insert(message_id.into());
                true
            },
            Err(..) => false,
        },
    };
    if warn_once {
        let severity = match crate::built_info::git_tag_name().is_some() {
            true => log::Level::Debug,
            false => log::Level::Warn,
        };
        match errors.is_empty() {
            true =>
                log::log!(logger: DeferredLogger::BEST_EFFORT, severity, "missing i18n message {message_id}"),
            false =>
                for e in errors {
                    log::log!(logger: DeferredLogger::BEST_EFFORT, severity, "malformed i18n message {message_id}: {e}");
                },
        }
    }
    res
}
#[inline]
pub fn with_current_bundle<R, F: FnOnce(&FluentBundle) -> R>(f: F) -> Option<R> {
    with_bundle_for(MSG_PLACEHOLDER_ALL, f)
}
#[inline]
pub fn with_fallback_bundle<R, F: FnOnce(&FluentBundle) -> R>(f: F) -> Option<R> {
    with_bundle_for(MSG_PLACEHOLDER_FALLBACK, f)
}
fn with_bundle_for<R, F: FnOnce(&FluentBundle) -> R>(msg: &str, f: F) -> Option<R> {
    let f = RefCell::new(Some(f));
    let res = LOADER.with_fluent_message_and_bundle(msg, |_m, b| {
        let f = match f.try_borrow_mut() {
            #[cfg(debug_assertions)]
            mut f => f
                .as_mut()
                .ok()
                .and_then(|f| f.take())
                .expect("with_message not FnOnce"),
            #[cfg(not(debug_assertions))]
            f => unsafe { f.unwrap_unchecked().take().unwrap_unchecked() },
        };
        f(b)
    });
    res
}
#[macro_export]
macro_rules! with_i18n {
    ($message_id:literal, $closure:expr) => {
        {
            'with_i18n_: {
                #![allow(unreachable_code)]
                let res = $crate::render::i18n::with_i18n($message_id, $closure);
                break 'with_i18n_ res;
                // still check ID at compile time...
                #[cfg(debug_assertions)]
                let _ = $crate::fl!($message_id);
            }
        }
    };
    (($message_id:expr, $($rest_id:tt)+), |($arg:ident, $($rest_arg:tt)*)| $closure:expr) => {
        $crate::with_i18n! {
            $message_id, |$arg| $crate::with_i18n! {
                $($rest_id)*,
                |$($rest_arg)*| $closure
            }
        }
    };
    ($message_id:expr, $closure:expr) => {
        {
            $crate::render::i18n::with_i18n($message_id, $closure)
        }
    };
}

/// idk what this is for but try to avoid using its fields as accessors to [LOADER] and [assets]
/// (indirection via `&dyn LanguageLoader` to a global singleton would be dumb)
static LOCALIZER: LazyLock<i18n_embed::DefaultLocalizer<'static>> = LazyLock::new(|| {
    let localizer = || i18n_embed::DefaultLocalizer::new(&*LOADER, assets());
    match localizer() {
        #[cfg(taimi_dev = "debug")]
        l => l.with_autoreload().unwrap_or_else(|e| {
            log::warn!("i18n autoreload init failed: {e}");
            localizer()
        }),
        #[cfg(not(taimi_dev = "debug"))]
        l => l,
    }
});

macro_rules! new_lang_id {
    (Language: $lang:expr) => {
        unsafe {
            $crate::render::i18n::unic_subtags::Language::from_raw_unchecked(
                u64::from_le_bytes(*$crate::render::i18n::new_lang_id!(TinyAsciiStr<8>: $lang).all_bytes())
            )
        }
    };
    (Region: $region:expr) => {
        unsafe {
            $crate::render::i18n::unic_subtags::Region::from_raw_unchecked(
                u32::from_le_bytes(*$crate::render::i18n::new_lang_id!(TinyAsciiStr<4>: $region).all_bytes())
            )
        }
    };
    (Script: $script:expr) => {
        unsafe {
            $crate::render::i18n::unic_subtags::Script::from_raw_unchecked(
                u32::from_le_bytes(*$crate::render::i18n::new_lang_id!(TinyAsciiStr<4>: $script).all_bytes())
            )
        }
    };
    (Variant: $variant:expr) => {
        unsafe {
            $crate::render::i18n::unic_subtags::Variant::from_raw_unchecked(
                u64::from_le_bytes(*$crate::render::i18n::new_lang_id!(TinyAsciiStr<8>: $variant).all_bytes())
            )
        }
    };
    (TinyAsciiStr<$n:literal>: $tag:expr) => {
        match tinystr::TinyAsciiStr::<$n>::try_from_utf8($tag.as_bytes()) {
            Ok(s) => s,
            Err(..) => panic!("bad lang tag"),
        }
    };
    ($lang:tt-$region:tt-$(*)?) => {
        LanguageIdentifier::from_raw_parts_unchecked(
            $crate::render::i18n::new_lang_id!(Language: stringify!($lang)),
            None,
            Some($crate::render::i18n::new_lang_id!(Region: stringify!($region))),
            None,
        )
    };
    ($lang:tt-*-$(*)?) => {
        LanguageIdentifier::from_raw_parts_unchecked(
            $crate::render::i18n::new_lang_id!(Language: stringify!($lang)),
            None,
            Some($crate::render::i18n::new_lang_id!(Region: stringify!($region))),
            None,
        )
    };
}
pub(crate) use new_lang_id;

pub static LANGUAGES_GAME: [LanguageIdentifier; 5] = [LANG_EN, LANG_FR, LANG_DE, LANG_ES, LANG_ZH];

pub const LANG_EN: LanguageIdentifier = new_lang_id!(en-*-);
pub const LANG_KO: LanguageIdentifier = new_lang_id!(ko-KR-);
pub const LANG_FR: LanguageIdentifier = new_lang_id!(fr-FR-);
pub const LANG_DE: LanguageIdentifier = new_lang_id!(de-DE-);
pub const LANG_ES: LanguageIdentifier = new_lang_id!(es-ES-);
pub const LANG_ZH: LanguageIdentifier = new_lang_id!(zh-CN-);

/// a message id guaranteed to only be present in the fallback language (english)
pub(crate) const MSG_PLACEHOLDER_FALLBACK: &'static str = "locale-native-en";
#[cfg(todo)]
const MSG_PLACEHOLDER_SENTINEL: &'static str = "locale-native-fallback";
/// a message id guaranteed be present in all languages
pub(crate) const MSG_PLACEHOLDER_ALL: &'static str = "locale-name";

pub fn load_language(language: &LanguageIdentifier) -> anyhow::Result<()> {
    let requested = slice::from_ref(language);
    let changing = LOADER.current_language() != *language;
    LOCALIZER
        .select(requested)
        .with_context(|| format!("Loading language {language}"))?;
    language_loader_setup(&LOADER);
    if changing && LOADER.current_language() == *language {
        log::info!("Selected language {language}");
        #[cfg(todo)]
        let fallback_fill = LOADER.with_fluent_message_and_bundle(MSG_PLACEHOLDER_SENTINEL, |_, b| {
            b.locales.get(0) == Some(language)
        }) == Some(false);
        #[cfg(todo)]
        if fallback_fill {
            // fill in missing keys so that english fallback patterns will interpolate
            // using the target language where possible.
            // TODO: i18n-embed doesn't expose the fluent resources, so we can't do the straightforward thing
            // here and just add_resource() the fallback... :<
            let msgs = with_current_bundle(|b| {
                LOADER.with_message_iter(LOADER.fallback_language, |m| {
                    m.filter(|m| !b.has_message(m.id())).collect()
                })
            });
            LOADER.with_bundles_mut(|b| {
                if b.locales.get(0) != Some(language) {
                    return
                }
            })
        }
    }
    I18N_INIT_LOADED.store(true, Ordering::Relaxed);
    Ok(())
}

/// current language, if it intersects with [LANGUAGES_GAME]
pub fn current_game_language() -> Option<LanguageIdentifier> {
    if let Ok(Some(lang)) = rt::game_language() {
        return Some(lang)
    }

    let lang = LOADER.current_language();
    if lang.language == LANG_EN.language && !rt::language_explicitly_set() {
        return None
    }
    if LANGUAGES_GAME.iter().any(|l| l.language == lang.language) {
        Some(lang)
    } else {
        None
    }
}
#[inline(always)]
pub fn current_language() -> LanguageIdentifier {
    LOADER.current_language()
}
#[inline(always)]
pub fn fallback_language() -> &'static LanguageIdentifier {
    LOADER.fallback_language()
}
#[inline(always)]
pub fn available_languages() -> &'static [LanguageIdentifier] {
    static AVAILABLE: LazyLock<Vec<LanguageIdentifier>> =
        LazyLock::new(|| rt::log::warn_ok(LOCALIZER.available_languages()).unwrap_or_default());
    &*AVAILABLE
}

pub fn language_to_string(id: &LanguageIdentifier) -> Cow<'_, str> {
    match id {
        LanguageIdentifier {
            language, script: None, region: None, ..
        } if id.variants().len() == 0 => Cow::Borrowed(language.as_str()),
        lang => Cow::Owned(lang.to_string()),
    }
}
