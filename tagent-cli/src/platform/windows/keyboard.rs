use super::keycodes::normalize_vk_code;
use crate::config::{self, ConfigManager, HotkeyParser, HotkeyType};
use crate::speech::SpeechManager;
use crate::translator::Translator;
use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use windows::{
    Win32::Foundation::*, Win32::System::LibraryLoader::GetModuleHandleW,
    Win32::UI::Input::KeyboardAndMouse::*, Win32::UI::WindowsAndMessaging::*,
};

static TRANSLATOR: OnceLock<Arc<Translator>> = OnceLock::new();
static IS_PROCESSING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_EXIT: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static MODIFIER_STATE: OnceLock<Arc<Mutex<HashMap<u32, bool>>>> = OnceLock::new();

/// The set of (normalized) modifier vk codes used by any currently-configured
/// `ModifierCombo` hotkey (translate and/or speech), of *any* modifier family (Alt, Ctrl,
/// Shift, Win). Computed once in `KeyboardHook::new` -- hotkey configuration is fixed for
/// the process lifetime (see module docs: "restart required" for hotkey changes), so this
/// never needs to change afterwards. Drives which keydowns/keyups are intercepted
/// centrally (`handle_combo_modifier_keydown`/`resolve_combo_modifier_keyup`) instead of
/// reaching `HotkeyState::handle`, so `MODIFIER_STATE` stays accurate for every combo
/// modifier -- not just the ones that additionally get swallow-and-replay (see
/// `needs_swallow_and_replay`, which narrows that down to `Alt` alone).
static COMBO_MODIFIER_VKS: OnceLock<HashSet<u32>> = OnceLock::new();

/// Per-modifier bookkeeping for the swallow-and-replay mechanism (see module docs on
/// `keyboard_hook_proc` for the full state machine). Keyed by normalized vk code. In
/// practice only ever holds `Alt` entries -- see `needs_swallow_and_replay` -- but keyed
/// generically rather than hardcoded to one constant in case that ever needs to widen.
static MODIFIER_REPLAY: OnceLock<Arc<Mutex<HashMap<u32, ModifierReplay>>>> = OnceLock::new();

// Hotkey state instances for translate and speech
static TRANSLATE_HOTKEY: OnceLock<HotkeyState> = OnceLock::new();
static SPEECH_HOTKEY: OnceLock<HotkeyState> = OnceLock::new();

// Speech-specific state
static SPEECH_HOTKEY_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static SPEECH_ENABLED: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static IS_SPEAKING: OnceLock<Arc<Mutex<bool>>> = OnceLock::new();
static SHOULD_STOP_SPEECH: OnceLock<Arc<AtomicBool>> = OnceLock::new();
static CONFIG_MANAGER: OnceLock<Arc<ConfigManager>> = OnceLock::new();

/// State of a single modifier hold, for the swallow-and-replay mechanism.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierReplayState {
    /// Its keydown was blocked from reaching the foreground app; not yet resolved.
    Swallowed,
    /// A `ModifierCombo` fired while this modifier was held. Its matching keyup must
    /// also be blocked so the app never observes an unpaired keyup.
    ConsumedByCombo,
    /// A synthetic keydown has already been replayed for this hold (because some other,
    /// non-trigger key arrived while it was swallowed). The real keyup must be let
    /// through normally to balance the synthetic press.
    PassedThrough,
}

/// Bookkeeping entry for one swallowed modifier hold.
#[derive(Debug, Clone, Copy)]
struct ModifierReplay {
    state: ModifierReplayState,
    /// The specific (non-normalized) vk code observed at keydown (e.g. `VK_LMENU`, not
    /// the generic `VK_MENU`), used so any replay matches the actual physical key.
    raw_vk: u32,
}

/// Outcome of resolving a combo-modifier keyup, split from its execution
/// (`apply_combo_modifier_keyup_action`) so the decision logic can be unit-tested without
/// triggering a real `SendInput` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModifierKeyupAction {
    /// Nothing was swallowed (or it was already replayed) -- let the real keyup through.
    PassThrough,
    /// A combo consumed this modifier; block the real keyup, nothing to replay.
    Block,
    /// A bare tap (never resolved by a combo or another key) -- block the real keyup and
    /// replay a synthetic down+up pair instead.
    ReplayPair { raw_vk: u32 },
}

/// Encapsulates hotkey detection state for a single hotkey.
/// Eliminates duplication between translate and speech hotkey handlers.
struct HotkeyState {
    config: Arc<Mutex<Option<HotkeyType>>>,
    last_key_time: Arc<Mutex<Option<Instant>>>,
    last_key_pressed: Arc<Mutex<bool>>,
    last_key_interrupted: Arc<Mutex<bool>>,
}

impl HotkeyState {
    fn new(hotkey: Option<HotkeyType>) -> Self {
        Self {
            config: Arc::new(Mutex::new(hotkey)),
            last_key_time: Arc::new(Mutex::new(None)),
            last_key_pressed: Arc::new(Mutex::new(false)),
            last_key_interrupted: Arc::new(Mutex::new(false)),
        }
    }

    /// Handle hotkey detection for key events.
    /// Returns true if the event was consumed (should be blocked).
    /// Calls `trigger_fn` when the hotkey combination is activated.
    ///
    /// For `ModifierCombo`, this is only ever meaningfully invoked with the trigger key
    /// (e.g. `Q`), not the modifier itself -- the modifier's own keydown/keyup is
    /// intercepted earlier in `keyboard_hook_proc` by the central swallow-and-replay
    /// path (see `handle_combo_modifier_keydown`/`resolve_combo_modifier_keyup`), which
    /// is also what keeps `MODIFIER_STATE` up to date for the check below.
    fn handle(&self, vk_code: u32, is_key_down: bool, trigger_fn: unsafe fn()) -> bool {
        let hotkey_opt = match self.config.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return false,
        };

