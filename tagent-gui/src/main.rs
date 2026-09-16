use slint::{Color, ComponentHandle, Model, ModelRc, VecModel};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tagent::{languages, providers};

mod config;
mod platform;

use config::GuiConfigManager;
use platform::window::WindowHandle;
use platform::{ClipboardManager, KeyboardHook};

slint::include_modules!();

thread_local! {
    // The foreground window captured by `show_popup`, to be restored once the
    // popup actually hides (see `show_popup`'s doc comment for why this moved
    // from "restore immediately after show()" to "restore at hide time").
    // `thread_local!` (not `Rc<Cell<_>>` passed around) specifically because
    // `show_popup` is invoked from a closure that has to satisfy `Send` (it's
    // moved through a background thread inside `spawn_translation` before
    // being called back on the UI thread) -- an `Rc` captured there wouldn't
    // compile, but a thread-local needs no such bound, and both the writer
    // (`show_popup`) and the reader (`on_hide_requested`'s handler in
    // `main()`) only ever run on the UI thread anyway.
    static POPUP_RESTORE_TARGET: std::cell::Cell<Option<WindowHandle>> = const { std::cell::Cell::new(None) };
}

/// Font-family choices offered for the phrase/translation style pickers in
/// Settings > View, in the same order as `SettingsDialog.font-options`.
const FONT_FAMILIES: [&str; 3] = ["monospace", "sans-serif", "serif"];

/// Must match `AppWindow`'s `preferred-width`/`preferred-height` in `app.slint`.
/// Re-asserted explicitly in Rust by [`show_window_restoring_geometry`] as a fix for
/// a real initial-window-sizing race -- see that function's doc comment.
const DEFAULT_WINDOW_SIZE: slint::PhysicalSize = slint::PhysicalSize::new(480, 480);

/// How long the global hotkey stays suppressed after the Settings dialog's
/// "Record" capture starts, if no matching stop signal arrives first. See the
/// `recording_started_at` doc comment in `main()` for why this exists.
const RECORDING_SUPPRESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

fn font_index_for(family: &str) -> i32 {
    FONT_FAMILIES.iter().position(|f| *f == family).unwrap_or(0) as i32
}

/// One preset entry for the "Color scheme" picker in Settings > View: a
/// one-shot bulk-fill for background/phrase/translation colors (plus the
/// matching Theme), not a persisted setting of its own — see
/// `SettingsDialog.color-scheme-options` in app.slint.
struct ColorScheme {
    name: &'static str,
    dark: bool,
    background: &'static str,
    phrase_color: &'static str,
    phrase_background: &'static str,
    translation_color: &'static str,
    translation_background: &'static str,
}

const COLOR_SCHEMES: &[ColorScheme] = &[
    ColorScheme {
        name: "Solarized Dark",
        dark: true,
        background: "#002B36",
        phrase_color: "#839496",
        phrase_background: "#073642",
        translation_color: "#268BD2",
        translation_background: "#073642",
    },
    ColorScheme {
        name: "Solarized Light",
        dark: false,
        background: "#FDF6E3",
        phrase_color: "#657B83",
        phrase_background: "#EEE8D5",
        translation_color: "#268BD2",
        translation_background: "#EEE8D5",
    },
    ColorScheme {
        name: "Dracula",
        dark: true,
        background: "#282A36",
        phrase_color: "#F8F8F2",
        phrase_background: "#44475A",
        translation_color: "#BD93F9",
        translation_background: "#44475A",
    },
    ColorScheme {
        name: "Nord",
        dark: true,
        background: "#2E3440",
        phrase_color: "#D8DEE9",
        phrase_background: "#3B4252",
        translation_color: "#88C0D0",
        translation_background: "#3B4252",
    },
    ColorScheme {
        name: "Gruvbox Dark",
        dark: true,
        background: "#282828",
        phrase_color: "#EBDBB2",
        phrase_background: "#3C3836",
        translation_color: "#FE8019",
        translation_background: "#3C3836",
    },
    ColorScheme {
        name: "Monokai",
        dark: true,
        background: "#272822",
        phrase_color: "#F8F8F2",
        phrase_background: "#3E3D32",
        translation_color: "#A6E22E",
        translation_background: "#3E3D32",
    },
    ColorScheme {
        name: "One Dark",
        dark: true,
        background: "#282C34",
        phrase_color: "#ABB2BF",
        phrase_background: "#2C313C",
        translation_color: "#61AFEF",
        translation_background: "#2C313C",
    },
    ColorScheme {
        name: "Tokyo Night",
        dark: true,
        background: "#1A1B26",
        phrase_color: "#C0CAF5",
        phrase_background: "#292E42",
        translation_color: "#7AA2F7",
        translation_background: "#292E42",
    },
    ColorScheme {
        name: "Catppuccin Mocha",
        dark: true,
        background: "#1E1E2E",
        phrase_color: "#CDD6F4",
        phrase_background: "#313244",
        translation_color: "#CBA6F7",
        translation_background: "#313244",
    },
    ColorScheme {
        name: "Night Owl",
        dark: true,
        background: "#011627",
        phrase_color: "#D6DEEB",
        phrase_background: "#1D3B53",
        translation_color: "#82AAFF",
        translation_background: "#1D3B53",
    },
    ColorScheme {
        name: "Ayu Dark",
        dark: true,
        background: "#0A0E14",
        phrase_color: "#B3B1AD",
        phrase_background: "#131721",
        translation_color: "#FFB454",
        translation_background: "#131721",
    },
    ColorScheme {
        name: "GitHub Light",
        dark: false,
        background: "#FFFFFF",
        phrase_color: "#24292E",
        phrase_background: "#F6F8FA",
        translation_color: "#0366D6",
        translation_background: "#F6F8FA",
    },
    ColorScheme {
        name: "Gruvbox Light",
        dark: false,
        background: "#FBF1C7",
        phrase_color: "#3C3836",
        phrase_background: "#EBDBB2",
        translation_color: "#D65D0E",
        translation_background: "#EBDBB2",
    },
    ColorScheme {
        name: "Catppuccin Latte",
        dark: false,
        background: "#EFF1F5",
        phrase_color: "#4C4F69",
        phrase_background: "#CCD0DA",
        translation_color: "#8839EF",
        translation_background: "#CCD0DA",
    },
];

/// Parses a `"#RRGGBB"` (or `"RRGGBB"`) string into 0-255 components.
fn parse_hex_color(text: &str) -> Option<(u8, u8, u8)> {
    let text = text.trim();
    let text = text.strip_prefix('#').unwrap_or(text);
    if text.len() != 6 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&text[0..2], 16).ok()?;
    let g = u8::from_str_radix(&text[2..4], 16).ok()?;
    let b = u8::from_str_radix(&text[4..6], 16).ok()?;
    Some((r, g, b))
}

