//! Accessibility helpers for custom clickable `div`s — port of the agenda
//! `A11y` trait: every interactive element gets an AccessKit role + a Russian
//! accessible name, and named element ids are exported as the platform
//! AutomationId / AXIdentifier so test locators stay stable.
use gpui::{
    Div, Element, ElementId, Role, SharedString, Stateful, StatefulInteractiveElement, Toggled,
};

/// `Role::Button` / `Role::MenuItem` / toggle roles + accessible name for
/// `Stateful<Div>` controls. Names must be Russian, matching the
/// user-facing label (or describing the control when it's icon-only).
pub trait A11y: Sized {
    /// `Role::Button` + accessible name.
    fn a11y_button(self, name: impl Into<SharedString>) -> Self;
    /// `Role::MenuItem` + accessible name — rows inside popup menus.
    fn a11y_menu_item(self, name: impl Into<SharedString>) -> Self;
    /// `Role::Switch` + accessible name + on/off state.
    fn a11y_switch(self, name: impl Into<SharedString>, on: bool) -> Self;
}

impl A11y for Stateful<Div> {
    fn a11y_button(self, name: impl Into<SharedString>) -> Self {
        a11y_id(self.role(Role::Button).aria_label(name))
    }
    fn a11y_menu_item(self, name: impl Into<SharedString>) -> Self {
        a11y_id(self.role(Role::MenuItem).aria_label(name))
    }
    fn a11y_switch(self, name: impl Into<SharedString>, on: bool) -> Self {
        a11y_id(
            self.role(Role::Switch)
                .aria_label(name)
                .aria_toggled(toggled(on)),
        )
    }
}

fn toggled(on: bool) -> Toggled {
    if on {
        Toggled::True
    } else {
        Toggled::False
    }
}

/// Export the element id as the platform AutomationId where it carries a
/// name; non-name ids (focus handles, paths) have no stable string form.
fn a11y_id(el: Stateful<Div>) -> Stateful<Div> {
    match Element::id(&el) {
        Some(ElementId::Name(id)) => el.accessibility_id(id),
        Some(ElementId::NamedInteger(id, i)) => {
            el.accessibility_id(SharedString::from(format!("{id}-{i}")))
        }
        _ => el,
    }
}