        let hotkey = match hotkey_opt.as_ref() {
            Some(h) => h,
            None => return false,
        };

        match hotkey {
            HotkeyType::SingleKey { vk_code: target_vk } => {
                if is_key_down && vk_code == *target_vk {
                    unsafe { trigger_fn() };
                    return true;
                }
            }

            HotkeyType::ModifierCombo { modifiers, key } => {
                if is_key_down && vk_code == *key {
                    if let Some(modifier_state) = MODIFIER_STATE.get() {
                        if let Ok(state) = modifier_state.lock() {
                            let all_modifiers_pressed = modifiers
                                .iter()
                                .all(|m| state.get(m).copied().unwrap_or(false));

                            if all_modifiers_pressed {
                                unsafe { trigger_fn() };
                                return true;
                            }
                        }
                    }
                }
            }

            HotkeyType::DoublePress {
                vk_code: target_vk,
                min_interval_ms,
                max_interval_ms,
            } => {
                let normalized_vk = normalize_vk_code(vk_code);
                if normalized_vk == *target_vk {
                    if is_key_down {
                        // Check if this is a key repeat (auto-repeat from holding key down)
                        if let Ok(mut is_pressed) = self.last_key_pressed.lock() {
                            if *is_pressed {
                                return false;
                            }
                            *is_pressed = true;
                        }

                        if let Ok(mut last_time) = self.last_key_time.lock() {
                            let now = Instant::now();

                            match *last_time {
                                Some(last) => {
                                    let elapsed = now.duration_since(last);

                                    // Check if sequence was interrupted
                                    let was_interrupted =
                                        self.last_key_interrupted.lock().ok().is_some_and(|f| *f);

                                    if !was_interrupted
                                        && elapsed >= Duration::from_millis(*min_interval_ms)
                                        && elapsed < Duration::from_millis(*max_interval_ms)
                                    {
                                        unsafe { trigger_fn() };
                                        *last_time = None;
                                        return true;
                                    } else if elapsed >= Duration::from_millis(*max_interval_ms)
                                        || was_interrupted
                                    {
                                        // Start new sequence
                                        *last_time = Some(now);
                                        if let Ok(mut flag) = self.last_key_interrupted.lock() {
                                            *flag = false;
                                        }
                                    }
                                }
                                None => {
                                    // First press - start new sequence
                                    *last_time = Some(now);
                                    if let Ok(mut flag) = self.last_key_interrupted.lock() {
                                        *flag = false;
                                    }
                                }
                            }
                        }
                    } else {
                        // Key up event - mark key as not pressed
                        if let Ok(mut is_pressed) = self.last_key_pressed.lock() {
                            *is_pressed = false;
                        }
                    }
                }
            }
        }

        false
    }

    /// If this holds a `ModifierCombo`, returns its modifier vk codes (already normalized
    /// by `HotkeyParser::parse`). Used after `handle` reports a combo fired, to mark
    /// exactly those modifiers `ConsumedByCombo` (see `mark_swallowed_modifiers_consumed_by_combo`).
    fn combo_modifiers(&self) -> Option<Vec<u32>> {
        let hotkey_opt = self.config.lock().ok()?.clone();
        match hotkey_opt {
            Some(HotkeyType::ModifierCombo { modifiers, .. }) => Some(modifiers),
            _ => None,
        }
    }

    /// Mark double-press sequence as interrupted if another key was pressed
    fn mark_interrupted_if_needed(&self, vk_code: u32) {
        let hotkey_opt = match self.config.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => return,
        };

        if let Some(HotkeyType::DoublePress {
            vk_code: target_vk, ..
        }) = hotkey_opt.as_ref()
        {
            let normalized_vk = normalize_vk_code(vk_code);
            if normalized_vk != *target_vk {
                if let Ok(time) = self.last_key_time.lock() {
                    if time.is_some() {
                        if let Ok(mut flag) = self.last_key_interrupted.lock() {
                            *flag = true;
                        }
                    }
                }
            }
        }
    }
}