fn format_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{r:02X}{g:02X}{b:02X}")
}

fn resolve_color(hex: &str, default: Color) -> Color {
    match parse_hex_color(hex) {
        Some((r, g, b)) => Color::from_rgb_u8(r, g, b),
        None => default,
    }
}

/// Reads a color-picker field's dialog state back into a `"#RRGGBB"` config
/// value, or `""` when the field is left at "Theme default".
fn color_field_hex(use_default: bool, r: f32, g: f32, b: f32) -> String {
    if use_default {
        String::new()
    } else {
        format_hex(r.round() as u8, g.round() as u8, b.round() as u8)
    }
}

/// Validates a `translate_hotkey` string typed into the Settings dialog,
/// using the same `HotkeyParser` the startup path (`main()`) parses the
/// saved value with. Returns `""` when valid, or the parser's own error
/// message otherwise, for direct display in the dialog's inline error `Text`.
fn hotkey_validation_error(text: &str) -> String {
    match config::HotkeyParser::parse(text)
        .and_then(|h| config::HotkeyParser::validate_hotkey(&h))
    {
        Ok(()) => String::new(),
        Err(err) => err,
    }
}

/// Maps one key press's raw `KeyEvent.text` (from the Settings dialog's
/// "Record" hotkey capture, see `HotkeyRecorder` in `app.slint`) to the key-name
/// vocabulary `platform::keycodes::key_name_to_vk` accepts, or `""` if the
/// key isn't one it recognizes (most commonly: a non-Latin keyboard layout
/// producing a non-ASCII character for what should be a plain letter key).
/// Only covers real keys, never modifiers or Escape — `HotkeyRecorder`
/// filters those out in `.slint` before this is ever called.
fn slint_key_text_to_hotkey_token(text: &str) -> String {
    use slint::platform::Key;

    let mut chars = text.chars();
    let (Some(ch), None) = (chars.next(), chars.next()) else {
        return String::new();
    };

    if ch.is_ascii_alphanumeric() {
        return ch.to_ascii_uppercase().to_string();
    }

    const NAMED_KEYS: &[(Key, &str)] = &[
        (Key::F1, "F1"),
        (Key::F2, "F2"),
        (Key::F3, "F3"),
        (Key::F4, "F4"),
        (Key::F5, "F5"),
        (Key::F6, "F6"),
        (Key::F7, "F7"),
        (Key::F8, "F8"),
        (Key::F9, "F9"),
        (Key::F10, "F10"),
        (Key::F11, "F11"),
        (Key::F12, "F12"),
        (Key::Space, "Space"),
        (Key::Tab, "Tab"),
        (Key::Return, "Enter"),
        (Key::Backspace, "Backspace"),
        (Key::Delete, "Delete"),
        (Key::Insert, "Insert"),
        (Key::Home, "Home"),
        (Key::End, "End"),
        (Key::PageUp, "PageUp"),
        (Key::PageDown, "PageDown"),
        (Key::LeftArrow, "Left"),
        (Key::RightArrow, "Right"),
        (Key::UpArrow, "Up"),
        (Key::DownArrow, "Down"),
    ];
    NAMED_KEYS
        .iter()
        .find(|(key, _)| char::from(*key) == ch)
        .map(|(_, name)| name.to_string())
        .unwrap_or_default()
}

/// Applies the theme and the phrase/translation display style from `config`
/// to `window`. Colors left at "theme default" (empty string in config) are
/// resolved against the window's own theme-driven `panel-foreground`/
/// `panel-background`, read back *after* the theme is applied so an "auto"
/// theme resolves to whatever the system's current dark/light preference is.
///
/// `panel-background` itself is resolved first (against the raw theme value,
/// `panel-background-theme-default`) since it's now also user-customizable —
/// phrase-background/translation-background's own "theme default" then
/// follows whatever `panel-background` ends up being, custom or not, so the
/// three stay visually consistent.
fn apply_style(window: &AppWindow, config: &config::GuiConfig) {
    window.invoke_apply_theme(config.theme.clone().into());

    let panel_background_theme_default = window.get_panel_background_theme_default().color();
    window.set_panel_background(resolve_color(
        &config.background_color,
        panel_background_theme_default,
    ));

    let default_fg = window.get_panel_foreground().color();
    let default_bg = window.get_panel_background();

    window.set_phrase_font(config.phrase_font.clone().into());
    window.set_phrase_size(config.phrase_size);
    window.set_phrase_color(resolve_color(&config.phrase_color, default_fg));
    window.set_phrase_background(resolve_color(&config.phrase_background, default_bg));

    window.set_translation_font(config.translation_font.clone().into());
    window.set_translation_size(config.translation_size);
    window.set_translation_color(resolve_color(&config.translation_color, default_fg));
    window.set_translation_background(resolve_color(&config.translation_background, default_bg));

    window.set_block_spacing_px(config.block_spacing_px);
    window.set_phrases_spacing_px(config.phrases_spacing_px);
}

/// Applies the theme and the phrase/translation display style to the Stage 6 popup —
/// the same subset of [`apply_style`] that's meaningful for it (no panel-background,
/// no block/phrase spacing, since the popup only ever shows one phrase/translation
/// pair). Kept as its own small function rather than widening `apply_style` to
/// branch on window type, since the two windows' style surfaces only partially
/// overlap.
fn apply_popup_style(popup: &TranslationPopup, config: &config::GuiConfig) {
    popup.invoke_apply_theme(config.theme.clone().into());

    let default_fg = popup.get_panel_foreground().color();
    let default_bg = popup.get_panel_background_theme_default().color();

    popup.set_phrase_font(config.phrase_font.clone().into());
    popup.set_phrase_size(config.phrase_size);
    popup.set_phrase_color(resolve_color(&config.phrase_color, default_fg));
    popup.set_phrase_background(resolve_color(&config.phrase_background, default_bg));

    popup.set_translation_font(config.translation_font.clone().into());
    popup.set_translation_size(config.translation_size);
    popup.set_translation_color(resolve_color(&config.translation_color, default_fg));
    popup.set_translation_background(resolve_color(&config.translation_background, default_bg));
}

