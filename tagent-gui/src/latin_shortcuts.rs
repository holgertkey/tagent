//! Ctrl+C/V/X/A/Z in any keyboard layout.
//!
//! Slint recognizes the standard editing shortcuts by the character a key produces in the
//! active layout (`"v"` for Paste), so with a Russian, Greek, Hebrew, ... layout Ctrl+V
//! arrives as Ctrl+`"м"` and nothing happens. Other toolkits fall back to the Latin
//! letter (Qt and GTK through another xkb group, Chromium through the US position, Win32
//! through the virtual-key code); Slint has no such fallback.
//!
//! [`install`] adds it: a winit event filter that, while the shortcut modifier is held and
//! the key produces a non-ASCII character, swallows the key and hands Slint the letter of
//! the same physical key on a US keyboard instead. Keys that already produce ASCII (any
//! Latin layout, Dvorak included) pass through untouched, so only non-Latin layouts are
//! affected. The decision is the pure [`fallback_letter`].

use slint::winit_030::winit::event::{ElementState, WindowEvent};
use slint::winit_030::winit::keyboard::{Key, KeyCode, ModifiersState, PhysicalKey};
use slint::winit_030::{EventResult, WinitWindowAccessor};
use std::cell::Cell;

/// Installs the fallback on `window` (a no-op for a window not backed by winit).
pub fn install(window: &slint::Window) {
    let modifiers = Cell::new(ModifiersState::empty());
    // The physical key whose press was remapped, so its release is remapped too.
    let remapped: Cell<Option<(KeyCode, &'static str)>> = Cell::new(None);
    window.on_winit_window_event(move |window, event| match event {
        WindowEvent::ModifiersChanged(new) => {
            modifiers.set(new.state());
            EventResult::Propagate
        }
        WindowEvent::KeyboardInput {
            event,
            is_synthetic: false,
            ..
        } => {
            let PhysicalKey::Code(code) = event.physical_key else {
                return EventResult::Propagate;
            };
            let text = match event.state {
                ElementState::Pressed => {
                    let Some(letter) =
                        fallback_letter(&event.logical_key, event.physical_key, modifiers.get())
                    else {
                        return EventResult::Propagate;
                    };
                    remapped.set(Some((code, letter)));
                    let text = letter.into();
                    window.dispatch_event(if event.repeat {
                        slint::platform::WindowEvent::KeyPressRepeated { text }
                    } else {
                        slint::platform::WindowEvent::KeyPressed { text }
                    });
                    return EventResult::PreventDefault;
                }
                ElementState::Released => match remapped.get() {
                    Some((pressed, letter)) if pressed == code => {
                        remapped.set(None);
                        letter
                    }
                    _ => return EventResult::Propagate,
                },
            };
            window.dispatch_event(slint::platform::WindowEvent::KeyReleased { text: text.into() });
            EventResult::PreventDefault
        }
        _ => EventResult::Propagate,
    });
}

/// The Latin letter Slint should see instead of `logical`, if any.
///
/// `Some` only while the shortcut modifier (Ctrl; Cmd on macOS, which Slint treats as its
/// Ctrl) is held without Alt, the key produces a non-ASCII character, and `physical` is
/// a letter key; the result is that key's lowercase letter on a US keyboard.
fn fallback_letter(
    logical: &Key,
    physical: PhysicalKey,
    modifiers: ModifiersState,
) -> Option<&'static str> {
    let shortcut_modifier = if cfg!(target_vendor = "apple") {
        modifiers.super_key()
    } else {
        modifiers.control_key()
    };
    if !shortcut_modifier || modifiers.alt_key() {
        return None;
    }
    let Key::Character(text) = logical else {
        return None;
    };
    if text.is_ascii() {
        return None;
    }
    let PhysicalKey::Code(code) = physical else {
        return None;
    };
    letter_of(code)
}

/// The lowercase letter of a letter key on a US keyboard.
fn letter_of(code: KeyCode) -> Option<&'static str> {
    use KeyCode::*;
    Some(match code {
        KeyA => "a",
        KeyB => "b",
        KeyC => "c",
        KeyD => "d",
        KeyE => "e",
        KeyF => "f",
        KeyG => "g",
        KeyH => "h",
        KeyI => "i",
        KeyJ => "j",
        KeyK => "k",
        KeyL => "l",
        KeyM => "m",
        KeyN => "n",
        KeyO => "o",
        KeyP => "p",
        KeyQ => "q",
        KeyR => "r",
        KeyS => "s",
        KeyT => "t",
        KeyU => "u",
        KeyV => "v",
        KeyW => "w",
        KeyX => "x",
        KeyY => "y",
        KeyZ => "z",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::winit_030::winit::keyboard::{NamedKey, NativeKeyCode};

    fn shortcut() -> ModifiersState {
        if cfg!(target_vendor = "apple") {
            ModifiersState::SUPER
        } else {
            ModifiersState::CONTROL
        }
    }

    fn char_key(text: &str) -> Key {
        Key::Character(text.into())
    }

    #[test]
    fn non_latin_letter_with_the_shortcut_modifier_becomes_the_us_letter() {
        let v = PhysicalKey::Code(KeyCode::KeyV);
        assert_eq!(fallback_letter(&char_key("м"), v, shortcut()), Some("v"));
        // Greek and Hebrew layouts on the same key.
        assert_eq!(fallback_letter(&char_key("ω"), v, shortcut()), Some("v"));
        assert_eq!(fallback_letter(&char_key("ה"), v, shortcut()), Some("v"));
        let c = PhysicalKey::Code(KeyCode::KeyC);
        assert_eq!(fallback_letter(&char_key("с"), c, shortcut()), Some("c"));
    }

    #[test]
    fn shift_is_kept_for_redo() {
        let z = PhysicalKey::Code(KeyCode::KeyZ);
        assert_eq!(
            fallback_letter(&char_key("Я"), z, shortcut() | ModifiersState::SHIFT),
            Some("z")
        );
    }

    #[test]
    fn latin_layouts_pass_through() {
        // Dvorak: the physical V key types "k"; Slint must see that, not "v".
        let v = PhysicalKey::Code(KeyCode::KeyV);
        assert_eq!(fallback_letter(&char_key("k"), v, shortcut()), None);
        assert_eq!(fallback_letter(&char_key("v"), v, shortcut()), None);
    }

    #[test]
    fn without_the_shortcut_modifier_or_with_alt_nothing_changes() {
        let v = PhysicalKey::Code(KeyCode::KeyV);
        assert_eq!(
            fallback_letter(&char_key("м"), v, ModifiersState::empty()),
            None
        );
        assert_eq!(
            fallback_letter(&char_key("м"), v, ModifiersState::SHIFT),
            None
        );
        assert_eq!(
            fallback_letter(&char_key("м"), v, shortcut() | ModifiersState::ALT),
            None
        );
    }

    #[test]
    fn only_letter_keys_with_characters_are_remapped() {
        assert_eq!(
            fallback_letter(
                &char_key("ж"),
                PhysicalKey::Code(KeyCode::Semicolon),
                shortcut()
            ),
            None
        );
        assert_eq!(
            fallback_letter(
                &Key::Named(NamedKey::Enter),
                PhysicalKey::Code(KeyCode::KeyV),
                shortcut()
            ),
            None
        );
        assert_eq!(
            fallback_letter(
                &char_key("м"),
                PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
                shortcut()
            ),
            None
        );
    }
}