/// Global hotkey listener for Windows, driven by a low-level `WH_KEYBOARD_LL` hook and
/// a Win32 message loop. All state is process-global (`OnceLock` statics) because the
/// hook callback is a plain `extern "system" fn` with no user-data pointer.
///
/// `ModifierCombo` hotkeys (e.g. `Alt+Q`) are detected entirely within the hook itself.
/// Every combo modifier's keydown/keyup is intercepted centrally to keep `MODIFIER_STATE`
/// accurate (see `handle_combo_modifier_keydown`/`resolve_combo_modifier_keyup`), but only
/// `Alt` additionally gets swallow-and-replay treatment (`needs_swallow_and_replay`): its
/// keydown is blocked from reaching the foreground app the instant it's pressed, and
/// either consumed (if the combo completes) or transparently replayed as synthetic input
/// (if it doesn't -- a bare tap, or some other shortcut like Alt-Tab/Alt+F4). Ctrl/Shift/
/// Win are tracked the same way but never blocked -- they don't share Alt's Win32-level
/// side effect (see below), so blocking them would only add risk for no benefit; confirmed
/// by comparison with QTranslate, which simulates Ctrl+C with zero interception and has no
/// equivalent bug reports.
///
/// This intentionally does not use `RegisterHotKey` for the modifier: that API only
/// suppresses the final trigger key, never the modifier itself, so the modifier's raw
/// keydown always reached the foreground window first -- which is what put apps like
/// Word/Chrome/Notepad into a menu-tracking mode that ate the simulated Ctrl+C in
/// `ClipboardManager::copy_selected_text` no matter what was done to clean up afterwards.
/// That menu-tracking mode is specifically an `Alt` (and `F10`, never used as a
/// `ModifierCombo` modifier here) thing in Win32 -- holding Alt is what routes the next
/// key through `WM_SYSKEYDOWN` handling; `Ctrl`/`Shift`/`Win` have no equivalent. Blocking
/// Alt at the source avoids that class of problem entirely, at the cost of needing to
/// replay it convincingly when it wasn't ours.
///
/// **Confirmed working end-to-end (v0.16.0+007)**: Word/Chrome/Notepad copy correctly via
/// `Alt+Q`, and Alt-Tab/Alt+F4/the system menu are unaffected. Getting here took five
/// prior attempts across several versions (`RegisterHotKey`+`WM_HOTKEY`, `WM_CANCELMODE`,
/// `wScan`, `WM_COPY`, retry-with-verification -- see CHANGELOG `0.16.0+004` through
/// `+007`), so before refactoring anything in this module, preserve these invariants:
/// - **Never reintroduce `RegisterHotKey` for `ModifierCombo` hotkeys.** It structurally
///   cannot suppress the modifier's own keydown, only the trigger key -- that's the exact
///   defect this design replaced, not an oversight to "fix" by adding it back.
/// - **Never go back to unconditionally blocking a modifier for as long as it's held**
///   (i.e. no replay). That's what `0.12.0+004` fixed -- it broke Alt-Tab/Alt+F4/the
///   system menu system-wide for the whole time tagent was running.
/// - **Don't widen `needs_swallow_and_replay` beyond `Alt` without a concrete, reproduced
///   bug report for a specific other modifier.** Ctrl/Shift/Win have no known Win32 side
///   effect that swallow-and-replay would fix; widening "to be safe" only adds untested
///   risk.
/// - **The `LLKHF_INJECTED` early-return at the top of `keyboard_hook_proc` must stay
///   the very first check**, before any combo/modifier logic. It's what makes this
///   module's own `SendInput` replays (`replay_key`/`replay_keydown_only`) safe against
///   re-entering the hook and being swallowed a second time.
/// - **Every `ModifierReplay` entry must be resolved on the matching keyup or combo
///   completion** (see `ModifierReplayState`'s three variants). A modifier left dangling
///   in `Swallowed` after its keyup is missed would stay permanently blocked for the rest
///   of the process, silently reintroducing the exact bug this design fixes.
pub struct KeyboardHook;

impl KeyboardHook {
    /// Initialize global hook state (translator, hotkey configs for translate and
    /// speech, exit flag) from `config_manager`. Does not install the hook itself;
    /// call [`KeyboardHook::start`] for that. Must only be called once per process —
    /// returns an error if any of the underlying `OnceLock`s are already set.
    pub fn new(
        translator: Translator,
        should_exit: Arc<AtomicBool>,
        config_manager: Arc<ConfigManager>,
    ) -> Result<Self, Box<dyn Error + Send + Sync>> {
        TRANSLATOR
            .set(Arc::new(translator))
            .map_err(|_| "Translator already initialized")?;

        CONFIG_MANAGER
            .set(config_manager.clone())
            .map_err(|_| "ConfigManager already initialized")?;

        IS_PROCESSING
            .set(Arc::new(Mutex::new(false)))
            .map_err(|_| "IsProcessing already initialized")?;
        SHOULD_EXIT
            .set(should_exit)
            .map_err(|_| "ShouldExit already initialized")?;

        MODIFIER_STATE
            .set(Arc::new(Mutex::new(HashMap::new())))
            .map_err(|_| "ModifierState already initialized")?;

        MODIFIER_REPLAY
            .set(Arc::new(Mutex::new(HashMap::new())))
            .map_err(|_| "ModifierReplay already initialized")?;

        // Initialize translation hotkey
        let config = config_manager.get_config();

        let translate_hotkey = match HotkeyParser::parse(&config.translate_hotkey) {
            Ok(hotkey) => match HotkeyParser::validate_hotkey(&hotkey) {
                Ok(_) => Some(hotkey),
                Err(e) => {
                    eprintln!(
                        "Warning: Hotkey validation failed for '{}': {}",
                        config.translate_hotkey, e
                    );
                    eprintln!("Translation hotkey disabled.");
                    None
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: Failed to parse hotkey '{}': {}",
                    config.translate_hotkey, e
                );
                eprintln!("Translation hotkey disabled.");
                None
            }
        };

        // Initialize speech hotkey
        let speech_hotkey = if config.enable_speech_hotkey && config.enable_text_to_speech {
            match HotkeyParser::parse(&config.speech_hotkey) {
                Ok(hotkey) => match HotkeyParser::validate_hotkey(&hotkey) {
                    Ok(_) => Some(hotkey),
                    Err(e) => {
                        eprintln!(
                            "Warning: Speech hotkey validation failed for '{}': {}",
                            config.speech_hotkey, e
                        );
                        eprintln!("Speech hotkey disabled.");
                        None
                    }
                },
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse speech hotkey '{}': {}",
                        config.speech_hotkey, e
                    );
                    eprintln!("Speech hotkey disabled.");
                    None
                }
            }
        } else {
            None
        };

        // Collect every modifier used by either hotkey's ModifierCombo, before the
        // configs are moved into their HotkeyStates below -- this is what routes their
        // keydown/keyup through the central swallow-and-replay path in keyboard_hook_proc
        // instead of HotkeyState::handle.
        let mut combo_modifiers = HashSet::new();
        if let Some(HotkeyType::ModifierCombo { modifiers, .. }) = &translate_hotkey {
            combo_modifiers.extend(modifiers.iter().copied());
        }
        if let Some(HotkeyType::ModifierCombo { modifiers, .. }) = &speech_hotkey {
            combo_modifiers.extend(modifiers.iter().copied());
        }
        COMBO_MODIFIER_VKS
            .set(combo_modifiers)
            .map_err(|_| "ComboModifierVks already initialized")?;

        TRANSLATE_HOTKEY
            .set(HotkeyState::new(translate_hotkey))
            .map_err(|_| "TranslateHotkey already initialized")?;

        SPEECH_HOTKEY
            .set(HotkeyState::new(speech_hotkey))
            .map_err(|_| "SpeechHotkey already initialized")?;

        SPEECH_HOTKEY_ENABLED
            .set(Arc::new(AtomicBool::new(config.enable_speech_hotkey)))
            .map_err(|_| "SpeechHotkeyEnabled already initialized")?;

        SPEECH_ENABLED
            .set(Arc::new(AtomicBool::new(config.enable_text_to_speech)))
            .map_err(|_| "SpeechEnabled already initialized")?;

        IS_SPEAKING
            .set(Arc::new(Mutex::new(false)))
            .map_err(|_| "IsSpeaking already initialized")?;

        SHOULD_STOP_SPEECH
            .set(Arc::new(AtomicBool::new(false)))
            .map_err(|_| "ShouldStopSpeech already initialized")?;

        Ok(Self)
    }

    /// Install the low-level keyboard hook and run the Win32 message loop until
    /// `should_exit` is set or `WM_QUIT` is received, then uninstall the hook.
    pub async fn start(&mut self) -> Result<(), Box<dyn Error + Send + Sync>> {
        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0)?;

            if hook.0 == 0 {
                return Err("Failed to set keyboard hook".into());
            }

            loop {
                // Check if we should exit
                if let Some(should_exit) = SHOULD_EXIT.get() {
                    if should_exit.load(Ordering::Relaxed) {
                        break;
                    }
                }

                let mut msg = MSG::default();

                // Use PeekMessage instead of GetMessage to avoid blocking
                let has_message = PeekMessageW(
                    &mut msg,
                    HWND::default(),
                    0,
                    0,
                    PEEK_MESSAGE_REMOVE_TYPE(1u32),
                );

                if has_message.as_bool() {
                    match msg.message {
                        WM_QUIT => {
                            println!("WM_QUIT received, exiting");
                            break;
                        }
                        _ => {
                            TranslateMessage(&msg);
                            DispatchMessageW(&msg);
                        }
                    }
                } else {
                    // No message available, sleep briefly to avoid busy waiting
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
            }

            UnhookWindowsHookEx(hook)?;
        }

        Ok(())
    }
}