/// Shows the Stage 6 popup with `entry`'s text, positioned next to the current mouse
/// cursor, and (re)starts its auto-hide timer. Called from the hotkey path's
/// `spawn_translation` completion callback, already on the UI thread.
///
/// The foreground window is captured here (before `popup.show()` changes it) but
/// deliberately **not** restored immediately after — that was the original design
/// (to stop the popup from ever holding keyboard focus, even transiently), but it
/// broke visibility entirely: `set_foreground_window` on Linux raises the target via
/// `XMapRaised` as well as focusing it, and once the popup is correctly positioned
/// right where the cursor is (i.e. right over the app the user was just using),
/// raising that app straight back put it right back on top of the popup, hiding it
/// completely. Restoring focus is instead deferred to `on_hide_requested`'s handler
/// in `main()` (via `POPUP_RESTORE_TARGET`, a thread-local since this value has to
/// survive a trip through a `Send`-bounded closure — see that constant's doc
/// comment), matching `tagent-cli`'s own `hide_terminal_and_restore`, which restores
/// focus only when actually hiding its own popup, not right after showing it. This
/// reopens a narrower version of the original concern (a hotkey re-trigger *while
/// the popup is still visible* could still copy from the popup, not the real source
/// app) — accepted, since it's the same tradeoff `tagent-cli` already lives with for
/// its own terminal popup, and strictly better than the popup never being visible.
///
/// The auto-hide timer deliberately lives inside the `.slint` component itself
/// (a `Timer` element), not as a `slint::Timer` held in `main()`: `on_done` (this
/// function's caller, indirectly) is a `Box<dyn FnOnce(&TranscriptEntry) + Send>` that
/// `spawn_translation` moves through a background thread before calling it back on the
/// UI thread, and `slint::Timer` is `!Send` — it can't be captured into that closure at
/// all, regardless of how carefully it'd actually be used only on the UI thread.
/// `slint::Weak<TranslationPopup>` (used here) has no such restriction.
fn show_popup(
    popup_weak: &slint::Weak<TranslationPopup>,
    entry: &TranscriptEntry,
    auto_hide_seconds: u64,
) {
    let Some(popup) = popup_weak.upgrade() else {
        return;
    };

    POPUP_RESTORE_TARGET.with(|cell| cell.set(platform::window::foreground_window()));

    popup.set_phrase_text(entry.phrase.clone());
    popup.set_translation_text(entry.translation.clone());

    popup.show().ok();

    // Positioned *after* show(), not before: the Stage 6 plan flagged this
    // ordering as needing empirical verification, and it turned out
    // position-before-show is the one that doesn't work -- on this X11 setup
    // it was silently ignored (no OS-level window exists yet when
    // set_position is called), leaving the popup at whatever position a
    // freshly mapped no-frame window defaults to (observed: the screen's
    // top-left corner), not the cursor. No monitor-edge clamping in this
    // stage -- accepted limitation, see the Stage 6 plan. A `None` cursor
    // position (unsupported window manager) just leaves the popup wherever
    // show() placed it.
    if let Some((x, y)) = platform::window::cursor_position() {
        popup
            .window()
            .set_position(slint::PhysicalPosition::new(x + 16, y + 16));
    }

    popup.set_auto_hide_seconds(auto_hide_seconds.min(i32::MAX as u64) as i32);
    popup.invoke_start_hide_timer();
}

/// Formats one transcript line, with or without its "[Auto]:"-style prompt.
///
/// The prompt is baked directly into the string (rather than kept as a
/// separately styled element) because Slint's plain `Text`/`TextInput` can't
/// mix two styles within one wrapped paragraph — an earlier attempt at a
/// separately colored/sized prompt element made the text wrap flush under the
/// prompt (hanging indent) instead of flush from the row's left margin like a
/// normal paragraph, which didn't match the desired look.
fn format_line(show_prompt: bool, lang: &str, text: &str) -> String {
    if show_prompt {
        format!("[{lang}]: {text}")
    } else {
        text.to_string()
    }
}

/// Populates one `ColorPickerField`'s dialog-side state from a `"#RRGGBB"` (or
/// empty, for "theme default") config value. Used five times (the shared
/// panel background, plus phrase/translation × text/background) — see the
/// matching macro call sites below.
macro_rules! init_color_field {
    ($dialog:expr, $hex:expr, $set_default:ident, $set_r:ident, $set_g:ident, $set_b:ident, $set_hex:ident) => {{
        let hex_value = $hex;
        match parse_hex_color(hex_value) {
            Some((r, g, b)) => {
                $dialog.$set_default(false);
                $dialog.$set_r(r as f32);
                $dialog.$set_g(g as f32);
                $dialog.$set_b(b as f32);
                $dialog.$set_hex(hex_value.to_string().into());
            }
            None => {
                $dialog.$set_default(true);
            }
        }
    }};
}

/// Wires one `ColorPickerField`'s `hex-committed` callback: parses the typed
/// hex text and reflects it into the field's RGB sliders. Used five times.
macro_rules! wire_hex_committed {
    ($dialog:expr, $on_committed:ident, $set_r:ident, $set_g:ident, $set_b:ident, $set_default:ident) => {{
        let dialog_weak = $dialog.as_weak();
        $dialog.$on_committed(move |text| {
            if let (Some(dialog), Some((r, g, b))) =
                (dialog_weak.upgrade(), parse_hex_color(text.as_str()))
            {
                dialog.$set_r(r as f32);
                dialog.$set_g(g as f32);
                dialog.$set_b(b as f32);
                dialog.$set_default(false);
            }
        });
    }};
}

/// Wires one `ColorPickerField`'s `rgb-changed` callback (fired on every
/// slider drag): reformats the field's current red/green/blue into
/// `"#RRGGBB"` and writes it back into the hex text, so the hex field doesn't
/// go stale while dragging sliders. Used five times.
macro_rules! wire_rgb_changed {
    ($dialog:expr, $on_changed:ident, $get_r:ident, $get_g:ident, $get_b:ident, $set_hex:ident) => {{
        let dialog_weak = $dialog.as_weak();
        $dialog.$on_changed(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                let hex = format_hex(
                    dialog.$get_r().round() as u8,
                    dialog.$get_g().round() as u8,
                    dialog.$get_b().round() as u8,
                );
                dialog.$set_hex(hex.into());
            }
        });
    }};
}

fn scroll_transcript_to_bottom(window: &AppWindow) {
    let overflow = window.get_transcript_viewport_height() - window.get_transcript_visible_height();
    window.set_transcript_viewport_y(if overflow > 0.0 { -overflow } else { 0.0 });
}

/// Appends one entry to the transcript and scrolls to show it.
///
/// Rebuilds the backing model rather than mutating a shared `VecModel`
/// because the new entry is produced on a background translation thread and
/// handed back via `slint::invoke_from_event_loop`, whose closure must be
/// `Send` — an `Rc<VecModel<_>>` captured in that closure wouldn't be, but
/// `AppWindow::as_weak()` is, so entries are read back from the window itself
/// instead of a separately shared model.
fn push_transcript_entry(window: &AppWindow, entry: TranscriptEntry) {
    let mut entries: Vec<TranscriptEntry> = window.get_transcript_entries().iter().collect();
    entries.push(entry);
    window.set_transcript_entries(ModelRc::new(VecModel::from(entries)));
    scroll_transcript_to_bottom(window);
}

