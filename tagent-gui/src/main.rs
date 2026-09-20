// Windows: no terminal window when the exe is started from Explorer or a shortcut.
// `platform::windows::console::attach_parent()` (first thing in `main`) brings the
// `eprintln!` output back when it is started from a terminal instead.
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use slint::{Color, ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tagent::{languages, providers};

mod config;
mod dictionary;
mod platform;
mod popup_position;
mod speech;

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
    /// One of `"auto"`, `"light"`, `"dark"` — applied to the Theme dropdown
    /// alongside this scheme's colors. Only `"Default"` uses `"auto"`.
    theme: &'static str,
    background: &'static str,
    phrase_color: &'static str,
    phrase_background: &'static str,
    translation_color: &'static str,
    translation_background: &'static str,
}

const COLOR_SCHEMES: &[ColorScheme] = &[
    // Resets the transcript's (or popup's) colors back to theme-following
    // and Theme to Auto — same shape as every other preset here, just with
    // no fixed colors of its own.
    ColorScheme {
        name: "Default",
        theme: "auto",
        background: "",
        phrase_color: "",
        phrase_background: "",
        translation_color: "",
        translation_background: "",
    },
    ColorScheme {
        name: "Solarized Dark",
        theme: "dark",
        background: "#002B36",
        phrase_color: "#839496",
        phrase_background: "#073642",
        translation_color: "#268BD2",
        translation_background: "#073642",
    },
    ColorScheme {
        name: "Solarized Light",
        theme: "light",
        background: "#FDF6E3",
        phrase_color: "#657B83",
        phrase_background: "#EEE8D5",
        translation_color: "#268BD2",
        translation_background: "#EEE8D5",
    },
    ColorScheme {
        name: "Dracula",
        theme: "dark",
        background: "#282A36",
        phrase_color: "#F8F8F2",
        phrase_background: "#44475A",
        translation_color: "#BD93F9",
        translation_background: "#44475A",
    },
    ColorScheme {
        name: "Nord",
        theme: "dark",
        background: "#2E3440",
        phrase_color: "#D8DEE9",
        phrase_background: "#3B4252",
        translation_color: "#88C0D0",
        translation_background: "#3B4252",
    },
    ColorScheme {
        name: "Gruvbox Dark",
        theme: "dark",
        background: "#282828",
        phrase_color: "#EBDBB2",
        phrase_background: "#3C3836",
        translation_color: "#FE8019",
        translation_background: "#3C3836",
    },
    ColorScheme {
        name: "Monokai",
        theme: "dark",
        background: "#272822",
        phrase_color: "#F8F8F2",
        phrase_background: "#3E3D32",
        translation_color: "#A6E22E",
        translation_background: "#3E3D32",
    },
    ColorScheme {
        name: "One Dark",
        theme: "dark",
        background: "#282C34",
        phrase_color: "#ABB2BF",
        phrase_background: "#2C313C",
        translation_color: "#61AFEF",
        translation_background: "#2C313C",
    },
    ColorScheme {
        name: "Tokyo Night",
        theme: "dark",
        background: "#1A1B26",
        phrase_color: "#C0CAF5",
        phrase_background: "#292E42",
        translation_color: "#7AA2F7",
        translation_background: "#292E42",
    },
    ColorScheme {
        name: "Catppuccin Mocha",
        theme: "dark",
        background: "#1E1E2E",
        phrase_color: "#CDD6F4",
        phrase_background: "#313244",
        translation_color: "#CBA6F7",
        translation_background: "#313244",
    },
    ColorScheme {
        name: "Night Owl",
        theme: "dark",
        background: "#011627",
        phrase_color: "#D6DEEB",
        phrase_background: "#1D3B53",
        translation_color: "#82AAFF",
        translation_background: "#1D3B53",
    },
    ColorScheme {
        name: "Ayu Dark",
        theme: "dark",
        background: "#0A0E14",
        phrase_color: "#B3B1AD",
        phrase_background: "#131721",
        translation_color: "#FFB454",
        translation_background: "#131721",
    },
    ColorScheme {
        name: "GitHub Light",
        theme: "light",
        background: "#FFFFFF",
        phrase_color: "#24292E",
        phrase_background: "#F6F8FA",
        translation_color: "#0366D6",
        translation_background: "#F6F8FA",
    },
    ColorScheme {
        name: "Gruvbox Light",
        theme: "light",
        background: "#FBF1C7",
        phrase_color: "#3C3836",
        phrase_background: "#EBDBB2",
        translation_color: "#D65D0E",
        translation_background: "#EBDBB2",
    },
    ColorScheme {
        name: "Catppuccin Latte",
        theme: "light",
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

/// Sets `dialog`'s Theme dropdown (and applies it to the dialog's own Palette
/// instance) to match a [`ColorScheme::theme`] value -- shared by both the
/// View tab's and the Popup tab's color-scheme-selected handlers, since
/// picking either one's preset also switches the single app-wide Theme.
fn apply_scheme_theme(dialog: &SettingsDialog, theme: &str) {
    let themes = dialog.get_themes();
    if let Some(index) = themes
        .iter()
        .position(|t| t.as_str().to_lowercase() == theme)
    {
        dialog.set_theme_index(index as i32);
    }
    dialog.invoke_apply_theme(theme.into());
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
    match config::HotkeyParser::parse(text).and_then(|h| config::HotkeyParser::validate_hotkey(&h))
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
///
/// Unlike `panel-foreground` (a live `Palette`-bound property that repaints on
/// its own once the system theme resolves), the colors this function writes
/// are plain snapshots taken at call time — so on Linux, where winit doesn't
/// deliver system theme detection synchronously (see the `Auto` theme note in
/// `docs/ARCHITECTURE.md`), calling this once at startup can permanently bake
/// in the wrong (light-default) background/colors under an `Auto` theme that
/// resolves to dark, while the live-bound foreground text correctly repaints
/// dark-on-dark — leaving the header unreadable rather than just flashing for
/// a frame.
///
/// Confirmed live (not just theoretical) that a short fixed delay after
/// window *creation* is not enough to work around this: with `start_minimized`
/// on, the window can sit unmapped for many seconds before the user ever opens
/// it from the tray, and the wrong snapshot from the startup call was still
/// showing at that point — so detection here appears tied to the window
/// actually being mapped on screen, not just wall-clock time since creation.
/// [`show_window_restoring_geometry`] re-calls this (immediately, then again
/// after a short settle delay) on the window's first real `show()` rather than
/// a fixed time after creation, for the same reason it re-asserts size/position
/// there instead of at creation time.
///
/// The same staleness bites again after startup, too: if the user changes the
/// OS-level dark/light preference while `tagent-gui` is already running under
/// `Auto`, `panel-foreground` and other live-bound elements repaint on their
/// own, but the snapshots this function writes don't, until something calls
/// this again. `main`'s `theme_poll_timer` covers that by re-calling this
/// periodically for as long as `config.theme == "auto"`.
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

/// Applies the theme and display style to the Stage 6 popup — its own
/// independent style (Stage 9), not the transcript's phrase/translation
/// style: one shared Font/Size/Text color/Background for both lines, since
/// the popup only ever shows one phrase/translation pair at a time. Kept as
/// its own small function rather than widening `apply_style` to branch on
/// window type, since the two windows' style surfaces only partially overlap.
///
/// "Theme default" (an empty `popup_color`/`popup_background`) resolves
/// against the transcript's own `translation_color`/`translation_background`
/// first (Stage 9 follow-up), not straight against the raw Palette theme
/// colors — so an unmodified popup automatically matches whatever Color
/// scheme is active for the transcript (View tab), with no separate picker
/// needed. Translation (not phrase) is the reference field deliberately, per
/// explicit request. Those transcript fields are themselves resolved against
/// Palette when *they're* empty (i.e. the transcript is also just following
/// the theme), which is when the popup falls all the way back to the raw
/// theme colors.
fn apply_popup_style(popup: &TranslationPopup, config: &config::GuiConfig) {
    popup.invoke_apply_theme(config.theme.clone().into());

    let theme_default_fg = popup.get_panel_foreground().color();
    let theme_default_bg = popup.get_panel_background_theme_default().color();
    let scheme_default_fg = resolve_color(&config.translation_color, theme_default_fg);
    let scheme_default_bg = resolve_color(&config.translation_background, theme_default_bg);

    popup.set_popup_font(config.popup_font.clone().into());
    popup.set_popup_size(config.popup_size);
    popup.set_popup_color(resolve_color(&config.popup_color, scheme_default_fg));
    popup.set_popup_background(resolve_color(&config.popup_background, scheme_default_bg));
    popup.set_popup_max_width(config.popup_max_width);
    popup.set_popup_max_height(config.popup_max_height);
    popup.set_popup_border_width(config.popup_border_width);
}

/// Shows the Stage 6 popup with `outcome`'s text -- formatted here using the popup's
/// own independent `show_prompt`/`show_phrase` settings (Stage 9), not the
/// transcript's -- positioned next to the current mouse cursor, and (re)starts its
/// auto-hide timer. Called from the hotkey path's `spawn_translation` completion
/// callback, already on the UI thread.
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
/// function's caller, indirectly) is a `Box<dyn FnOnce(&TranscriptEntry, &TranslationOutcome) + Send>`
/// that `spawn_translation` moves through a background thread before calling it back on
/// the UI thread, and `slint::Timer` is `!Send` — it can't be captured into that closure
/// at all, regardless of how carefully it'd actually be used only on the UI thread.
/// `slint::Weak<TranslationPopup>` (used here) has no such restriction.
fn show_popup(
    popup_weak: &slint::Weak<TranslationPopup>,
    outcome: &TranslationOutcome,
    show_prompt: bool,
    show_phrase: bool,
    auto_hide_seconds: u64,
    remembered_position: Option<config::PopupPosition>,
) {
    let Some(popup) = popup_weak.upgrade() else {
        return;
    };

    POPUP_RESTORE_TARGET.with(|cell| cell.set(platform::window::foreground_window()));

    popup.set_phrase_text(format_line(show_prompt, &outcome.from_lang, &outcome.phrase_raw).into());
    popup.set_translation_text(if outcome.is_error {
        outcome.translation_raw.clone().into()
    } else {
        format_line(show_prompt, &outcome.to_lang, &outcome.translation_raw).into()
    });
    popup.set_show_phrase(show_phrase);

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
    //
    // A remembered position (`remember_popup_position`, from the last drag) wins
    // over the cursor, but is clamped back onto the current desktop first: the
    // monitor layout or resolution may have changed since it was saved, and this
    // window has no frame or close button to recover a popup stranded off-screen
    // (only the auto-hide would eventually dismiss it).
    let target = match remembered_position {
        Some(saved) => {
            let size = popup.window().size();
            Some(match platform::window::virtual_screen_bounds() {
                Some(bounds) => popup_position::clamp_to_bounds(
                    (saved.x, saved.y),
                    (size.width as i32, size.height as i32),
                    bounds,
                ),
                None => (saved.x, saved.y),
            })
        }
        None => platform::window::cursor_position().map(|(x, y)| (x + 16, y + 16)),
    };
    if let Some((x, y)) = target {
        popup
            .window()
            .set_position(slint::PhysicalPosition::new(x, y));
    }

    popup.set_auto_hide_seconds(auto_hide_seconds.min(i32::MAX as u64) as i32);
    popup.invoke_start_hide_timer();
}

/// Where a popup drag started, and how far it has got. Lives from the button going
/// down to it coming back up; see [`wire_popup_drag`].
#[derive(Clone, Copy)]
struct PopupDrag {
    /// Global cursor position when the button went down.
    start_cursor: (i32, i32),
    /// The popup's top-left corner when the button went down.
    start_position: (i32, i32),
    /// The last position the popup was moved to.
    last_position: (i32, i32),
    /// Whether the pointer has travelled far enough to count as a drag rather
    /// than a click.
    moved: bool,
}

/// Lets the user move the hotkey popup by dragging it, and -- if
/// `remember_popup_position` is on -- saves where it was dropped so later popups
/// reappear there instead of next to the cursor.
///
/// The three callbacks come from the popup's `TouchArea` (`drag-started`/`drag-moved`/
/// `drag-ended` in app.slint) and carry no coordinates, since Slint only reports
/// pointer positions relative to the popup, which itself moves under the pointer
/// with every step. Instead each step reads the *global* cursor position
/// ([`platform::window::cursor_position`]) and places the popup at its
/// drag-start position plus the pointer's travel since, which is jitter-free.
///
/// Dragging works whether or not the setting is on -- the popup just moves for as
/// long as it stays open; only the save at drag end is conditional. That save
/// re-reads the setting from the live config, so toggling it in Settings applies
/// to the very next drag.
///
/// The popup keeps the OS focus it took when shown until it auto-hides, at which
/// point `on_hide_requested` hands focus back to the window that had it before (see
/// [`show_popup`]) -- a drag doesn't change that, so no focus handling is needed
/// here.
fn wire_popup_drag(popup: &TranslationPopup, config_manager: &Arc<Mutex<GuiConfigManager>>) {
    let drag: Rc<Cell<Option<PopupDrag>>> = Rc::new(Cell::new(None));

    let popup_weak = popup.as_weak();
    let drag_for_start = drag.clone();
    popup.on_drag_started(move || {
        let Some(popup) = popup_weak.upgrade() else {
            return;
        };
        let position = popup.window().position();
        drag_for_start.set(
            platform::window::cursor_position().map(|start_cursor| PopupDrag {
                start_cursor,
                start_position: (position.x, position.y),
                last_position: (position.x, position.y),
                moved: false,
            }),
        );
    });

    let popup_weak = popup.as_weak();
    let drag_for_move = drag.clone();
    popup.on_drag_moved(move || {
        let (Some(popup), Some(mut state)) = (popup_weak.upgrade(), drag_for_move.get()) else {
            return;
        };
        let Some(cursor) = platform::window::cursor_position() else {
            return;
        };
        if !state.moved && !popup_position::exceeds_drag_threshold(state.start_cursor, cursor) {
            return;
        }
        state.moved = true;
        state.last_position =
            popup_position::dragged_position(state.start_position, state.start_cursor, cursor);
        popup.window().set_position(slint::PhysicalPosition::new(
            state.last_position.0,
            state.last_position.1,
        ));
        drag_for_move.set(Some(state));
    });

    let config_manager = config_manager.clone();
    popup.on_drag_ended(move || {
        let Some(state) = drag.take() else {
            return;
        };
        if !state.moved {
            return;
        }
        let mut manager = config_manager.lock().unwrap();
        if !manager.config().remember_popup_position {
            return;
        }
        let mut new_config = manager.config().clone();
        new_config.popup_position = Some(config::PopupPosition {
            x: state.last_position.0,
            y: state.last_position.1,
        });
        if let Err(err) = manager.update(new_config) {
            eprintln!("Warning: failed to save tagent-gui.json: {err}");
        }
    });
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

/// Builds a [`TranscriptEntry`] for a message that isn't a real translation (a
/// clipboard error, or the "Auto"-as-target-language guard) -- no speech text
/// on either side (Stage 10), since there's nothing meaningful to speak;
/// `translation_is_error: true` also keeps the translation speaker button
/// hidden even if that changes.
fn info_transcript_entry(
    phrase: impl Into<slint::SharedString>,
    translation: impl Into<slint::SharedString>,
) -> TranscriptEntry {
    TranscriptEntry {
        phrase: phrase.into(),
        translation: translation.into(),
        phrase_speech: String::new().into(),
        translation_speech: String::new().into(),
        from_code: String::new().into(),
        to_code: String::new().into(),
        translation_is_error: true,
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

/// Index of `name` in a provider dropdown's `model` (case-insensitive, like the provider
/// factories), or `0` when it isn't listed -- e.g. a hand-edited `tagent-gui.json` naming
/// a backend the dropdown doesn't offer, which a Settings save then replaces with the
/// first entry rather than keeping a value the dropdown can't display.
fn combo_index(model: &ModelRc<SharedString>, name: &str) -> i32 {
    model
        .iter()
        .position(|p| p.as_str().eq_ignore_ascii_case(name))
        .unwrap_or(0) as i32
}

/// The entry `index` selects in a dropdown's `model`, or `fallback` if the index is out
/// of range (which the `ComboBox` itself never produces).
fn combo_selection(model: &ModelRc<SharedString>, index: i32, fallback: &str) -> String {
    usize::try_from(index)
        .ok()
        .and_then(|i| model.row_data(i))
        .map_or_else(|| fallback.to_string(), |name| name.to_string())
}

/// A provider dropdown's model built from `names` (one of `tagent`'s `*_PROVIDERS` lists),
/// with the index of the currently configured provider `current` selected.
fn provider_choices(names: &[&str], current: &str) -> (ModelRc<SharedString>, i32) {
    let model = ModelRc::new(VecModel::from(
        names
            .iter()
            .map(|name| SharedString::from(*name))
            .collect::<Vec<_>>(),
    ));
    let index = combo_index(&model, current);
    (model, index)
}

/// Fills every `SettingsDialog` field from `config` -- used both to seed the
/// dialog when Settings opens (from the current saved config) and by the
/// General tab's "Reset to Defaults" button (from `GuiConfig::default()`).
/// Only fills field *values*; callback wiring (`on_translate_hotkey_edited`,
/// `on_speech_hotkey_edited`, `on_save_requested`, etc.) happens once per dialog
/// instance in
/// `on_settings_requested` and isn't repeated here.
fn seed_dialog_fields(dialog: &SettingsDialog, config: &config::GuiConfig) {
    // The dropdown lists come from `tagent` itself (one per provider axis), so a backend
    // added there is offered here without touching `app.slint`.
    let (model, index) =
        provider_choices(providers::TRANSLATION_PROVIDERS, &config.translate_provider);
    dialog.set_providers(model);
    dialog.set_provider_index(index);
    let (model, index) =
        provider_choices(providers::DICTIONARY_PROVIDERS, &config.dictionary_provider);
    dialog.set_dictionary_providers(model);
    dialog.set_dictionary_provider_index(index);
    let (model, index) = provider_choices(providers::SPEECH_PROVIDERS, &config.speech_provider);
    dialog.set_speech_providers(model);
    dialog.set_speech_provider_index(index);
    dialog.set_show_dictionary(config.show_dictionary);
    dialog.set_spell_check(config.spell_check);
    dialog.set_enable_text_to_speech(config.enable_text_to_speech);

    let themes = dialog.get_themes();
    let theme_index = themes
        .iter()
        .position(|t| t.as_str().to_lowercase() == config.theme)
        .unwrap_or(0);
    dialog.set_theme_index(theme_index as i32);
    // The dialog gets its own Palette instance (globals aren't shared
    // between windows) — apply the currently-active theme to it too, or
    // it would render in the system default regardless of what's saved.
    dialog.invoke_apply_theme(config.theme.clone().into());

    dialog.set_block_spacing_px(config.block_spacing_px);
    dialog.set_phrases_spacing_px(config.phrases_spacing_px);

    // Show the matching preset's name in the "Color scheme" dropdown
    // (instead of the "Custom" placeholder at index 0) when the five
    // colors currently in effect are exactly one of the presets — e.g.
    // right after it was applied and saved, or "Default" when they're all
    // still at "" (theme-following). Index +1 accounts for the "Custom"
    // placeholder being first in color-scheme-options.
    let matching_scheme_index = COLOR_SCHEMES.iter().position(|scheme| {
        scheme.background == config.background_color
            && scheme.phrase_color == config.phrase_color
            && scheme.phrase_background == config.phrase_background
            && scheme.translation_color == config.translation_color
            && scheme.translation_background == config.translation_background
    });
    dialog.set_color_scheme_index(matching_scheme_index.map_or(0, |i| i as i32 + 1));

    dialog.set_phrase_font_index(font_index_for(&config.phrase_font));
    dialog.set_phrase_size(config.phrase_size);
    dialog.set_translation_font_index(font_index_for(&config.translation_font));
    dialog.set_translation_size(config.translation_size);
    dialog.set_popup_font_index(font_index_for(&config.popup_font));
    dialog.set_popup_size(config.popup_size);
    dialog.set_popup_show_prompt(config.popup_show_prompt);
    dialog.set_popup_show_phrase(config.popup_show_phrase);
    dialog.set_popup_max_width(config.popup_max_width);
    dialog.set_popup_max_height(config.popup_max_height);
    dialog.set_popup_border_width(config.popup_border_width);

    dialog.set_show_prompt(config.show_prompt);
    dialog.set_start_minimized(config.start_minimized);
    dialog.set_remember_window_geometry(config.remember_window_geometry);

    dialog.set_translate_hotkey(config.translate_hotkey.clone().into());
    dialog.set_translate_hotkey_error(hotkey_validation_error(&config.translate_hotkey).into());
    dialog.set_speech_hotkey(config.speech_hotkey.clone().into());
    dialog.set_speech_hotkey_error(hotkey_validation_error(&config.speech_hotkey).into());
    dialog.set_enable_speech_hotkey(config.enable_speech_hotkey);
    dialog.set_popup_auto_hide_seconds(config.popup_auto_hide_seconds.min(60) as i32);
    dialog.set_remember_popup_position(config.remember_popup_position);

    init_color_field!(
        dialog,
        config.background_color.as_str(),
        set_background_use_default,
        set_background_red,
        set_background_green,
        set_background_blue,
        set_background_hex
    );
    init_color_field!(
        dialog,
        config.phrase_color.as_str(),
        set_phrase_color_use_default,
        set_phrase_color_red,
        set_phrase_color_green,
        set_phrase_color_blue,
        set_phrase_color_hex
    );
    init_color_field!(
        dialog,
        config.phrase_background.as_str(),
        set_phrase_bg_use_default,
        set_phrase_bg_red,
        set_phrase_bg_green,
        set_phrase_bg_blue,
        set_phrase_bg_hex
    );
    init_color_field!(
        dialog,
        config.translation_color.as_str(),
        set_translation_color_use_default,
        set_translation_color_red,
        set_translation_color_green,
        set_translation_color_blue,
        set_translation_color_hex
    );
    init_color_field!(
        dialog,
        config.translation_background.as_str(),
        set_translation_bg_use_default,
        set_translation_bg_red,
        set_translation_bg_green,
        set_translation_bg_blue,
        set_translation_bg_hex
    );
    init_color_field!(
        dialog,
        config.popup_color.as_str(),
        set_popup_color_use_default,
        set_popup_color_red,
        set_popup_color_green,
        set_popup_color_blue,
        set_popup_color_hex
    );
    init_color_field!(
        dialog,
        config.popup_background.as_str(),
        set_popup_bg_use_default,
        set_popup_bg_red,
        set_popup_bg_green,
        set_popup_bg_blue,
        set_popup_bg_hex
    );
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
    dictionary_provider: String,
    show_prompt: bool,
    show_dictionary: bool,
    spell_check: bool,
    from_lang: String,
    to_lang: String,
    from_code: String,
    to_code: String,
    text: String,
}

/// Raw (un-prompt-formatted) translation result, handed to [`spawn_translation`]'s
/// `on_done` callback alongside the already-formatted [`TranscriptEntry`] -- the
/// Stage 6/9 popup uses this to format its own phrase/translation lines with its own
/// independent `popup_show_prompt`/`popup_show_phrase` settings, rather than
/// inheriting whatever the transcript's `show_prompt` baked into `TranscriptEntry`.
struct TranslationOutcome {
    from_lang: String,
    to_lang: String,
    phrase_raw: String,
    translation_raw: String,
    /// `true` when `translation_raw` is already a formatted `"Error: ..."` message
    /// (never itself lang-prompt-formatted, same as the transcript's own handling).
    is_error: bool,
}

/// Callback type for [`spawn_translation`]'s `on_done` parameter — named (rather than
/// spelled out inline) because `clippy::type_complexity` flags it inline once it grew
/// a `&TranscriptEntry` argument for Stage 6.
type TranslationDoneCallback = Box<dyn FnOnce(&TranscriptEntry, &TranslationOutcome) + Send>;

/// Everything [`start_speaking`] needs about *what* to speak, grouped into one struct
/// rather than passed as separate arguments -- clippy's `too_many_arguments` threshold
/// is 7, and this is naturally past that once the shared state (`window`/
/// `config_manager`/`speech_stop_flag`/`weak`) is included too. Mirrors
/// [`TranslationRequest`]'s own reason for existing.
struct SpeakRequest {
    /// Row index into `transcript-entries` to mark as speaking.
    index: i32,
    /// Whether `index`'s phrase side (vs. translation side) is the one speaking.
    is_phrase: bool,
    /// Raw text to speak.
    text: String,
    /// Provider language code, possibly `"auto"` (resolved lazily).
    code: String,
}

/// Starts speaking `request.text` in `request.code` (resolving `"auto"` lazily) and
/// marks transcript row `request.index`'s `request.is_phrase` side as the one currently
/// speaking, via a background thread. Extracted from `on_speak_requested`'s own "nothing
/// is currently speaking yet, start a new playback" branch (Stage 10 follow-up) so the
/// speech hotkey's trigger handler can reuse the exact same code path: it pushes a new
/// transcript entry for whatever it just grabbed from the clipboard, then calls this
/// with that entry's own (freshly assigned) index and `is_phrase: true`, exactly as if
/// the user had clicked that row's own 🔊 button — no separate state machine for
/// hotkey- vs. button-triggered speech.
///
/// Callers are expected to have already confirmed nothing else is currently speaking
/// (`window.get_speaking_entry_index() == -1`) and that `request.text` is non-empty.
fn start_speaking(
    window: &AppWindow,
    config_manager: &Arc<Mutex<GuiConfigManager>>,
    speech_stop_flag: &Arc<Mutex<Option<Arc<AtomicBool>>>>,
    weak: slint::Weak<AppWindow>,
    request: SpeakRequest,
) {
    let SpeakRequest {
        index,
        is_phrase,
        text,
        code,
    } = request;

    let stop_flag = Arc::new(AtomicBool::new(false));
    *speech_stop_flag.lock().unwrap() = Some(stop_flag.clone());
    window.set_speaking_entry_index(index);
    window.set_speaking_is_phrase(is_phrase);

    let config_manager = config_manager.clone();
    let speech_stop_flag = speech_stop_flag.clone();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
        let outcome: Result<(), Box<dyn std::error::Error + Send + Sync>> =
            runtime.block_on(async {
                let (translate_provider, speech_provider) = {
                    let mut manager = config_manager.lock().unwrap();
                    manager.check_and_reload();
                    let cfg = manager.config();
                    (cfg.translate_provider.clone(), cfg.speech_provider.clone())
                };
                let speech_provider = providers::create_speech_provider(&speech_provider)?;
                // Speech has its own provider, independent of `translate_provider`: a
                // translate provider is only constructed when `code == "auto"` actually
                // needs `detect_language`, so a broken/unimplemented translate provider
                // never blocks speaking a concrete language (every translation-side
                // call, and every phrase-side call where the source language wasn't
                // "Auto").
                let lang_code = if code == "auto" {
                    match providers::create_provider(&translate_provider) {
                        Ok(translate) => {
                            providers::resolve_source_language(translate.as_ref(), &text, "auto")
                                .await
                        }
                        Err(err) => {
                            eprintln!("Language detection unavailable ({err}); using 'en'");
                            "en".to_string()
                        }
                    }
                } else {
                    code
                };
                speech::speak(speech_provider.as_ref(), &text, &lang_code, stop_flag).await
            });
        if let Err(err) = outcome {
            eprintln!("Speech error: {err}");
        }

        // Cleared together with speaking-entry-index, both on the UI thread -- not
        // separately here, or a click on the active ⏹ landing in the gap between this
        // thread clearing the flag and the event-loop hop below actually running
        // would find speaking-entry-index still set but the stop flag already gone,
        // and silently do nothing.
        slint::invoke_from_event_loop(move || {
            *speech_stop_flag.lock().unwrap() = None;
            if let Some(window) = weak.upgrade() {
                window.set_speaking_entry_index(-1);
            }
        })
        .ok();
    });
}

/// Translates `request.text` in a background thread and pushes the result (or an error)
/// into the transcript. Shared by the Translate button/Enter key
/// (`on_translate_requested`) and the global translate hotkey (Stage 5) — the only two
/// callers, extracted here specifically to avoid duplicating the provider-call/
/// transcript-push logic between them.
///
/// `on_done`, if given, runs on the UI thread with the resolved [`TranscriptEntry`] and
/// the raw [`TranslationOutcome`] it was built from, before either is pushed into the
/// transcript. The hotkey path (Stage 5/6) uses this to clear its "already processing"
/// guard and to populate+show the Stage 6 popup; the button path has no such guard and
/// no popup, and passes `None`.
fn spawn_translation(
    weak: slint::Weak<AppWindow>,
    request: TranslationRequest,
    on_done: Option<TranslationDoneCallback>,
) {
    let TranslationRequest {
        translate_provider,
        dictionary_provider,
        show_prompt,
        show_dictionary,
        spell_check,
        from_lang,
        to_lang,
        from_code,
        to_code,
        text,
    } = request;

    // Trimmed here (not left to each caller) so both the button/Enter path and the
    // hotkey path -- which passes the clipboard text untrimmed, see the hotkey
    // callback in main() -- agree on what "the text" is before it reaches
    // `dictionary::is_single_word` or the corrected-word comparison below. Before
    // this fix, an untrimmed hotkey selection like "violent\n" could still take the
    // dictionary path (is_single_word trims non-alphabetic edge characters on its
    // own) but then spuriously report a spelling correction, since the comparison
    // would see "violent" (from the provider) against "violent\n" (the untrimmed
    // original) and treat them as different.
    let text = text.trim().to_string();

    std::thread::spawn(move || {
        let runtime = tokio::runtime::Runtime::new().expect("Failed to start Tokio runtime");
        let request_text = text.clone();
        // Cloned before from_code/to_code are moved into the async block below --
        // needed again afterward, in invoke_from_event_loop, for TranscriptEntry
        // (Stage 10).
        let from_code_for_entry = from_code.clone();
        let to_code_for_entry = to_code.clone();
        // (display_body, speech_text) -- speech_text is the raw *primary* translation
        // only (Stage 10): for a plain translation the two are identical, but for a
        // Stage 9 dictionary hit display_body is the full formatted block while
        // speech_text is just its header line (dictionary::primary_line), so the
        // per-entry speaker button never reads out part-of-speech/synonym lists.
        let result = runtime.block_on(async move {
            let provider = providers::create_provider(&translate_provider)?;

            // Built only when a dictionary lookup is actually about to happen, and never
            // fatally: a bad `dictionary_provider` value must not break translation, so
            // it warns and takes the plain-translation path below instead.
            let dictionary_provider =
                if show_dictionary && dictionary::is_single_word(&request_text) {
                    match providers::create_dictionary_provider(&dictionary_provider) {
                        Ok(dictionary) => Some(dictionary),
                        Err(e) => {
                            eprintln!(
                                "Dictionary provider unavailable ({e}); dictionary lookups disabled"
                            );
                            None
                        }
                    }
                } else {
                    None
                };

            if let Some(dictionary_provider) = dictionary_provider {
                let (translate_result, dict_result) = tokio::join!(
                    provider.translate_text(&request_text, &from_code, &to_code),
                    dictionary_provider.lookup(&request_text, &from_code, &to_code),
                );

                match dict_result {
                    Ok(Some(entry)) => {
                        let mut body = String::new();
                        if spell_check {
                            if let Some(corrected) = &entry.corrected_word {
                                if corrected.to_lowercase() != request_text.to_lowercase() {
                                    body.push_str(&dictionary::correction_notice(
                                        corrected, &to_code,
                                    ));
                                    body.push_str("\n\n");
                                }
                            }
                        }
                        body.push_str(&dictionary::format_dictionary_entry(
                            &entry,
                            &to_code,
                            translate_result.as_deref().ok(),
                        ));
                        let speech_text =
                            dictionary::primary_line(&entry, translate_result.as_deref().ok())
                                .unwrap_or_default();
                        Ok((body, speech_text))
                    }
                    // No dictionary entry (word not found / provider returned None) or a
                    // dictionary-lookup error: fall back to the plain translation already
                    // fetched above rather than a second network call -- `translate_result`
                    // is already the exact `Result<String, tagent::error::Error>` this
                    // function needs to return.
                    _ => translate_result.map(|t| (t.clone(), t)),
                }
            } else {
                provider
                    .translate_text(&request_text, &from_code, &to_code)
                    .await
                    .map(|t| (t.clone(), t))
            }
        });

        slint::invoke_from_event_loop(move || {
            let (translation_raw, translation_speech, is_error) = match &result {
                Ok((body, speech_text)) => (body.clone(), speech_text.clone(), false),
                Err(err) => {
                    let message = format!("Error: {err}");
                    (message.clone(), message, true)
                }
            };

            let entry = TranscriptEntry {
                phrase: format_line(show_prompt, &from_lang, &text).into(),
                translation: if is_error {
                    translation_raw.clone().into()
                } else {
                    format_line(show_prompt, &to_lang, &translation_raw).into()
                },
                phrase_speech: text.clone().into(),
                translation_speech: if is_error {
                    String::new().into()
                } else {
                    translation_speech.clone().into()
                },
                from_code: from_code_for_entry.into(),
                to_code: to_code_for_entry.into(),
                translation_is_error: is_error,
            };
            let outcome = TranslationOutcome {
                from_lang: from_lang.clone(),
                to_lang: to_lang.clone(),
                phrase_raw: text.clone(),
                translation_raw,
                is_error,
            };
            if let Some(on_done) = on_done {
                on_done(&entry, &outcome);
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
/// previous run -- or otherwise re-asserts [`DEFAULT_WINDOW_SIZE`] explicitly. That
/// same first-show gate also re-runs [`apply_style`] (see its doc comment), for the
/// same underlying reason: some window state doesn't settle to its real value until
/// the window has an actual on-screen surface, so anything resolved at window
/// *creation* time (well before `start_minimized` users ever hit their first real
/// `show()`) can still be showing a stale/default value here.
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

    // Re-resolve Auto-theme-dependent colors now that the window has a real
    // on-screen surface -- see `apply_style`'s doc comment for why a fixed
    // delay from window *creation* isn't enough here. Immediate call plus a
    // short deferred retry, the same settle-and-retry shape as the geometry
    // re-apply below.
    apply_style(window, config_manager.lock().unwrap().config());
    {
        let weak_window = window.as_weak();
        let config_manager = config_manager.clone();
        slint::Timer::single_shot(std::time::Duration::from_millis(150), move || {
            if let Some(window) = weak_window.upgrade() {
                apply_style(&window, config_manager.lock().unwrap().config());
            }
        });
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
    #[cfg(target_os = "windows")]
    platform::windows::console::attach_parent();

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

    // Stage 10: the stop flag for whichever text-to-speech playback is currently
    // in flight, if any -- `None` while nothing is speaking. Set by
    // `on_speak_requested` when it starts a new playback, read by the same
    // handler when the *same* row's button is clicked again (stop), and cleared
    // by the playback thread itself once it finishes (naturally or via the flag).
    let speech_stop_flag: Arc<Mutex<Option<Arc<AtomicBool>>>> = Arc::new(Mutex::new(None));

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

    window.set_app_version(env!("CARGO_PKG_VERSION").into());
    apply_style(&window, config_manager.lock().unwrap().config());
    window.set_tts_enabled(
        config_manager
            .lock()
            .unwrap()
            .config()
            .enable_text_to_speech,
    );

    // Stage 6: one persistent popup instance, reused (repositioned/re-texted/
    // re-shown) on every hotkey trigger rather than constructed per trigger --
    // see the "one persistent PopupWindow instance" note in the Stage 6 plan.
    // `popup` itself must stay alive for the rest of `main()` (never dropped
    // early), same lifetime reasoning as `window`/`config_manager` below.
    // Created here (before `theme_poll_timer` below) so its own style can be
    // kept in sync by that same timer.
    let popup = TranslationPopup::new()?;
    apply_popup_style(&popup, config_manager.lock().unwrap().config());
    let popup_weak = popup.as_weak();

    // Live-tracks the system's dark/light preference for the `Auto` theme while
    // the app keeps running. `apply_style`'s baked snapshots (`panel-background`,
    // phrase/translation colors) don't react to `Palette` changes on their own --
    // see that function's doc comment -- so without this, toggling the OS theme
    // while `tagent-gui` is already running leaves those colors stuck at
    // whatever they resolved to at the last `apply_style` call, even though the
    // window's OS-drawn decorations and Palette-bound elements (`field-background`,
    // the input bar's frame, etc.) update immediately on their own. `apply_popup_style`
    // has the same staleness issue for the popup's own "theme default" color/background.
    //
    // Not event-driven: Slint doesn't expose a "system theme changed" callback,
    // and (per the `apply-theme` doc comment in `app.slint`) this project
    // deliberately avoids reaching for Slint's private `ColorScheme` type from
    // Rust to build one. Polling instead, at a light 1s interval; calling
    // `apply_style`/`apply_popup_style` when nothing actually changed is harmless;
    // they resolve the same values they already set. Skips the work entirely once
    // `config.theme` isn't `"auto"`, since an explicit Light/Dark theme never
    // changes live.
    //
    // `theme_poll_timer` must be kept alive for the timer to keep firing --
    // bound here so it lives until `main` returns (i.e. until
    // `run_event_loop_until_quit()` below exits).
    let theme_poll_timer = slint::Timer::default();
    let weak_window_for_theme_poll = window.as_weak();
    let weak_popup_for_theme_poll = popup.as_weak();
    let config_manager_for_theme_poll = config_manager.clone();
    theme_poll_timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_secs(1),
        move || {
            let config = config_manager_for_theme_poll
                .lock()
                .unwrap()
                .config()
                .clone();
            if config.theme != "auto" {
                return;
            }
            if let Some(window) = weak_window_for_theme_poll.upgrade() {
                apply_style(&window, &config);
            }
            if let Some(popup) = weak_popup_for_theme_poll.upgrade() {
                apply_popup_style(&popup, &config);
            }
        },
    );

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

    wire_popup_drag(&popup, &config_manager);

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

        let (
            translate_provider,
            dictionary_provider,
            show_prompt,
            show_dictionary,
            spell_check,
            enable_text_to_speech,
        ) = {
            let mut manager = config_manager.lock().unwrap();
            manager.check_and_reload();
            let cfg = manager.config();
            (
                cfg.translate_provider.clone(),
                cfg.dictionary_provider.clone(),
                cfg.show_prompt,
                cfg.show_dictionary,
                cfg.spell_check,
                cfg.enable_text_to_speech,
            )
        };

        if let Some(window) = weak.upgrade() {
            window.set_tts_enabled(enable_text_to_speech);
        }

        if to_code == "auto" {
            if let Some(window) = weak.upgrade() {
                push_transcript_entry(
                    &window,
                    info_transcript_entry(
                        format_line(show_prompt, &from_lang, &text),
                        "Error: \"Auto\" is not a valid target language",
                    ),
                );
            }
            return;
        }

        spawn_translation(
            weak.clone(),
            TranslationRequest {
                translate_provider,
                dictionary_provider,
                show_prompt,
                show_dictionary,
                spell_check,
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
                            info_transcript_entry("[Clipboard]", format!("Error: {err}")),
                        ),
                    }
                }
            })
            .ok();
        });
    });

    let window_weak_for_settings = window.as_weak();
    let popup_weak_for_settings = popup.as_weak();
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

        seed_dialog_fields(&dialog, &current_config);

        let dialog_weak = dialog.as_weak();
        dialog.on_translate_hotkey_edited(move |text| {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.set_translate_hotkey_error(hotkey_validation_error(text.as_str()).into());
            }
        });

        let dialog_weak = dialog.as_weak();
        dialog.on_speech_hotkey_edited(move |text| {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.set_speech_hotkey_error(hotkey_validation_error(text.as_str()).into());
            }
        });

        dialog.on_map_key_to_token(|text| slint_key_text_to_hotkey_token(text.as_str()).into());

        const UNRECOGNIZED_KEY_MESSAGE: &str =
            "Couldn't recognize that key — if you're on a non-Latin keyboard \
             layout, switch to a Latin layout while recording, or type the \
             hotkey manually above.";

        let dialog_weak = dialog.as_weak();
        dialog.on_translate_hotkey_unrecognized(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.set_translate_hotkey_error(UNRECOGNIZED_KEY_MESSAGE.into());
            }
        });

        let dialog_weak = dialog.as_weak();
        dialog.on_speech_hotkey_unrecognized(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.set_speech_hotkey_error(UNRECOGNIZED_KEY_MESSAGE.into());
            }
        });

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
        wire_hex_committed!(
            dialog,
            on_popup_color_hex_committed,
            set_popup_color_red,
            set_popup_color_green,
            set_popup_color_blue,
            set_popup_color_use_default
        );
        wire_hex_committed!(
            dialog,
            on_popup_bg_hex_committed,
            set_popup_bg_red,
            set_popup_bg_green,
            set_popup_bg_blue,
            set_popup_bg_use_default
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
        wire_rgb_changed!(
            dialog,
            on_popup_color_rgb_changed,
            get_popup_color_red,
            get_popup_color_green,
            get_popup_color_blue,
            set_popup_color_hex
        );
        wire_rgb_changed!(
            dialog,
            on_popup_bg_rgb_changed,
            get_popup_bg_red,
            get_popup_bg_green,
            get_popup_bg_blue,
            set_popup_bg_hex
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

            apply_scheme_theme(&dialog, scheme.theme);
        });

        let dialog_weak = dialog.as_weak();
        let config_manager_for_save = config_manager_for_settings.clone();
        let window_weak_for_save = window_weak_for_settings.clone();
        let popup_weak_for_save = popup_weak_for_settings.clone();
        dialog.on_save_requested(move |provider, theme| {
            let Some(dialog) = dialog_weak.upgrade() else {
                return;
            };

            let new_config = config::GuiConfig {
                translate_provider: provider.to_string(),
                theme: theme.to_lowercase(),
                show_dictionary: dialog.get_show_dictionary(),
                spell_check: dialog.get_spell_check(),
                enable_text_to_speech: dialog.get_enable_text_to_speech(),
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
                popup_font: FONT_FAMILIES[dialog
                    .get_popup_font_index()
                    .clamp(0, FONT_FAMILIES.len() as i32 - 1)
                    as usize]
                    .to_string(),
                popup_size: dialog.get_popup_size(),
                popup_color: color_field_hex(
                    dialog.get_popup_color_use_default(),
                    dialog.get_popup_color_red(),
                    dialog.get_popup_color_green(),
                    dialog.get_popup_color_blue(),
                ),
                popup_background: color_field_hex(
                    dialog.get_popup_bg_use_default(),
                    dialog.get_popup_bg_red(),
                    dialog.get_popup_bg_green(),
                    dialog.get_popup_bg_blue(),
                ),
                popup_show_prompt: dialog.get_popup_show_prompt(),
                popup_show_phrase: dialog.get_popup_show_phrase(),
                popup_max_width: dialog.get_popup_max_width(),
                popup_max_height: dialog.get_popup_max_height(),
                popup_border_width: dialog.get_popup_border_width(),
                block_spacing_px: dialog.get_block_spacing_px(),
                phrases_spacing_px: dialog.get_phrases_spacing_px(),
                show_prompt: dialog.get_show_prompt(),
                translate_hotkey: dialog.get_translate_hotkey().to_string(),
                speech_hotkey: dialog.get_speech_hotkey().to_string(),
                enable_speech_hotkey: dialog.get_enable_speech_hotkey(),
                popup_auto_hide_seconds: dialog.get_popup_auto_hide_seconds() as u64,
                start_minimized: dialog.get_start_minimized(),
                remember_window_geometry: dialog.get_remember_window_geometry(),
                // Not dialog-editable -- captured automatically from the real window
                // (see save_window_geometry) -- so carried through unchanged, same
                // treatment as translate_hotkey/popup_auto_hide_seconds above.
                window_geometry: current_config.window_geometry,
                remember_popup_position: dialog.get_remember_popup_position(),
                // Also not dialog-editable (captured by dragging the popup), but read
                // fresh from the live config rather than from `current_config`: a drag
                // can land while the dialog is open, and its position mustn't be
                // overwritten with the stale copy taken when the dialog opened.
                popup_position: config_manager_for_save
                    .lock()
                    .unwrap()
                    .config()
                    .popup_position,
                dictionary_provider: combo_selection(
                    &dialog.get_dictionary_providers(),
                    dialog.get_dictionary_provider_index(),
                    &current_config.dictionary_provider,
                ),
                speech_provider: combo_selection(
                    &dialog.get_speech_providers(),
                    dialog.get_speech_provider_index(),
                    &current_config.speech_provider,
                ),
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
                window.set_tts_enabled(new_config.enable_text_to_speech);
            }
            if let Some(popup) = popup_weak_for_save.upgrade() {
                apply_popup_style(&popup, &new_config);
            }
            dialog.hide().ok();
        });

        let dialog_weak = dialog.as_weak();
        dialog.on_cancel_requested(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                dialog.hide().ok();
            }
        });

        let dialog_weak = dialog.as_weak();
        dialog.on_reset_to_defaults_requested(move || {
            if let Some(dialog) = dialog_weak.upgrade() {
                seed_dialog_fields(&dialog, &config::GuiConfig::default());
            }
        });

        dialog.show().unwrap();
    });

    // Stage 10: per-entry text-to-speech speaker buttons. `index`/`is_phrase`
    // identify which row/side was clicked; `speaking-entry-index`/
    // `speaking-is-phrase` (app.slint) are the single shared "who's currently
    // speaking" state every row's button checks -- only one Sink ever plays at
    // a time, mirroring tagent-cli's own single-Sink design.
    let weak_for_speak = window.as_weak();
    let config_manager_for_speak = config_manager.clone();
    let speech_stop_flag_for_speak = speech_stop_flag.clone();
    window.on_speak_requested(move |index, is_phrase| {
        let Some(window) = weak_for_speak.upgrade() else {
            return;
        };

        if window.get_speaking_entry_index() != -1 {
            // Either a click on the currently-speaking row's own button (stop
            // it), or -- defensively, since .slint already disables every
            // other row's button while one is speaking -- a different row
            // (ignored).
            if window.get_speaking_entry_index() == index
                && window.get_speaking_is_phrase() == is_phrase
            {
                if let Some(flag) = speech_stop_flag_for_speak.lock().unwrap().as_ref() {
                    flag.store(true, Ordering::Relaxed);
                }
            }
            return;
        }

        let Some(entry) = window.get_transcript_entries().row_data(index as usize) else {
            return;
        };
        let text = if is_phrase {
            entry.phrase_speech.to_string()
        } else {
            entry.translation_speech.to_string()
        };
        let code = if is_phrase {
            entry.from_code.to_string()
        } else {
            entry.to_code.to_string()
        };
        if text.trim().is_empty() {
            return;
        }

        start_speaking(
            &window,
            &config_manager_for_speak,
            &speech_stop_flag_for_speak,
            weak_for_speak.clone(),
            SpeakRequest {
                index,
                is_phrase,
                text,
                code,
            },
        );
    });

    // Global hotkeys (Stage 5; speech hotkey added Stage 10 follow-up): parse+validate
    // both once at startup from the hand-editable `translate_hotkey`/`speech_hotkey`
    // config fields. `translate_hotkey` stays the hard gate: if it fails to parse, the
    // whole hook (speech hotkey and Escape observation included) stays disabled, same
    // as before this follow-up -- unchanged behavior, not a new decision. A failed
    // `speech_hotkey` (or `enable_speech_hotkey: false`) only disables that one hotkey;
    // `translate_hotkey`, if valid, still gets registered. On any failure, log a
    // warning and leave that hotkey disabled rather than failing to start — same "log
    // and keep running" convention `tagent-cli` uses for its own hotkeys. Changes to
    // either hotkey string take effect only on restart (no live-reload of the OS-level
    // grab itself).
    let (hotkey_str, speech_hotkey_str, enable_speech_hotkey) = {
        let cfg = config_manager.lock().unwrap();
        let cfg = cfg.config();
        (
            cfg.translate_hotkey.clone(),
            cfg.speech_hotkey.clone(),
            cfg.enable_speech_hotkey,
        )
    };
    match config::HotkeyParser::parse(&hotkey_str)
        .and_then(|h| config::HotkeyParser::validate_hotkey(&h).map(|_| h))
    {
        Ok(hotkey) => {
            let speech_hotkey = if enable_speech_hotkey {
                match config::HotkeyParser::parse(&speech_hotkey_str)
                    .and_then(|h| config::HotkeyParser::validate_hotkey(&h).map(|_| h))
                {
                    Ok(h) => Some(h),
                    Err(e) => {
                        eprintln!(
                            "Warning: failed to parse/validate speech_hotkey '{}': {}",
                            speech_hotkey_str, e
                        );
                        eprintln!("Speech hotkey disabled.");
                        None
                    }
                }
            } else {
                None
            };

            // Shown in the transcript header: only what is actually registered.
            window.set_active_translate_hotkey(hotkey_str.clone().into());
            if speech_hotkey.is_some() {
                window.set_active_speech_hotkey(speech_hotkey_str.clone().into());
            }

            let is_processing = Arc::new(AtomicBool::new(false));
            let is_speech_processing = Arc::new(AtomicBool::new(false));
            let weak = window.as_weak();
            let config_manager = config_manager.clone();
            let popup_weak_for_hotkey = popup_weak.clone();
            let recording_started_at = recording_started_at.clone();

            let recording_started_at_for_speech = recording_started_at.clone();
            let weak_for_speech = window.as_weak();
            let config_manager_for_speech = config_manager.clone();
            let speech_stop_flag_for_speech_trigger = speech_stop_flag.clone();
            let on_speech_trigger = move || {
                // Same fast-return constraints as the translate trigger below -- runs on
                // the platform hook's own thread.
                if let Some(started) = *recording_started_at_for_speech.lock().unwrap() {
                    if started.elapsed() < RECORDING_SUPPRESSION_TIMEOUT {
                        return;
                    }
                }

                if is_speech_processing.swap(true, Ordering::SeqCst) {
                    return; // already handling a previous trigger
                }

                let is_speech_processing = is_speech_processing.clone();
                let weak = weak_for_speech.clone();
                let config_manager = config_manager_for_speech.clone();
                let speech_stop_flag = speech_stop_flag_for_speech_trigger.clone();
                slint::invoke_from_event_loop(move || {
                    let Some(window) = weak.upgrade() else {
                        is_speech_processing.store(false, Ordering::SeqCst);
                        return;
                    };

                    // Deliberately a no-op, not a stop -- Esc is the only way to cancel
                    // this hotkey's speech (design decision 3, Stage 10 follow-up plan).
                    // Reuses the exact same "is anything currently speaking" signal the
                    // transcript buttons themselves check.
                    if window.get_speaking_entry_index() != -1 {
                        is_speech_processing.store(false, Ordering::SeqCst);
                        return;
                    }

                    // translate_provider/speech_provider themselves aren't needed here --
                    // start_speaking (below) re-reads both fresh from config right
                    // before they're actually used to build providers, same as every
                    // other speech-starting path.
                    let enable_text_to_speech = {
                        let mut manager = config_manager.lock().unwrap();
                        manager.check_and_reload();
                        manager.config().enable_text_to_speech
                    };
                    window.set_tts_enabled(enable_text_to_speech);
                    if !enable_text_to_speech {
                        is_speech_processing.store(false, Ordering::SeqCst);
                        return;
                    }

                    let from_code = languages::name_to_code(
                        &window
                            .get_languages()
                            .row_data(window.get_source_language_index() as usize)
                            .unwrap_or_default(),
                    )
                    .to_string();

                    let weak2 = weak.clone();
                    let is_speech_processing2 = is_speech_processing.clone();
                    let config_manager2 = config_manager.clone();
                    let speech_stop_flag2 = speech_stop_flag.clone();
                    std::thread::spawn(move || {
                        match ClipboardManager::new().get_text_with_copy() {
                            Ok(text) if !text.trim().is_empty() => {
                                slint::invoke_from_event_loop(move || {
                                    let Some(window) = weak2.upgrade() else {
                                        is_speech_processing2.store(false, Ordering::SeqCst);
                                        return;
                                    };
                                    push_transcript_entry(
                                        &window,
                                        TranscriptEntry {
                                            phrase: format!("[Speech]: {text}").into(),
                                            translation: "".into(),
                                            phrase_speech: text.clone().into(),
                                            translation_speech: "".into(),
                                            from_code: from_code.clone().into(),
                                            to_code: "".into(),
                                            translation_is_error: true,
                                        },
                                    );
                                    let index =
                                        window.get_transcript_entries().row_count() as i32 - 1;
                                    start_speaking(
                                        &window,
                                        &config_manager2,
                                        &speech_stop_flag2,
                                        weak2.clone(),
                                        SpeakRequest {
                                            index,
                                            is_phrase: true,
                                            text,
                                            code: from_code,
                                        },
                                    );
                                    is_speech_processing2.store(false, Ordering::SeqCst);
                                })
                                .ok();
                            }
                            Ok(_) => {
                                is_speech_processing2.store(false, Ordering::SeqCst);
                            }
                            Err(err) => {
                                slint::invoke_from_event_loop(move || {
                                    if let Some(window) = weak2.upgrade() {
                                        push_transcript_entry(
                                            &window,
                                            info_transcript_entry(
                                                "[Speech]",
                                                format!("Error: {err}"),
                                            ),
                                        );
                                    }
                                    is_speech_processing2.store(false, Ordering::SeqCst);
                                })
                                .ok();
                            }
                        }
                    });
                })
                .ok();
            };

            let speech_stop_flag_for_escape = speech_stop_flag.clone();
            let on_escape = move || {
                // Fast enough to run directly on the platform hook thread -- an
                // AtomicBool store, same class of operation as is_processing's own
                // guard check. Cancels whichever entry is currently speaking, hotkey-
                // or button-triggered, since both share this one flag (Stage 10
                // follow-up design decision 2).
                if let Some(flag) = speech_stop_flag_for_escape.lock().unwrap().as_ref() {
                    flag.store(true, Ordering::Relaxed);
                }
            };

            KeyboardHook::spawn(
                hotkey,
                speech_hotkey,
                move || {
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

                    let (
                        translate_provider,
                        dictionary_provider,
                        show_prompt,
                        show_dictionary,
                        spell_check,
                        enable_text_to_speech,
                        popup_auto_hide_seconds,
                        popup_show_prompt,
                        popup_show_phrase,
                        remembered_popup_position,
                    ) = {
                        let mut manager = config_manager.lock().unwrap();
                        manager.check_and_reload();
                        let cfg = manager.config();
                        (
                            cfg.translate_provider.clone(),
                            cfg.dictionary_provider.clone(),
                            cfg.show_prompt,
                            cfg.show_dictionary,
                            cfg.spell_check,
                            cfg.enable_text_to_speech,
                            cfg.popup_auto_hide_seconds_or_default(),
                            cfg.popup_show_prompt,
                            cfg.popup_show_phrase,
                            cfg.remember_popup_position
                                .then_some(cfg.popup_position)
                                .flatten(),
                        )
                    };
                    window.set_tts_enabled(enable_text_to_speech);

                    let from_code = languages::name_to_code(&from_lang).to_string();
                    let to_code = languages::name_to_code(&to_lang).to_string();

                    if to_code == "auto" {
                        push_transcript_entry(
                            &window,
                            info_transcript_entry(
                                "[Hotkey]",
                                "Error: \"Auto\" is not a valid target language",
                            ),
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
                                        dictionary_provider,
                                        show_prompt,
                                        show_dictionary,
                                        spell_check,
                                        from_lang: from_lang.to_string(),
                                        to_lang: to_lang.to_string(),
                                        from_code,
                                        to_code,
                                        text,
                                    },
                                    Some(Box::new(move |_entry: &TranscriptEntry, outcome: &TranslationOutcome| {
                                        is_processing2.store(false, Ordering::SeqCst);
                                        show_popup(
                                            &popup_weak2,
                                            outcome,
                                            popup_show_prompt,
                                            popup_show_phrase,
                                            popup_auto_hide_seconds,
                                            remembered_popup_position,
                                        );
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
                                            info_transcript_entry(
                                                "[Hotkey]",
                                                format!("Error: {err}"),
                                            ),
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
                },
                on_speech_trigger,
                on_escape,
            );
        }
        Err(e) => {
            eprintln!(
                "Warning: failed to parse/validate translate_hotkey '{}': {}",
                hotkey_str, e
            );
            eprintln!("Global hotkeys disabled.");
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

#[cfg(test)]
mod tests {
    use super::*;

    fn model(items: &[&str]) -> ModelRc<SharedString> {
        ModelRc::new(VecModel::from(
            items
                .iter()
                .map(|s| SharedString::from(*s))
                .collect::<Vec<_>>(),
        ))
    }

    #[test]
    fn combo_index_finds_entry_case_insensitively() {
        let m = model(&["google", "other"]);
        assert_eq!(combo_index(&m, "other"), 1);
        assert_eq!(combo_index(&m, "Google"), 0);
        assert_eq!(combo_index(&m, "OTHER"), 1);
    }

    #[test]
    fn combo_index_defaults_to_first_entry_for_unknown_name() {
        let m = model(&["google", "other"]);
        assert_eq!(combo_index(&m, "bogus"), 0);
        assert_eq!(combo_index(&m, ""), 0);
        assert_eq!(combo_index(&model(&[]), "google"), 0);
    }

    #[test]
    fn provider_choices_lists_the_names_and_selects_the_configured_one() {
        let (model, index) = provider_choices(&["google", "other"], "other");
        assert_eq!(model.row_count(), 2);
        assert_eq!(index, 1);
        assert_eq!(combo_selection(&model, index, ""), "other");
    }

    /// A fresh config must land on a real entry of every dropdown, not on the "unlisted
    /// name falls back to index 0" path.
    #[test]
    fn default_config_providers_are_all_offered_by_tagent() {
        let config = config::GuiConfig::default();
        for (names, current) in [
            (providers::TRANSLATION_PROVIDERS, &config.translate_provider),
            (providers::DICTIONARY_PROVIDERS, &config.dictionary_provider),
            (providers::SPEECH_PROVIDERS, &config.speech_provider),
        ] {
            assert!(
                names.iter().any(|n| n.eq_ignore_ascii_case(current)),
                "{current} missing from {names:?}"
            );
        }
    }

    #[test]
    fn combo_selection_returns_selected_entry() {
        let m = model(&["google", "other"]);
        assert_eq!(combo_selection(&m, 0, "fallback"), "google");
        assert_eq!(combo_selection(&m, 1, "fallback"), "other");
    }

    #[test]
    fn combo_selection_falls_back_when_index_is_out_of_range() {
        let m = model(&["google"]);
        assert_eq!(combo_selection(&m, 5, "fallback"), "fallback");
        assert_eq!(combo_selection(&m, -1, "fallback"), "fallback");
    }
}