/// Returns whether `normalized_vk` is a modifier used by any active `ModifierCombo`
/// hotkey -- i.e. whether its keydown/keyup should go through the central bookkeeping
/// path (`handle_combo_modifier_keydown`/`resolve_combo_modifier_keyup`) instead of
/// `HotkeyState::handle`, to keep `MODIFIER_STATE` up to date.
fn is_combo_modifier(normalized_vk: u32) -> bool {
    COMBO_MODIFIER_VKS
        .get()
        .map(|set| set.contains(&normalized_vk))
        .unwrap_or(false)
}

/// Returns whether `normalized_vk` needs the swallow-and-replay treatment at all, as
/// opposed to plain state tracking. Only `Alt` gets it: per Win32, holding Alt (or
/// pressing F10, never usable as a `ModifierCombo` modifier here) is what routes the
/// *next* key through `WM_SYSKEYDOWN`/menu-tracking handling and can leave the foreground
/// window in a state that swallows a simulated Ctrl+C -- see the `KeyboardHook` doc
/// comment for the full history. `Ctrl`/`Shift`/`Win` have no such side effect (confirmed
/// by comparison with QTranslate, which simulates Ctrl+C with zero interception and has
/// no equivalent bug reports), so swallowing them too would only add risk -- extra
/// `SendInput` replays, extra state-machine edges -- for a problem they don't have.
fn needs_swallow_and_replay(normalized_vk: u32) -> bool {
    normalized_vk == super::keycodes::KEY_ALT
}

/// Central bookkeeping for a combo-modifier keydown. Always updates `MODIFIER_STATE` (so
/// `HotkeyState::handle`'s `ModifierCombo` check sees it as pressed). For `Alt`
/// specifically (see `needs_swallow_and_replay`), also starts swallowing it if this is a
/// fresh press. Returns true if the event should be blocked from reaching the foreground
/// app.
///
/// `normalized_vk` is used as the tracking key (matching `HotkeyParser`'s normalized
/// modifier codes); `raw_vk` is the specific L/R vk code actually observed, kept around
/// so any later replay uses the same physical key.
fn handle_combo_modifier_keydown(normalized_vk: u32, raw_vk: u32) -> bool {
    let was_down = MODIFIER_STATE
        .get()
        .and_then(|m| {
            m.lock()
                .ok()
                .map(|s| s.get(&normalized_vk).copied().unwrap_or(false))
        })
        .unwrap_or(false);

    if let Some(modifier_state) = MODIFIER_STATE.get() {
        if let Ok(mut state) = modifier_state.lock() {
            state.insert(normalized_vk, true);
        }
    }

    if !needs_swallow_and_replay(normalized_vk) {
        // Ctrl/Shift/Win: tracked above for combo completion, never blocked.
        return false;
    }

    if !was_down {
        // Fresh press: start swallowing it until we learn whether it's part of our
        // combo (trigger key follows), some other shortcut (Tab, F4, ...), or a bare tap
        // (released alone) -- resolved in resolve_combo_modifier_keyup / the non-modifier
        // keydown branch of keyboard_hook_proc.
        if let Some(replay) = MODIFIER_REPLAY.get() {
            if let Ok(mut map) = replay.lock() {
                map.insert(
                    normalized_vk,
                    ModifierReplay {
                        state: ModifierReplayState::Swallowed,
                        raw_vk,
                    },
                );
            }
        }
        return true;
    }

    // Auto-repeat while held: keep blocking unless this hold has already been replayed
    // (in which case the repeats should flow through normally too, like real input would).
    MODIFIER_REPLAY
        .get()
        .and_then(|r| {
            r.lock().ok().map(|map| {
                !matches!(
                    map.get(&normalized_vk).map(|e| e.state),
                    Some(ModifierReplayState::PassedThrough)
                )
            })
        })
        .unwrap_or(true)
}