/// Everything [`spawn_translation`] needs, grouped into one struct rather than passed as
/// separate arguments (clippy's `too_many_arguments` threshold is 7; this is naturally
/// more than that once both the display names and the resolved provider codes are
/// included). `from_lang`/`to_lang` are the human-readable names used for the
/// transcript's "[Lang]:"-style prompt; `from_code`/`to_code` are their already-resolved
/// provider codes.
struct TranslationRequest {
    translate_provider: String,
    show_prompt: bool,
    from_lang: String,
    to_lang: String,
    from_code: String,
    to_code: String,
    text: String,
}

/// Callback type for [`spawn_translation`]'s `on_done` parameter — named (rather than
/// spelled out inline) because `clippy::type_complexity` flags it inline once it grew
/// a `&TranscriptEntry` argument for Stage 6.
type TranslationDoneCallback = Box<dyn FnOnce(&TranscriptEntry) + Send>;

/// Translates `request.text` in a background thread and pushes the result (or an error)
/// into the transcript. Shared by the Translate button/Enter key
/// (`on_translate_requested`) and the global hotkey (Stage 5) — the only two callers,
/// extracted here specifically to avoid duplicating the provider-call/transcript-push
/// logic between them.
///
/// `on_done`, if given, runs on the UI thread with the resolved [`TranscriptEntry`],
/// before it's pushed into the transcript. The hotkey path (Stage 5/6) uses this to
/// clear its "already processing" guard and to populate+show the Stage 6 popup; the
/// button path has no such guard and no popup, and passes `None`.
fn spawn_translation(
    weak: slint::Weak<AppWindow>,
    request: TranslationRequest,
    on_done: Option<TranslationDoneCallback>,
) {
    let TranslationRequest {
        translate_provider,
        show_prompt,
        from_lang,
        to_lang,
        from_code,
        to_code,
        text,
    } = request;

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
        let request_text = text.clone();
        let result = runtime.block_on(async move {
            let provider = providers::create_provider(&translate_provider)?;
            provider
                .translate_text(&request_text, &from_code, &to_code)
                .await
        });

        slint::invoke_from_event_loop(move || {
            let entry = match result {
                Ok(translated) => TranscriptEntry {
                    phrase: format_line(show_prompt, &from_lang, &text).into(),
                    translation: format_line(show_prompt, &to_lang, &translated).into(),
                },
                Err(err) => TranscriptEntry {
                    phrase: format_line(show_prompt, &from_lang, &text).into(),
                    translation: format!("Error: {err}").into(),
                },
            };
            if let Some(on_done) = on_done {
                on_done(&entry);
            }
            if let Some(window) = weak.upgrade() {
                push_transcript_entry(&window, entry);
            }
        })
        .ok();
    });
}

/// Reads `window`'s current position/size as a [`config::WindowGeometry`].
fn current_window_geometry(window: &AppWindow) -> config::WindowGeometry {
    let position = window.window().position();
    let size = window.window().size();
    config::WindowGeometry {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

/// Captures `window`'s current position/size into `config_manager` and persists it,
/// if [`config::GuiConfig::remember_window_geometry`] is enabled -- a no-op otherwise,
/// so a geometry saved from before the setting was turned off is left on disk rather
/// than overwritten with nothing. Called right before the window is hidden
/// (`on_close_requested`) or the app quits (`tray.on_quit_requested`) -- the two
/// points in `main()` that actually call this.
fn save_window_geometry(window: &AppWindow, config_manager: &Arc<Mutex<GuiConfigManager>>) {
    let mut manager = config_manager.lock().unwrap();
    if !manager.config().remember_window_geometry {
        return;
    }
    let geometry = current_window_geometry(window);
    let mut new_config = manager.config().clone();
    new_config.window_geometry = Some(geometry);
    let _ = manager.update(new_config);
}

/// Shows `window` and, only the *first* time this is called in a given run (tracked
/// via `geometry_restored`), restores its saved position/size from config -- if
/// `remember_window_geometry` is enabled and a geometry was actually saved by a
/// previous run -- or otherwise re-asserts [`DEFAULT_WINDOW_SIZE`] explicitly.
///
/// Applied *after* `.show()`, not before: Stage 6 found that `set_position` before
/// `.show()` is silently ignored on this project's X11 setup (no OS-level window
/// exists yet at that point), and the same is assumed to hold for `set_size`.
///
/// The explicit re-assert of `DEFAULT_WINDOW_SIZE` (even though `AppWindow` already
/// declares `preferred-width`/`preferred-height: 480px` in `app.slint`) is a real fix
/// for a real bug found while testing this function, not defensive-programming
/// speculation: on this project's X11/mutter setup, the *first* one or two window
/// creations after a fresh launch sometimes settle at a much smaller size (observed:
/// 458x188) instead of the requested 480x480 -- a winit/X11 initial-size-negotiation
/// race, reproduced consistently even with this function's own restore logic fully
/// bypassed (a plain `window.show()?` hit it too), and gone once the same process ran
/// a few more launches. Since explicitly calling `set_size()` *after* `.show()` is
/// already established as reliable here (the same pattern this popup-derived doc
/// comment already describes for position), re-asserting the intended size the same
/// way corrects the race instead of leaving it to chance.
///
/// Only the *first* show restores/re-asserts anything -- a later one (e.g.
/// re-opening from the tray after hiding, in the same run) leaves the window exactly
/// as the user last had it, since blindly re-applying a size every time would fight
/// with a live resize/move that hasn't been captured back into `config_manager` yet
/// (only `save_window_geometry`, called on hide/quit, does that).
///
/// The immediate `set_size`/`set_position` call is followed by a second, deferred
/// re-apply via `slint::Timer::single_shot`. This isn't defensive speculation: live
/// testing found that this function is only reliable when it runs *before*
/// `run_event_loop_until_quit()` (the `!start_minimized` startup path, which settles
/// within ~0.5s). When it instead runs *from inside* the already-running event loop --
/// which is the normal case once `start_minimized` is on, since then the first-ever
/// `show()` happens from the tray's "Show Tagent" click, delivered via a D-Bus/ksni
/// callback -- the synchronous `set_size`/`set_position` calls are silently dropped:
/// the window sticks at winit/X11's own race-default size (observed: 458x188) and
/// never settles, even given seconds of dwell time (confirmed live by driving the
/// tray icon's `org.kde.StatusNotifierItem.Activate` over D-Bus directly). Re-issuing
/// the same calls ~150ms later, after the window manager's own initial map/placement
/// negotiation has had a chance to finish, reliably corrects it.
fn show_window_restoring_geometry(
    window: &AppWindow,
    config_manager: &Arc<Mutex<GuiConfigManager>>,
    geometry_restored: &Rc<Cell<bool>>,
) {
    window.show().ok();

    if geometry_restored.replace(true) {
        return;
    }

    let config = config_manager.lock().unwrap().config().clone();
    let saved_geometry = config
        .remember_window_geometry
        .then_some(config.window_geometry)
        .flatten();

    let (target_size, target_position) = match saved_geometry {
        Some(geometry) => (
            slint::PhysicalSize::new(geometry.width, geometry.height),
            Some(slint::PhysicalPosition::new(geometry.x, geometry.y)),
        ),
        // No saved geometry to restore (or the setting is off) -- still
        // re-assert the default size, to correct the initial-sizing race
        // described above rather than leave the window at whatever it raced
        // into.
        None => (DEFAULT_WINDOW_SIZE, None),
    };

    apply_window_geometry(window, target_size, target_position);

    let weak_window = window.as_weak();
    slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
        if let Some(window) = weak_window.upgrade() {
            apply_window_geometry(&window, target_size, target_position);
        }
    });
}

