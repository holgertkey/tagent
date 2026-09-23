//! Slint renderer selection on Windows.
//!
//! Slint's default Windows renderer (femtovg) draws through OpenGL, and with an NVIDIA
//! driver that loads `nvoglv64.dll`, which runs its own thread with hidden top-level
//! windows (`NVOpenGLPbuffer`). A keyboard-layout change that reaches both that thread and
//! our UI thread at the same time deadlocks the process: the UI thread's `DefWindowProc`
//! for `WM_INPUTLANGCHANGEREQUEST` takes the process-wide IMM lock in `ImmActivateLayout`,
//! creates a TSF window whose creation calls back into `nvoglv64`, and the driver waits
//! for its own thread, which is itself blocked on that IMM lock in `ImmSystemHandler`.
//! Layout switchers that broadcast `WM_INPUTLANGCHANGEREQUEST` to every top-level window
//! (`HWND_BROADCAST`) trigger it readily. The software renderer never loads the OpenGL
//! driver, so the second thread doesn't exist.

/// The `SLINT_BACKEND` value Tagent defaults to on Windows.
const DEFAULT_BACKEND: &str = "winit-software";

/// Returns the backend to force, given the current `SLINT_BACKEND` value, or `None`
/// when the user set one explicitly (so they can still opt back into OpenGL).
fn backend_override(slint_backend_env: Option<&str>) -> Option<&'static str> {
    match slint_backend_env {
        Some(value) if !value.trim().is_empty() => None,
        _ => Some(DEFAULT_BACKEND),
    }
}

/// Selects the software renderer unless `SLINT_BACKEND` is set. Must run before the
/// first Slint component is created. A failure is reported and Slint's default is kept.
pub fn select_default_renderer() {
    let env = std::env::var("SLINT_BACKEND").ok();
    let Some(backend) = backend_override(env.as_deref()) else {
        return;
    };
    if let Err(e) = slint::BackendSelector::new()
        .backend_name(backend.to_string())
        .select()
    {
        eprintln!("Warning: could not select the '{backend}' Slint backend: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unset_env_forces_software_renderer() {
        assert_eq!(backend_override(None), Some("winit-software"));
    }

    #[test]
    fn test_blank_env_forces_software_renderer() {
        assert_eq!(backend_override(Some("")), Some("winit-software"));
        assert_eq!(backend_override(Some("  ")), Some("winit-software"));
    }

    #[test]
    fn test_explicit_env_is_respected() {
        assert_eq!(backend_override(Some("winit-femtovg")), None);
        assert_eq!(backend_override(Some("winit")), None);
    }
}