/// Mark every one of `modifiers` that's currently `Swallowed` as `ConsumedByCombo`,
/// called right after a `ModifierCombo` fires so their eventual real keyup is blocked too
/// instead of being replayed as a bare tap.
fn mark_swallowed_modifiers_consumed_by_combo(modifiers: &[u32]) {
    if let Some(replay) = MODIFIER_REPLAY.get() {
        if let Ok(mut map) = replay.lock() {
            for m in modifiers {
                if let Some(entry) = map.get_mut(m) {
                    if entry.state == ModifierReplayState::Swallowed {
                        entry.state = ModifierReplayState::ConsumedByCombo;
                    }
                }
            }
        }
    }
}

/// Drains every currently-`Swallowed` modifier's raw vk code, transitioning each to
/// `PassedThrough`. Called when some other, non-trigger key arrives while a modifier is
/// held, so the caller can replay the modifier's keydown before letting that other key
/// through -- exactly the order the OS/foreground app would see without the hook in the
/// way. Split from the actual `SendInput` call (see `replay_pending_swallowed_modifiers`)
/// so the pure state transition can be unit-tested.
fn take_pending_replay_vks() -> Vec<u32> {
    let mut result = Vec::new();
    if let Some(replay) = MODIFIER_REPLAY.get() {
        if let Ok(mut map) = replay.lock() {
            for entry in map.values_mut() {
                if entry.state == ModifierReplayState::Swallowed {
                    result.push(entry.raw_vk);
                    entry.state = ModifierReplayState::PassedThrough;
                }
            }
        }
    }
    result
}

/// Replays a synthetic keydown (no matching keyup -- the real one is let through later to
/// balance it) for every currently-swallowed modifier. See `take_pending_replay_vks`.
unsafe fn replay_pending_swallowed_modifiers() {
    for raw_vk in take_pending_replay_vks() {
        replay_keydown_only(raw_vk);
    }
}

/// Central bookkeeping for a combo-modifier keyup. Always updates `MODIFIER_STATE`; for
/// `Alt` (see `needs_swallow_and_replay`) also resolves whatever `ModifierReplay` state
/// this hold ended up in, without performing the actual `SendInput` replay -- see
/// `apply_combo_modifier_keyup_action` for that, kept separate so this decision can be
/// unit-tested without side effects on the real system. Ctrl/Shift/Win always resolve to
/// `PassThrough` since they were never blocked in the first place.
fn resolve_combo_modifier_keyup(normalized_vk: u32) -> ModifierKeyupAction {
    if let Some(modifier_state) = MODIFIER_STATE.get() {
        if let Ok(mut state) = modifier_state.lock() {
            state.insert(normalized_vk, false);
        }
    }

    if !needs_swallow_and_replay(normalized_vk) {
        return ModifierKeyupAction::PassThrough;
    }

    let resolved = MODIFIER_REPLAY
        .get()
        .and_then(|r| r.lock().ok().and_then(|mut map| map.remove(&normalized_vk)));

    match resolved {
        Some(entry) if entry.state == ModifierReplayState::Swallowed => {
            // Bare tap: nothing else happened between down and up. Replay the whole pair
            // now so Alt-Tab/menu activation/etc. still see a normal, complete press.
            ModifierKeyupAction::ReplayPair {
                raw_vk: entry.raw_vk,
            }
        }
        Some(entry) if entry.state == ModifierReplayState::ConsumedByCombo => {
            ModifierKeyupAction::Block
        }
        // PassedThrough (already replayed a down earlier in this hold) or nothing tracked
        // at all -- either way, let the real keyup through to balance things.
        _ => ModifierKeyupAction::PassThrough,
    }
}

/// Executes `action`'s `SendInput` side effect (if any) and reports whether the real
/// keyup should be blocked.
unsafe fn apply_combo_modifier_keyup_action(action: ModifierKeyupAction) -> bool {
    match action {
        ModifierKeyupAction::PassThrough => false,
        ModifierKeyupAction::Block => true,
        ModifierKeyupAction::ReplayPair { raw_vk } => {
            replay_key(raw_vk);
            true
        }
    }
}

/// Injects a synthetic keydown+keyup pair for `vk` via `SendInput`.
unsafe fn replay_key(vk: u32) {
    let inputs = [
        create_key_input(vk as u16, false),
        create_key_input(vk as u16, true),
    ];
    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

/// Injects a synthetic keydown only (no matching keyup) for `vk` via `SendInput`. Used
/// when some other key arrives while `vk` is swallowed and still physically held; the
/// real keyup is let through later to balance this synthetic down (see
/// `ModifierReplayState::PassedThrough`).
unsafe fn replay_keydown_only(vk: u32) {
    let inputs = [create_key_input(vk as u16, false)];
    SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
}

/// Builds a `KEYBDINPUT`/`INPUT` for `SendInput`. Uses `KEYEVENTF_SCANCODE` (scan code
/// from `MapVirtualKeyW`) instead of a bare `wVk` event, mirroring
/// `ClipboardManager::create_key_input` -- this routes the injected event through the
/// same scan-code-to-virtual-key translation path real hardware keystrokes take, which
/// some apps require to act on simulated input at all. `KEYEVENTF_EXTENDEDKEY` is added
/// for the AT-101 extended set (here: Right Ctrl/Alt and the Windows keys).
unsafe fn create_key_input(vk_code: u16, is_keyup: bool) -> INPUT {
    let scan_code = MapVirtualKeyW(vk_code as u32, MAPVK_VK_TO_VSC) as u16;

    let is_extended = matches!(
        vk_code,
        v if v == VK_RCONTROL.0 || v == VK_RMENU.0 || v == VK_LWIN.0 || v == VK_RWIN.0
    );

    let mut flags = KEYEVENTF_SCANCODE;
    if is_keyup {
        flags |= KEYEVENTF_KEYUP;
    }
    if is_extended {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }

    let ki = KEYBDINPUT {
        wVk: VIRTUAL_KEY(vk_code),
        wScan: scan_code,
        dwFlags: flags,
        dwExtraInfo: GetMessageExtraInfo().0 as usize,
        ..Default::default()
    };

    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: windows::Win32::UI::Input::KeyboardAndMouse::INPUT_0 { ki },
    }
}