fn apply_window_geometry(
    window: &AppWindow,
    size: slint::PhysicalSize,
    position: Option<slint::PhysicalPosition>,
) {
    window.window().set_size(size);
    if let Some(position) = position {
        window.window().set_position(position);
    }
}

fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let window = AppWindow::new()?;

    let config_manager = Arc::new(Mutex::new(GuiConfigManager::new()));

    // Timestamp of when the Settings dialog's "Record" capture (the
    // `HotkeyRecorder` in app.slint) last started, `None` while not recording.
    // Checked by the global hotkey callback below to suppress firing for real
    // while a recording is in progress -- otherwise, if the hotkey being
    // recorded happens to be (or be close to) the currently *active* hotkey,
    // the real trigger fires mid-capture, and its `ClipboardManager::get_text_with_copy`
    // step -- which simulates a real Ctrl+C keypress to grab the selection --
    // lands right back on the Settings dialog (since it still holds keyboard
    // focus), getting captured as "Ctrl+C" instead of whatever was actually
    // pressed. The dialog signals start/stop via `recording-changed`, but a
    // timestamp-plus-timeout (rather than a plain latch cleared only on the
    // expected stop signal) means a dialog closed uncleanly mid-recording
    // (e.g. the window's own X button, bypassing the Record button's own
    // "Cancel") can't wedge the global hotkey off forever -- it self-clears
    // after `RECORDING_SUPPRESSION_TIMEOUT`.
    let recording_started_at: Arc<Mutex<Option<std::time::Instant>>> = Arc::new(Mutex::new(None));

    // Whether the saved window geometry (if any) has been restored yet in this
    // run -- see `show_window_restoring_geometry`'s doc comment for why this is
    // only ever done once, not on every show.
    let geometry_restored = Rc::new(Cell::new(false));

    // Stage 7: redirect the OS-level close button (and Alt+F4/Cmd+Q-equivalent)
    // to hide the window instead of quitting the app -- the tray's "Quit" item
    // (wired below) becomes the only way to actually exit from here on. Also
    // captures the window's current position/size first, so closing it is one
    // of the two points (the other: Quit, below) "remember window geometry"
    // actually saves from.
    let weak_for_close = window.as_weak();
    let config_manager_for_close = config_manager.clone();
    window.window().on_close_requested(move || {
        if let Some(window) = weak_for_close.upgrade() {
            save_window_geometry(&window, &config_manager_for_close);
        }
        slint::CloseRequestResponse::HideWindow
    });

    apply_style(&window, config_manager.lock().unwrap().config());

    // Stage 6: one persistent popup instance, reused (repositioned/re-texted/
    // re-shown) on every hotkey trigger rather than constructed per trigger --
    // see the "one persistent PopupWindow instance" note in the Stage 6 plan.
    // `popup` itself must stay alive for the rest of `main()` (never dropped
    // early), same lifetime reasoning as `window`/`config_manager` below.
    let popup = TranslationPopup::new()?;
    apply_popup_style(&popup, config_manager.lock().unwrap().config());
    let popup_weak = popup.as_weak();

    // The popup's own `hide-timer` (a `Timer` *element* declared in app.slint, not
    // the Rust `slint::Timer` API) decides when to actually hide it -- see
    // `show_popup`'s doc comment for why that has to live in .slint rather than as a
    // `slint::Timer` held here. It only signals "time to hide" via this callback;
    // hiding it, and restoring focus to whatever was focused before the popup was
    // shown (see `POPUP_RESTORE_TARGET`'s doc comment for why this happens here now,
    // not immediately after `show()`), happens here.
    let popup_for_hide = popup.as_weak();
    popup.on_hide_requested(move || {
        if let Some(popup) = popup_for_hide.upgrade() {
            popup.hide().ok();
        }
        if let Some(handle) = POPUP_RESTORE_TARGET.with(|cell| cell.take()) {
            let _ = platform::window::set_foreground_window(handle);
        }
    });

    // Stage 7: persistent tray icon -- same "must stay alive for the rest of
    // main()" reasoning as `popup` above. Its own `.show()`/`.hide()` are
    // deliberately never called (see TrayIcon's doc comment in app.slint):
    // the icon exists for as long as this binding does, and disappears when
    // `main()` returns.
    let tray = TrayIcon::new()?;

    let weak_for_tray_show = window.as_weak();
    let config_manager_for_tray_show = config_manager.clone();
    let geometry_restored_for_tray_show = geometry_restored.clone();
    tray.on_show_requested(move || {
        if let Some(window) = weak_for_tray_show.upgrade() {
            show_window_restoring_geometry(
                &window,
                &config_manager_for_tray_show,
                &geometry_restored_for_tray_show,
            );
        }
    });

    // Reuses the *existing* `on_settings_requested` handler registered on
    // `window` below by invoking that callback programmatically, rather than
    // duplicating the ~300-line Settings-opening body here. `invoke_<name>` is
    // Slint's standard generated way to fire a callback from Rust regardless of
    // whether it has a Rust-registered handler (unlike `invoke_apply_theme`/
    // `invoke_start_hide_timer` elsewhere in this file, which invoke
    // `public function`s with a body defined in .slint -- a different
    // mechanism); confirmed by this file compiling, since a missing/wrong
    // generated method is a build error, not a runtime one. What was *not*
    // checked in this session is that it does the right thing at runtime (this
    // stage's own "no live launch" policy) -- the manual verification algorithm
    // covers that.
    let weak_for_tray_settings = window.as_weak();
    tray.on_settings_requested(move || {
        if let Some(window) = weak_for_tray_settings.upgrade() {
            window.invoke_settings_requested();
        }
    });

    let weak_for_quit = window.as_weak();
    let config_manager_for_quit = config_manager.clone();
    tray.on_quit_requested(move || {
        // Only save if the window is actually visible right now -- an
        // already-hidden (or never-shown, if start_minimized and the window
        // was never opened this run) window's position()/size() would just
        // return stale/default values, which would otherwise silently
        // overwrite a perfectly good previously-saved geometry with nothing
        // meaningful.
        if let Some(window) = weak_for_quit.upgrade() {
            if window.window().is_visible() {
                save_window_geometry(&window, &config_manager_for_quit);
            }
        }
        slint::quit_event_loop().ok();
    });

    let config_manager_for_settings = config_manager.clone();
    let config_manager_for_translate = config_manager.clone();
    let weak = window.as_weak();
    window.on_translate_requested(move |text, from_lang, to_lang| {
        let config_manager = &config_manager_for_translate;
        let text = text.trim().to_string();
        if text.is_empty() {
            return;
        }

        let from_code = languages::name_to_code(&from_lang).to_string();
        let to_code = languages::name_to_code(&to_lang).to_string();

        let (translate_provider, show_prompt) = {
            let mut manager = config_manager.lock().unwrap();
            manager.check_and_reload();
            let cfg = manager.config();
            (cfg.translate_provider.clone(), cfg.show_prompt)
        };

        if to_code == "auto" {
            if let Some(window) = weak.upgrade() {
                push_transcript_entry(
                    &window,
                    TranscriptEntry {
                        phrase: format_line(show_prompt, &from_lang, &text).into(),
                        translation: "Error: \"Auto\" is not a valid target language".into(),
                    },
                );
            }
            return;
        }

        spawn_translation(
            weak.clone(),
            TranslationRequest {
                translate_provider,
                show_prompt,
                from_lang: from_lang.to_string(),
                to_lang: to_lang.to_string(),
                from_code,
                to_code,
                text,
            },
            None,
        );
    });

    let weak = window.as_weak();
    window.on_swap_requested(move || {
        if let Some(window) = weak.upgrade() {
            let source = window.get_source_language_index();
            let target = window.get_target_language_index();
            window.set_source_language_index(target);
            // "Auto" (index 0) is only valid as a source language; fall back to
            // "English" (index 1) rather than making it the new target.
            window.set_target_language_index(if source == 0 { 1 } else { source });
        }
    });

    let weak = window.as_weak();
    window.on_copy_requested(move || {
        let weak = weak.clone();
        std::thread::spawn(move || {
            let result = ClipboardManager::new().get_text_with_copy();

            slint::invoke_from_event_loop(move || {
                if let Some(window) = weak.upgrade() {
                    match result {
                        Ok(text) => window.set_input_text(text.into()),
                        Err(err) => push_transcript_entry(
                            &window,
                            TranscriptEntry {
                                phrase: "[Clipboard]".into(),
                                translation: format!("Error: {err}").into(),
                            },
                        ),
                    }
                }
            })
            .ok();
        });
    });

    let window_weak_for_settings = window.as_weak();
    let recording_started_at_for_settings = recording_started_at.clone();
    window.on_settings_requested(move || {
        let dialog = SettingsDialog::new().unwrap();
        dialog.set_app_version(env!("CARGO_PKG_VERSION").into());

        let recording_started_at_for_recording = recording_started_at_for_settings.clone();
        dialog.on_recording_changed(move |active| {
            *recording_started_at_for_recording.lock().unwrap() =
                active.then(std::time::Instant::now);
        });

        let current_config = config_manager_for_settings.lock().unwrap().config().clone();

        let providers = dialog.get_providers();
        let provider_index = providers
            .iter()
            .position(|p| p.as_str() == current_config.translate_provider)
            .unwrap_or(0);
        dialog.set_provider_index(provider_index as i32);

        let themes = dialog.get_themes();
        let theme_index = themes
            .iter()
            .position(|t| t.as_str().to_lowercase() == current_config.theme)
            .unwrap_or(0);
        dialog.set_theme_index(theme_index as i32);
        // The dialog gets its own Palette instance (globals aren't shared
        // between windows) — apply the currently-active theme to it too, or
        // it would render in the system default regardless of what's saved.
        dialog.invoke_apply_theme(current_config.theme.clone().into());

        dialog.set_block_spacing_px(current_config.block_spacing_px);
        dialog.set_phrases_spacing_px(current_config.phrases_spacing_px);

        // Show the matching preset's name in the "Color scheme" dropdown
        // (instead of the "Presets…" placeholder at index 0) when the five
        // colors currently in effect are exactly one of the presets — e.g.
        // right after it was applied and saved. Index +1 accounts for that
        // placeholder being first in color-scheme-options.
        let matching_scheme_index = COLOR_SCHEMES.iter().position(|scheme| {
            scheme.background == current_config.background_color
                && scheme.phrase_color == current_config.phrase_color
                && scheme.phrase_background == current_config.phrase_background
                && scheme.translation_color == current_config.translation_color
                && scheme.translation_background == current_config.translation_background
        });
        dialog.set_color_scheme_index(matching_scheme_index.map_or(0, |i| i as i32 + 1));

        dialog.set_phrase_font_index(font_index_for(&current_config.phrase_font));
        dialog.set_phrase_size(current_config.phrase_size);
        dialog.set_translation_font_index(font_index_for(&current_config.translation_font));
        dialog.set_translation_size(current_config.translation_size);

        dialog.set_show_prompt(current_config.show_prompt);
        dialog.set_start_minimized(current_config.start_minimized);
        dialog.set_remember_window_geometry(current_config.remember_window_geometry);

        dialog.set_translate_hotkey(current_config.translate_hotkey.clone().into());
        dialog.set_translate_hotkey_error(
            hotkey_validation_error(&current_config.translate_hotkey).into(),
        );
        dialog.set_popup_auto_hide_seconds(
            current_config.popup_auto_hide_seconds.min(60) as i32,
        );

        let dialog_weak = dialog.as_weak();
        dialog.on_hotkey_edited(move |text| {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.set_translate_hotkey_error(hotkey_validation_error(text.as_str()).into());
            }
        });

        dialog.on_map_key_to_token(|text| slint_key_text_to_hotkey_token(text.as_str()).into());

        let dialog_weak = dialog.as_weak();
        dialog.on_hotkey_unrecognized(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.set_translate_hotkey_error(
                    "Couldn't recognize that key — if you're on a non-Latin keyboard \
                     layout, switch to a Latin layout while recording, or type the \
                     hotkey manually above."
                        .into(),
                );
            }
        });

        init_color_field!(
            dialog,
            current_config.background_color.as_str(),
            set_background_use_default,
            set_background_red,
            set_background_green,
            set_background_blue,
            set_background_hex
        );
        init_color_field!(
            dialog,
            current_config.phrase_color.as_str(),
            set_phrase_color_use_default,
            set_phrase_color_red,
            set_phrase_color_green,
            set_phrase_color_blue,
            set_phrase_color_hex
        );
        init_color_field!(
            dialog,
            current_config.phrase_background.as_str(),
            set_phrase_bg_use_default,
            set_phrase_bg_red,
            set_phrase_bg_green,
            set_phrase_bg_blue,
            set_phrase_bg_hex
        );
        init_color_field!(
            dialog,
            current_config.translation_color.as_str(),
            set_translation_color_use_default,
            set_translation_color_red,
            set_translation_color_green,
            set_translation_color_blue,
            set_translation_color_hex
        );
        init_color_field!(
            dialog,
            current_config.translation_background.as_str(),
            set_translation_bg_use_default,
            set_translation_bg_red,
            set_translation_bg_green,
            set_translation_bg_blue,
            set_translation_bg_hex
        );
        wire_hex_committed!(
            dialog,
            on_background_hex_committed,
            set_background_red,
            set_background_green,
            set_background_blue,
            set_background_use_default
        );
        wire_hex_committed!(
            dialog,
            on_phrase_color_hex_committed,
            set_phrase_color_red,
            set_phrase_color_green,
            set_phrase_color_blue,
            set_phrase_color_use_default
        );
        wire_hex_committed!(
            dialog,
            on_phrase_bg_hex_committed,
            set_phrase_bg_red,
            set_phrase_bg_green,
            set_phrase_bg_blue,
            set_phrase_bg_use_default
        );
        wire_hex_committed!(
            dialog,
            on_translation_color_hex_committed,
            set_translation_color_red,
            set_translation_color_green,
            set_translation_color_blue,
            set_translation_color_use_default
        );
        wire_hex_committed!(
            dialog,
            on_translation_bg_hex_committed,
            set_translation_bg_red,
            set_translation_bg_green,
            set_translation_bg_blue,
            set_translation_bg_use_default
        );
        wire_rgb_changed!(
            dialog,
            on_background_rgb_changed,
            get_background_red,
            get_background_green,
            get_background_blue,
            set_background_hex
        );
        wire_rgb_changed!(
            dialog,
            on_phrase_color_rgb_changed,
            get_phrase_color_red,
            get_phrase_color_green,
            get_phrase_color_blue,
            set_phrase_color_hex
        );
        wire_rgb_changed!(
            dialog,
            on_phrase_bg_rgb_changed,
            get_phrase_bg_red,
            get_phrase_bg_green,
            get_phrase_bg_blue,
            set_phrase_bg_hex
        );
        wire_rgb_changed!(
            dialog,
            on_translation_color_rgb_changed,
            get_translation_color_red,
            get_translation_color_green,
            get_translation_color_blue,
            set_translation_color_hex
        );
        wire_rgb_changed!(
            dialog,
            on_translation_bg_rgb_changed,
            get_translation_bg_red,
            get_translation_bg_green,
            get_translation_bg_blue,
            set_translation_bg_hex
        );

        let dialog_weak = dialog.as_weak();
        dialog.on_color_scheme_selected(move |name| {
            let Some(dialog) = dialog_weak.upgrade() else {
                return;
            };
            let Some(scheme) = COLOR_SCHEMES.iter().find(|s| s.name == name.as_str()) else {
                return;
            };

            init_color_field!(
                dialog,
                scheme.background,
                set_background_use_default,
                set_background_red,
                set_background_green,
                set_background_blue,
                set_background_hex
            );
            init_color_field!(
                dialog,
                scheme.phrase_color,
                set_phrase_color_use_default,
                set_phrase_color_red,
                set_phrase_color_green,
                set_phrase_color_blue,
                set_phrase_color_hex
            );
            init_color_field!(
                dialog,
                scheme.phrase_background,
                set_phrase_bg_use_default,
                set_phrase_bg_red,
                set_phrase_bg_green,
                set_phrase_bg_blue,
                set_phrase_bg_hex
            );
            init_color_field!(
                dialog,
                scheme.translation_color,
                set_translation_color_use_default,
                set_translation_color_red,
                set_translation_color_green,
                set_translation_color_blue,
                set_translation_color_hex
            );
            init_color_field!(
                dialog,
                scheme.translation_background,
                set_translation_bg_use_default,
                set_translation_bg_red,
                set_translation_bg_green,
                set_translation_bg_blue,
                set_translation_bg_hex
            );

            let target_theme = if scheme.dark { "dark" } else { "light" };
            let themes = dialog.get_themes();
            if let Some(index) = themes
                .iter()
                .position(|t| t.as_str().to_lowercase() == target_theme)
            {
                dialog.set_theme_index(index as i32);
            }
            dialog.invoke_apply_theme(target_theme.into());
        });

        let dialog_weak = dialog.as_weak();
        let config_manager_for_save = config_manager_for_settings.clone();
        let window_weak_for_save = window_weak_for_settings.clone();
        dialog.on_save_requested(move |provider, theme| {
            let Some(dialog) = dialog_weak.upgrade() else {
                return;
            };

            let new_config = config::GuiConfig {
                translate_provider: provider.to_string(),
                theme: theme.to_lowercase(),
                background_color: color_field_hex(
                    dialog.get_background_use_default(),
                    dialog.get_background_red(),
                    dialog.get_background_green(),
                    dialog.get_background_blue(),
                ),
                phrase_font: FONT_FAMILIES[dialog
                    .get_phrase_font_index()
                    .clamp(0, FONT_FAMILIES.len() as i32 - 1)
                    as usize]
                    .to_string(),
                phrase_size: dialog.get_phrase_size(),
                phrase_color: color_field_hex(
                    dialog.get_phrase_color_use_default(),
                    dialog.get_phrase_color_red(),
                    dialog.get_phrase_color_green(),
                    dialog.get_phrase_color_blue(),
                ),
                phrase_background: color_field_hex(
                    dialog.get_phrase_bg_use_default(),
                    dialog.get_phrase_bg_red(),
                    dialog.get_phrase_bg_green(),
                    dialog.get_phrase_bg_blue(),
                ),
                translation_font: FONT_FAMILIES[dialog
                    .get_translation_font_index()
                    .clamp(0, FONT_FAMILIES.len() as i32 - 1)
                    as usize]
                    .to_string(),
                translation_size: dialog.get_translation_size(),
                translation_color: color_field_hex(
                    dialog.get_translation_color_use_default(),
                    dialog.get_translation_color_red(),
                    dialog.get_translation_color_green(),
                    dialog.get_translation_color_blue(),
                ),
                translation_background: color_field_hex(
                    dialog.get_translation_bg_use_default(),
                    dialog.get_translation_bg_red(),
                    dialog.get_translation_bg_green(),
                    dialog.get_translation_bg_blue(),
                ),
                block_spacing_px: dialog.get_block_spacing_px(),
                phrases_spacing_px: dialog.get_phrases_spacing_px(),
                show_prompt: dialog.get_show_prompt(),
                translate_hotkey: dialog.get_translate_hotkey().to_string(),
                popup_auto_hide_seconds: dialog.get_popup_auto_hide_seconds() as u64,
                start_minimized: dialog.get_start_minimized(),
                remember_window_geometry: dialog.get_remember_window_geometry(),
                // Not dialog-editable -- captured automatically from the real window
                // (see save_window_geometry) -- so carried through unchanged, same
                // treatment as translate_hotkey/popup_auto_hide_seconds above.
                window_geometry: current_config.window_geometry,
            };

            if let Err(err) = config_manager_for_save
                .lock()
                .unwrap()
                .update(new_config.clone())
            {
                eprintln!("Warning: failed to save tagent-gui.json: {err}");
            }
            if let Some(window) = window_weak_for_save.upgrade() {
                apply_style(&window, &new_config);
            }
            dialog.hide().ok();
        });

        let dialog_weak = dialog.as_weak();
        dialog.on_cancel_requested(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.hide().ok();
            }
        });

        dialog.show().unwrap();
    });

    // Global hotkey (Stage 5): parse+validate once at startup from the hand-editable
    // `translate_hotkey` config field. On failure, log a warning and leave the hotkey
    // disabled rather than failing to start — same "log and keep running" convention
    // `tagent-cli` uses for its own hotkeys. Changes to `translate_hotkey` take effect
    // only on restart (no live-reload of the OS-level grab itself).
    let hotkey_str = config_manager
        .lock()
        .unwrap()
        .config()
        .translate_hotkey
        .clone();
    match config::HotkeyParser::parse(&hotkey_str)
        .and_then(|h| config::HotkeyParser::validate_hotkey(&h).map(|_| h))
    {
        Ok(hotkey) => {
            let is_processing = Arc::new(AtomicBool::new(false));
            let weak = window.as_weak();
            let config_manager = config_manager.clone();
            let popup_weak_for_hotkey = popup_weak.clone();
            let recording_started_at = recording_started_at.clone();
            KeyboardHook::spawn(hotkey, move || {
                // Runs on the platform hook's own thread (on Windows, inside the
                // WH_KEYBOARD_LL callback itself) -- must stay fast and non-blocking,
                // hence the atomic guard and invoke_from_event_loop hand-off below
                // rather than doing any real work here.

                // Suppressed while the Settings dialog is actively recording a new
                // hotkey -- see `recording_started_at`'s doc comment in `main()` for
                // why (this is what stops a recorded "Ctrl+C" from ever showing up:
                // that was this same trigger firing for real mid-capture, not a bug
                // in the capture logic itself).
                if let Some(started) = *recording_started_at.lock().unwrap() {
                    if started.elapsed() < RECORDING_SUPPRESSION_TIMEOUT {
                        return;
                    }
                }

                if is_processing.swap(true, Ordering::SeqCst) {
                    return; // already handling a previous trigger
                }

                let is_processing = is_processing.clone();
                let weak = weak.clone();
                let config_manager = config_manager.clone();
                let popup_weak = popup_weak_for_hotkey.clone();
                slint::invoke_from_event_loop(move || {
                    let Some(window) = weak.upgrade() else {
                        is_processing.store(false, Ordering::SeqCst);
                        return;
                    };

                    let languages_model = window.get_languages();
                    let from_lang = languages_model
                        .row_data(window.get_source_language_index() as usize)
                        .unwrap_or_default();
                    let to_lang = languages_model
                        .row_data(window.get_target_language_index() as usize)
                        .unwrap_or_default();

                    let (translate_provider, show_prompt, popup_auto_hide_seconds) = {
                        let mut manager = config_manager.lock().unwrap();
                        manager.check_and_reload();
                        let cfg = manager.config();
                        (
                            cfg.translate_provider.clone(),
                            cfg.show_prompt,
                            cfg.popup_auto_hide_seconds_or_default(),
                        )
                    };

                    let from_code = languages::name_to_code(&from_lang).to_string();
                    let to_code = languages::name_to_code(&to_lang).to_string();

                    if to_code == "auto" {
                        push_transcript_entry(
                            &window,
                            TranscriptEntry {
                                phrase: "[Hotkey]".into(),
                                translation: "Error: \"Auto\" is not a valid target language"
                                    .into(),
                            },
                        );
                        is_processing.store(false, Ordering::SeqCst);
                        return;
                    }

                    let weak2 = weak.clone();
                    let is_processing2 = is_processing.clone();
                    let popup_weak2 = popup_weak.clone();
                    std::thread::spawn(move || {
                        match ClipboardManager::new().get_text_with_copy() {
                            Ok(text) if !text.trim().is_empty() => {
                                spawn_translation(
                                    weak2,
                                    TranslationRequest {
                                        translate_provider,
                                        show_prompt,
                                        from_lang: from_lang.to_string(),
                                        to_lang: to_lang.to_string(),
                                        from_code,
                                        to_code,
                                        text,
                                    },
                                    Some(Box::new(move |entry: &TranscriptEntry| {
                                        is_processing2.store(false, Ordering::SeqCst);
                                        show_popup(&popup_weak2, entry, popup_auto_hide_seconds);
                                    })),
                                );
                            }
                            Ok(_) => {
                                is_processing2.store(false, Ordering::SeqCst);
                            }
                            Err(err) => {
                                slint::invoke_from_event_loop(move || {
                                    if let Some(window) = weak2.upgrade() {
                                        push_transcript_entry(
                                            &window,
                                            TranscriptEntry {
                                                phrase: "[Hotkey]".into(),
                                                translation: format!("Error: {err}").into(),
                                            },
                                        );
                                    }
                                    is_processing2.store(false, Ordering::SeqCst);
                                })
                                .ok();
                            }
                        }
                    });
                })
                .ok();
            });
        }
        Err(e) => {
            eprintln!(
                "Warning: failed to parse/validate translate_hotkey '{}': {}",
                hotkey_str, e
            );
            eprintln!("Global hotkey disabled.");
        }
    }

    // Stage 7: `run_event_loop_until_quit()` replaces the old `window.run()`
    // (which showed the window, ran the loop, and quit as soon as the window
    // was hidden) -- the loop must now keep running even with `window` hidden,
    // whether that's from `start_minimized` at launch or from the user closing
    // it later, since `tray` (and the hotkey, if enabled) are still live. Only
    // `tray`'s "Quit" item (`slint::quit_event_loop()`, wired above) ends it.
    let start_minimized = config_manager.lock().unwrap().config().start_minimized;
    if !start_minimized {
        show_window_restoring_geometry(&window, &config_manager, &geometry_restored);
    }
    slint::run_event_loop_until_quit()?;
    Ok(())
}
