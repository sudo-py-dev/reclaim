//! Automatic layout checks for headless Slint UI tests.
//!
//! Screenshots catch what a person notices; these catch it without a person.
//! Call them for every page, state, language (at least English, Hebrew and the
//! longest one) and window size (minimum and default) the render test visits:
//!
//! - [`assert_controls_clear`]: every visible control lies inside the window
//!   and no two controls overlap (a notice covering a card, a button pushed
//!   off-screen by long German text, a mirrored element landing on another).
//! - [`assert_steady`]: an element keeps its size and position across states
//!   (a status strip that grows when its text wraps, a header that shifts).
//!
//! Copy into `src/` of the GUI crate as a `#[cfg(test)]` module. Requires the
//! dev-dependency `i-slint-backend-testing` (same exact version as `slint`).

use i_slint_backend_testing::{AccessibleRole, ElementHandle, ElementRoot};

/// Sub-pixel slack for anti-aliased edges and rounding.
const SLACK: f32 = 0.5;

/// Roles a user clicks, types into or focuses.
const CONTROLS: [AccessibleRole; 9] = [
    AccessibleRole::Button,
    AccessibleRole::Checkbox,
    AccessibleRole::Combobox,
    AccessibleRole::TextInput,
    AccessibleRole::Slider,
    AccessibleRole::Spinbox,
    AccessibleRole::Switch,
    AccessibleRole::RadioButton,
    AccessibleRole::Tab,
];

/// An element's box in window coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Bounds {
    pub fn of(element: &ElementHandle) -> Self {
        let position = element.absolute_position();
        let size = element.size();
        Self {
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
        }
    }
    fn right(&self) -> f32 {
        self.x + self.width
    }
    fn bottom(&self) -> f32 {
        self.y + self.height
    }
    fn is_empty(&self) -> bool {
        self.width <= SLACK || self.height <= SLACK
    }
    fn overlaps(&self, other: &Self) -> bool {
        self.x < other.right() - SLACK
            && other.x < self.right() - SLACK
            && self.y < other.bottom() - SLACK
            && other.y < self.bottom() - SLACK
    }
    fn intersect(&self, other: &Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        Self {
            x,
            y,
            width: (self.right().min(other.right()) - x).max(0.0),
            height: (self.bottom().min(other.bottom()) - y).max(0.0),
        }
    }
    fn contains(&self, other: &Self) -> bool {
        other.x >= self.x - SLACK
            && other.y >= self.y - SLACK
            && other.right() <= self.right() + SLACK
            && other.bottom() <= self.bottom() + SLACK
    }
}

/// Every visible control is inside the window, and no two controls overlap.
///
/// Controls inside a scrollable area are clipped to its viewport first, as the
/// renderer does, so a row scrolled under a header or a button cut off at the
/// edge of a scroll view is judged by the part a user can see.
pub fn assert_controls_clear(ui: &impl ElementRoot, window: (f32, f32), context: &str) {
    let screen = Bounds {
        x: 0.0,
        y: 0.0,
        width: window.0,
        height: window.1,
    };
    // Visible box of every control: its own box, cut to each scroll viewport
    // that contains it (nested viewports intersect).
    let mut controls: Vec<(AccessibleRole, String, Bounds, Bounds)> = Vec::new();
    for role in CONTROLS {
        for element in ui
            .root_element()
            .query_descendants()
            .match_accessible_role(role)
            .find_all()
        {
            let bounds = Bounds::of(&element);
            let name = element.accessible_label().unwrap_or_default().to_string();
            controls.push((role, name, bounds, bounds));
        }
    }
    for viewport in ui
        .root_element()
        .query_descendants()
        .match_inherits("Flickable")
        .find_all()
    {
        let clip = Bounds::of(&viewport);
        for role in CONTROLS {
            for inner in viewport
                .query_descendants()
                .match_accessible_role(role)
                .find_all()
            {
                let bounds = Bounds::of(&inner);
                let name = inner.accessible_label().unwrap_or_default().to_string();
                for control in &mut controls {
                    if control.0 == role && control.1 == name && control.2 == bounds {
                        control.3 = control.3.intersect(&clip);
                    }
                }
            }
        }
    }
    let visible: Vec<_> = controls
        .into_iter()
        .filter(|(_, _, _, shown)| !shown.is_empty())
        .collect();
    for (role, name, _, shown) in &visible {
        assert!(
            screen.contains(shown),
            "{context}: {role:?} {name:?} at {shown:?} leaves the window {window:?}"
        );
    }
    for (i, (role_a, name_a, _, a)) in visible.iter().enumerate() {
        for (role_b, name_b, _, b) in visible.iter().skip(i + 1) {
            // A control drawn inside another (a checkbox inside a list row) is nesting, not overlap.
            if a.contains(b) || b.contains(a) {
                continue;
            }
            assert!(
                !a.overlaps(b),
                "{context}: {role_a:?} {name_a:?} {a:?} overlaps {role_b:?} {name_b:?} {b:?}"
            );
        }
    }
}

/// One state for [`assert_steady`]: a name for messages and the change that puts
/// the UI into that state.
pub type State<'a, R> = (&'a str, &'a mut dyn FnMut(&R));

/// The element with qualified id `id` (such as `"ScanStatus::headline"`) has
/// the same size and position in every state `states` puts the UI into.
pub fn assert_steady<R: ElementRoot>(ui: &R, id: &str, context: &str, states: &mut [State<'_, R>]) {
    let mut first: Option<(String, Bounds)> = None;
    for (name, apply) in states.iter_mut() {
        apply(ui);
        let element = ElementHandle::find_by_element_id(ui, id)
            .next()
            .unwrap_or_else(|| panic!("{context}: no element {id} in state {name}"));
        let bounds = Bounds::of(&element);
        match &first {
            None => first = Some(((*name).to_owned(), bounds)),
            Some((first_name, expected)) => assert!(
                (bounds.x - expected.x).abs() <= SLACK
                    && (bounds.y - expected.y).abs() <= SLACK
                    && (bounds.width - expected.width).abs() <= SLACK
                    && (bounds.height - expected.height).abs() <= SLACK,
                "{context}: {id} moved or resized from {expected:?} in {first_name} to {bounds:?} in {name}"
            ),
        }
    }
}