/// Trigger translation in a separate thread
unsafe fn trigger_translation() {
    if let Some(is_processing) = IS_PROCESSING.get() {
        if let Ok(mut processing) = is_processing.lock() {
            if *processing {
                return;
            }
            *processing = true;
        }
    }

    if let Some(translator) = TRANSLATOR.get() {
        let translator_clone = translator.clone();
        let processing_clone = IS_PROCESSING.get().unwrap().clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = translator_clone.translate_clipboard().await {
                    eprintln!("Translation error: {}", e);
                }
                if let Ok(mut proc) = processing_clone.lock() {
                    *proc = false;
                }
            });
        });
    }
}

/// Trigger text-to-speech in a separate thread
unsafe fn trigger_speech() {
    // Check if speech is enabled
    if let Some(speech_enabled) = SPEECH_ENABLED.get() {
        if !speech_enabled.load(Ordering::Relaxed) {
            println!("Text-to-speech is disabled in configuration");
            return;
        }
    }

    if let Some(is_speaking) = IS_SPEAKING.get() {
        if let Ok(mut speaking) = is_speaking.lock() {
            if *speaking {
                println!("Already speaking, ignoring request");
                return;
            }
            *speaking = true;
        }
    }

    // Reset stop flag
    if let Some(stop_flag) = SHOULD_STOP_SPEECH.get() {
        stop_flag.store(false, Ordering::Relaxed);
    }

    if let Some(translator) = TRANSLATOR.get() {
        let translator_clone = translator.clone();
        let speaking_clone = IS_SPEAKING.get().unwrap().clone();
        let stop_flag_clone = SHOULD_STOP_SPEECH.get().unwrap().clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                if let Err(e) = speak_clipboard(&translator_clone, stop_flag_clone).await {
                    eprintln!("Speech error: {}", e);
                }
                if let Ok(mut speaking) = speaking_clone.lock() {
                    *speaking = false;
                }
            });
        });
    }
}

/// Speak text from clipboard
async fn speak_clipboard(
    _translator: &Translator,
    stop_flag: Arc<AtomicBool>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    use crate::platform::ClipboardManager;
    use crate::platform::WindowManager;
    use std::io::{self, Write};

    let clipboard = ClipboardManager::new();
    clipboard.copy_selected_text()?;
    let text = clipboard.get_text()?;

    // Get config from shared ConfigManager
    let config_manager = CONFIG_MANAGER
        .get()
        .ok_or("ConfigManager not initialized")?;
    if let Err(e) = config_manager.check_and_reload() {
        eprintln!("Config reload error: {}", e);
    }
    let config = config_manager.get_config();

    if text.trim().is_empty() {
        print!("\r");
        io::stdout().flush().ok();
        println!("Clipboard is empty, nothing to speak");
        println!();
        print_source_prompt(&config);
        return Ok(());
    }

    // Show terminal window if configured
    if config.show_terminal_on_translate {
        if let Ok(window_manager) = WindowManager::new() {
            let _ = window_manager.show_terminal();
        }
    }

    // Detect language
    let (source_code, _) = config_manager.get_language_codes();
    let speech_manager = SpeechManager::new();
    let provider = tagent::providers::create_provider(&config.translate_provider)?;
    let lang_code =
        tagent::providers::resolve_source_language(provider.as_ref(), &text, &source_code).await;

    // Clear any existing prompt and print speech info
    print!("\r");
    io::stdout().flush().ok();

    // Show speech label and speak
    SpeechManager::print_speech_label(&text, Some(&config.target_prompt_color));

    if let Err(e) = speech_manager
        .speak_text_with_cancel(provider.as_ref(), &text, &lang_code, stop_flag)
        .await
    {
        eprintln!("Speech error: {}", e);
    }

    // Show source language prompt after speech completes
    println!();
    print_source_prompt(&config);
    Ok(())
}

/// Print source language prompt with color
fn print_source_prompt(cfg: &crate::config::Config) {
    use std::io::{self, Write};

    let source_prompt = format!("[{}]: ", cfg.source_language);
    config::print_colored(&source_prompt, &cfg.source_prompt_color);
    io::stdout().flush().ok();
}

