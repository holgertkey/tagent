//! Pure geometry helpers for dragging the hotkey popup and restoring a remembered
//! position — no Slint or OS calls here, so they're unit-testable. The
//! drag/restore wiring itself lives in `main.rs` (`wire_popup_drag`, `show_popup`).

/// How far (px, on either axis) the pointer has to travel from where the button
/// went down before it counts as a drag rather than a click. Keeps a slightly
/// shaky click from nudging the popup, and from saving a position nobody chose.
pub const DRAG_THRESHOLD_PX: i32 = 3;

/// Whether the pointer has moved far enough from `start` to count as a drag.
pub fn exceeds_drag_threshold(start: (i32, i32), current: (i32, i32)) -> bool {
    (current.0 - start.0).abs() >= DRAG_THRESHOLD_PX
        || (current.1 - start.1).abs() >= DRAG_THRESHOLD_PX
}

/// Where the popup's top-left corner should be while dragging: where it was when
/// the button went down, shifted by however far the pointer has travelled since.
///
/// Both pointer positions are *global* screen coordinates. Using coordinates
/// relative to the popup itself instead would feed back on itself, since the
/// popup moves under the pointer with every step.
pub fn dragged_position(
    start_position: (i32, i32),
    start_cursor: (i32, i32),
    cursor: (i32, i32),
) -> (i32, i32) {
    (
        start_position.0 + (cursor.0 - start_cursor.0),
        start_position.1 + (cursor.1 - start_cursor.1),
    )
}

/// Moves `position` (a window's top-left corner) the minimum distance needed to
/// bring a window of `size` fully inside `bounds`, all as `(x, y, width, height)`
/// / `(width, height)` in physical pixels.
///
/// A window larger than `bounds` on some axis is pinned to that axis's top/left
/// edge instead, so its start (where the text begins) stays reachable.
pub fn clamp_to_bounds(
    position: (i32, i32),
    size: (i32, i32),
    bounds: (i32, i32, i32, i32),
) -> (i32, i32) {
    let (bounds_x, bounds_y, bounds_width, bounds_height) = bounds;
    // `min` before `max`: when the window is larger than the bounds the upper
    // limit drops below the lower one, and this order makes the lower one win.
    let x = position
        .0
        .min(bounds_x + bounds_width - size.0)
        .max(bounds_x);
    let y = position
        .1
        .min(bounds_y + bounds_height - size.1)
        .max(bounds_y);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCREEN: (i32, i32, i32, i32) = (0, 0, 1920, 1080);

    #[test]
    fn small_pointer_jitter_is_not_a_drag() {
        assert!(!exceeds_drag_threshold((100, 100), (100, 100)));
        assert!(!exceeds_drag_threshold((100, 100), (102, 98)));
    }

    #[test]
    fn travelling_the_threshold_on_either_axis_is_a_drag() {
        assert!(exceeds_drag_threshold((100, 100), (103, 100)));
        assert!(exceeds_drag_threshold((100, 100), (100, 97)));
    }

    #[test]
    fn dragged_position_follows_the_pointer_delta() {
        assert_eq!(
            dragged_position((500, 300), (600, 320), (650, 310)),
            (550, 290)
        );
    }

    #[test]
    fn dragged_position_with_no_travel_is_the_start_position() {
        assert_eq!(
            dragged_position((500, 300), (600, 320), (600, 320)),
            (500, 300)
        );
    }

    #[test]
    fn clamp_leaves_an_on_screen_position_alone() {
        assert_eq!(clamp_to_bounds((100, 200), (300, 100), SCREEN), (100, 200));
    }

    #[test]
    fn clamp_pulls_an_overhanging_window_back_inside() {
        assert_eq!(
            clamp_to_bounds((1800, 1050), (300, 100), SCREEN),
            (1620, 980)
        );
    }

    #[test]
    fn clamp_pulls_a_negative_position_back_to_the_top_left() {
        assert_eq!(clamp_to_bounds((-50, -20), (300, 100), SCREEN), (0, 0));
    }

    #[test]
    fn clamp_handles_bounds_that_start_off_origin() {
        // A second monitor to the left of the primary one: x runs from -1280.
        let bounds = (-1280, 0, 3200, 1080);
        assert_eq!(
            clamp_to_bounds((-1500, 50), (300, 100), bounds),
            (-1280, 50)
        );
        assert_eq!(clamp_to_bounds((-900, 50), (300, 100), bounds), (-900, 50));
    }

    #[test]
    fn clamp_pins_a_window_larger_than_the_bounds_to_the_top_left() {
        assert_eq!(clamp_to_bounds((400, 400), (3000, 2000), SCREEN), (0, 0));
    }
}