unsafe extern "system" fn keyboard_hook_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let kbd_struct = *(l_param.0 as *const KBDLLHOOKSTRUCT);

        // Ignore injected events (simulated by keybd_event, SendInput, etc.) -- this is
        // what makes the synthetic replays below safe: they re-enter this same hook, and
        // must fall straight through instead of being swallowed again.
        const LLKHF_INJECTED: u32 = 0x10;
        if (kbd_struct.flags.0 & LLKHF_INJECTED) != 0 {
            return CallNextHookEx(HHOOK::default(), n_code, w_param, l_param);
        }

        let vk = kbd_struct.vkCode;
        let normalized = normalize_vk_code(vk);

        if w_param.0 as u32 == WM_KEYDOWN || w_param.0 as u32 == WM_SYSKEYDOWN {
            // Mark double-press sequences as interrupted if another key was pressed
            if let Some(state) = TRANSLATE_HOTKEY.get() {
                state.mark_interrupted_if_needed(vk);
            }
            if let Some(state) = SPEECH_HOTKEY.get() {
                state.mark_interrupted_if_needed(vk);
            }

            // Handle Esc key to stop speech
            if vk == VK_ESCAPE.0 as u32 {
                if let Some(is_speaking) = IS_SPEAKING.get() {
                    if let Ok(speaking) = is_speaking.lock() {
                        if *speaking {
                            if let Some(stop_flag) = SHOULD_STOP_SPEECH.get() {
                                stop_flag.store(true, Ordering::Relaxed);
                                println!("Speech cancelled by user (Esc)");
                            }
                            return LRESULT(1);
                        }
                    }
                }
            }

            if is_combo_modifier(normalized) {
                // Swallow-and-replay: block this modifier's keydown until we learn what
                // follows it (see handle_combo_modifier_keydown's doc comment).
                if handle_combo_modifier_keydown(normalized, vk) {
                    return LRESULT(1);
                }
                // Already PassedThrough for this hold (a repeat after replay) -- fall
                // through to CallNextHookEx below like real input would.
            } else {
                let mut combo_fired = false;
                let mut fired_modifiers: Option<Vec<u32>> = None;

                if let Some(state) = TRANSLATE_HOTKEY.get() {
                    if state.handle(vk, true, trigger_translation) {
                        combo_fired = true;
                        fired_modifiers = state.combo_modifiers();
                    }
                }

                if !combo_fired {
                    if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
                        if speech_enabled.load(Ordering::Relaxed) {
                            if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                                if tts_enabled.load(Ordering::Relaxed) {
                                    if let Some(state) = SPEECH_HOTKEY.get() {
                                        if state.handle(vk, true, trigger_speech) {
                                            combo_fired = true;
                                            fired_modifiers = state.combo_modifiers();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                if combo_fired {
                    if let Some(modifiers) = fired_modifiers {
                        mark_swallowed_modifiers_consumed_by_combo(&modifiers);
                    }
                    return LRESULT(1);
                }

                // Not our trigger key: if any modifier is still swallowed, replay it now
                // so the OS/foreground app see it go down before this key, exactly as
                // they would without the hook in the way (e.g. Alt then Tab).
                replay_pending_swallowed_modifiers();
            }
        } else if w_param.0 as u32 == WM_KEYUP || w_param.0 as u32 == WM_SYSKEYUP {
            if is_combo_modifier(normalized) {
                let action = resolve_combo_modifier_keyup(normalized);
                if apply_combo_modifier_keyup_action(action) {
                    return LRESULT(1);
                }
            } else {
                // Handle translation hotkey key up (for DoublePress repeat tracking)
                if let Some(state) = TRANSLATE_HOTKEY.get() {
                    state.handle(vk, false, trigger_translation);
                }

                // Handle speech hotkey key up (for DoublePress repeat tracking)
                if let Some(speech_enabled) = SPEECH_HOTKEY_ENABLED.get() {
                    if speech_enabled.load(Ordering::Relaxed) {
                        if let Some(tts_enabled) = SPEECH_ENABLED.get() {
                            if tts_enabled.load(Ordering::Relaxed) {
                                if let Some(state) = SPEECH_HOTKEY.get() {
                                    state.handle(vk, false, trigger_speech);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    CallNextHookEx(HHOOK::default(), n_code, w_param, l_param)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

    // NOTE: written but unverified locally -- this module only compiles under
    // `#[cfg(target_os = "windows")]`, so it cannot be run on the Linux dev machine.
    // Needs a run on Windows CI/hardware before this test is trusted.

    static TEST_TRIGGERED: AtomicBool = AtomicBool::new(false);

    // TEST_TRIGGERED, MODIFIER_STATE and MODIFIER_REPLAY are process-global, and Rust
    // runs tests in the same binary on parallel threads by default, so every test in this
    // module must serialize on this lock before touching any of them.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe fn test_trigger_fn() {
        TEST_TRIGGERED.store(true, AtomicOrdering::SeqCst);
    }

    /// Clears MODIFIER_STATE/MODIFIER_REPLAY (initializing them if this is the first test
    /// to run) so tests don't see leftover state from each other.
    fn reset_modifier_globals() {
        let modifier_state = MODIFIER_STATE.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_state.lock().unwrap().clear();
        let modifier_replay = MODIFIER_REPLAY.get_or_init(|| Arc::new(Mutex::new(HashMap::new())));
        modifier_replay.lock().unwrap().clear();
    }

    #[test]
    fn test_hotkey_state_triggers_on_lr_specific_modifier() {
        let _guard = TEST_LOCK.lock().unwrap();
        // Regression test for a parsed "LAlt+Q" config: MODIFIER_STATE is tracked keyed by
        // the normalized code (see handle_combo_modifier_keydown), so HotkeyParser::parse
        // must normalize the configured modifier too, or this never triggers.
        reset_modifier_globals();
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let hotkey = HotkeyParser::parse("LAlt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        let lalt = super::super::keycodes::KEY_LALT;
        let normalized = normalize_vk_code(lalt);

        // Physical left Alt down goes through the central swallow path first in the real
        // hook, keyed by the normalized code.
        handle_combo_modifier_keydown(normalized, lalt);
        assert!(!TEST_TRIGGERED.load(AtomicOrdering::SeqCst));

        // Q down while left Alt held - combo should trigger.
        let triggered = state.handle('Q' as u32, true, test_trigger_fn);
        assert!(triggered);
        assert!(TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }

    #[test]
    fn test_fresh_modifier_press_is_swallowed() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();

        let alt = super::super::keycodes::KEY_ALT;
        let blocked = handle_combo_modifier_keydown(alt, alt);
        assert!(
            blocked,
            "a fresh combo-modifier press must be swallowed until resolved"
        );
    }

    #[test]
    fn test_modifier_auto_repeat_stays_swallowed_until_resolved() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();

        let alt = super::super::keycodes::KEY_ALT;
        handle_combo_modifier_keydown(alt, alt);
        // OS-generated auto-repeat keydown while still held and unresolved.
        let still_blocked = handle_combo_modifier_keydown(alt, alt);
        assert!(still_blocked);
    }

    #[test]
    fn test_bare_modifier_tap_replays_pair_and_blocks_real_keyup() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();

        let alt = super::super::keycodes::KEY_ALT;
        handle_combo_modifier_keydown(alt, alt);

        // Nothing else happened before release: this must replay a full down+up pair.
        let action = resolve_combo_modifier_keyup(alt);
        assert_eq!(action, ModifierKeyupAction::ReplayPair { raw_vk: alt });
    }

    #[test]
    fn test_combo_completion_still_blocks_target_key_and_consumes_modifier() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let alt = super::super::keycodes::KEY_ALT;
        handle_combo_modifier_keydown(alt, alt);

        let hotkey = HotkeyParser::parse("Alt+Q").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        let blocked = state.handle('Q' as u32, true, test_trigger_fn);
        assert!(
            blocked,
            "the target key must still be blocked once the combo fires"
        );
        assert!(TEST_TRIGGERED.load(AtomicOrdering::SeqCst));

        mark_swallowed_modifiers_consumed_by_combo(&state.combo_modifiers().unwrap());

        // The modifier's matching keyup must also be blocked now (no replay -- it was
        // legitimately used by the combo, not a bare tap).
        let action = resolve_combo_modifier_keyup(alt);
        assert_eq!(action, ModifierKeyupAction::Block);
    }

    #[test]
    fn test_other_key_while_modifier_held_replays_before_passthrough() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();

        let alt = super::super::keycodes::KEY_ALT;
        handle_combo_modifier_keydown(alt, alt);

        // Some other key (e.g. Tab) arrives -- not our combo's trigger, so the pending
        // Alt press must be replayed before it, exactly like Alt-Tab would expect.
        let pending = take_pending_replay_vks();
        assert_eq!(pending, vec![alt]);

        // A second, unrelated key while still held must not re-trigger a replay.
        let pending_again = take_pending_replay_vks();
        assert!(pending_again.is_empty());

        // Once replayed, the real keyup must be let through normally to balance it.
        let action = resolve_combo_modifier_keyup(alt);
        assert_eq!(action, ModifierKeyupAction::PassThrough);
    }

    #[test]
    fn test_is_combo_modifier_reflects_registered_set() {
        let _guard = TEST_LOCK.lock().unwrap();
        let _ = COMBO_MODIFIER_VKS.set(HashSet::from([super::super::keycodes::KEY_ALT]));

        assert!(is_combo_modifier(super::super::keycodes::KEY_ALT));
        assert!(!is_combo_modifier(super::super::keycodes::KEY_CONTROL));
    }

    // Regression tests for the Alt-only scoping of swallow-and-replay: Ctrl/Shift/Win
    // don't share Alt's Win32 menu-tracking side effect, so a combo using them (e.g.
    // Ctrl+Shift+T) must never have its modifiers blocked -- only tracked, exactly like
    // before this mechanism existed.

    #[test]
    fn test_needs_swallow_and_replay_is_true_only_for_alt() {
        assert!(needs_swallow_and_replay(super::super::keycodes::KEY_ALT));
        assert!(!needs_swallow_and_replay(
            super::super::keycodes::KEY_CONTROL
        ));
        assert!(!needs_swallow_and_replay(super::super::keycodes::KEY_SHIFT));
        assert!(!needs_swallow_and_replay(super::super::keycodes::KEY_LWIN));
    }

    #[test]
    fn test_non_alt_modifier_keydown_is_never_blocked() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();

        let ctrl = super::super::keycodes::KEY_CONTROL;
        let fresh_press_blocked = handle_combo_modifier_keydown(ctrl, ctrl);
        assert!(
            !fresh_press_blocked,
            "Ctrl has no menu-tracking side effect and must pass through unblocked"
        );
        // Auto-repeat while held must also stay unblocked.
        let repeat_blocked = handle_combo_modifier_keydown(ctrl, ctrl);
        assert!(!repeat_blocked);

        // State tracking must still work, so a Ctrl+Shift+T-style combo can complete.
        let is_down = MODIFIER_STATE
            .get()
            .unwrap()
            .lock()
            .unwrap()
            .get(&ctrl)
            .copied();
        assert_eq!(is_down, Some(true));
    }

    #[test]
    fn test_non_alt_modifier_keyup_is_always_passthrough() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();

        let shift = super::super::keycodes::KEY_SHIFT;
        handle_combo_modifier_keydown(shift, shift);

        let action = resolve_combo_modifier_keyup(shift);
        assert_eq!(
            action,
            ModifierKeyupAction::PassThrough,
            "a bare Shift tap must never trigger a synthetic replay"
        );

        // Nothing should have been left behind in MODIFIER_REPLAY for a non-Alt modifier.
        let replay = MODIFIER_REPLAY.get().unwrap().lock().unwrap();
        assert!(!replay.contains_key(&shift));
    }

    #[test]
    fn test_multi_modifier_combo_with_non_alt_modifiers_still_fires() {
        let _guard = TEST_LOCK.lock().unwrap();
        reset_modifier_globals();
        TEST_TRIGGERED.store(false, AtomicOrdering::SeqCst);

        let ctrl = super::super::keycodes::KEY_CONTROL;
        let shift = super::super::keycodes::KEY_SHIFT;
        handle_combo_modifier_keydown(ctrl, ctrl);
        handle_combo_modifier_keydown(shift, shift);

        let hotkey = HotkeyParser::parse("Ctrl+Shift+T").unwrap();
        let state = HotkeyState::new(Some(hotkey));

        let triggered = state.handle('T' as u32, true, test_trigger_fn);
        assert!(
            triggered,
            "Ctrl+Shift+T must still fire via plain state tracking"
        );
        assert!(TEST_TRIGGERED.load(AtomicOrdering::SeqCst));
    }
}
