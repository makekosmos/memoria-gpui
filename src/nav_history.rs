//! Port of `useNavigationHistory.ts` — back/forward stacks with a 30-entry
//! cap, first-snapshot seeding and a suppression flag so replaying history
//! does not re-record itself.
//!
//! The Vue composable watches a computed snapshot and records the *previous*
//! value when it changes; here `record(current)` is called explicitly from the
//! app's route setter, which keeps the ordering obvious.

use crate::routes::Route;

/// `MAX_NAVIGATION_HISTORY` in Vue.
pub const MAX_NAVIGATION_HISTORY: usize = 30;

#[derive(Debug, Default)]
pub struct NavHistory {
    back: Vec<Route>,
    forward: Vec<Route>,
    /// Vue `historyReady` — the first snapshot seeds instead of recording.
    ready: bool,
    /// Vue `suppressHistoryRecording` — set while applying back/forward.
    suppress: bool,
    /// Last recorded snapshot (`previousSnapshot` in the Vue watch).
    previous: Option<Route>,
}

fn push_capped(stack: &mut Vec<Route>, route: Route) {
    stack.push(route);
    if stack.len() > MAX_NAVIGATION_HISTORY {
        let excess = stack.len() - MAX_NAVIGATION_HISTORY;
        stack.drain(0..excess);
    }
}

impl NavHistory {
    pub fn new() -> Self {
        Self::default()
    }

    /// First call seeds `previous` without pushing (`historyReady` flip).
    /// Later calls push `previous` onto the back stack and clear forward —
    /// exactly the Vue watch semantics. Calls while `suppress` is set update
    /// `previous` but do not record (mirrors `applyHistorySnapshot`).
    pub fn record(&mut self, current: Route) {
        if !self.ready {
            self.ready = true;
            self.previous = Some(current);
            return;
        }
        if self.suppress {
            self.previous = Some(current);
            return;
        }
        if self.previous.as_ref() == Some(&current) {
            return;
        }
        if let Some(prev) = self.previous.take() {
            push_capped(&mut self.back, prev);
        }
        self.forward.clear();
        self.previous = Some(current);
    }

    /// `suppressHistoryRecording = true` during `applyHistorySnapshot`.
    pub fn set_suppress(&mut self, suppress: bool) {
        self.suppress = suppress;
    }

    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward.is_empty()
    }

    /// `navigateBack` — pops the target and pushes the current route onto the
    /// forward stack. Returns the route to apply (or `None` when empty).
    pub fn navigate_back(&mut self, current: &Route) -> Option<Route> {
        let target = self.back.pop()?;
        push_capped(&mut self.forward, current.clone());
        Some(target)
    }

    /// `navigateForward` — pops the target and pushes the current route onto
    /// the back stack (uncapped push through `push_capped` like Vue).
    pub fn navigate_forward(&mut self, current: &Route) -> Option<Route> {
        let target = self.forward.pop()?;
        push_capped(&mut self.back, current.clone());
        Some(target)
    }

    /// The entry id of the route that back would land on — used to decide
    /// whether a back navigation needs an `Entry` load.
    pub fn peek_back(&self) -> Option<&Route> {
        self.back.last()
    }

    /// Test helpers — stack sizes for assertions.
    pub fn back_len(&self) -> usize {
        self.back.len()
    }
    pub fn forward_len(&self) -> usize {
        self.forward.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(key: &str) -> Route {
        Route::parse_key(key).unwrap()
    }

    #[test]
    fn seeds_then_records() {
        let mut h = NavHistory::new();
        h.record(r("everything"));
        assert!(!h.can_go_back());
        h.record(r("entry:a"));
        assert!(h.can_go_back());
        h.record(r("entry:b"));
        assert_eq!(h.back_len(), 2);
    }

    #[test]
    fn back_forward_roundtrip() {
        let mut h = NavHistory::new();
        h.record(r("everything"));
        h.record(r("entry:a"));
        h.record(r("entry:b"));
        let cur = r("entry:b");
        assert_eq!(h.navigate_back(&cur), Some(r("entry:a")));
        assert!(h.can_go_forward());
        let cur = r("entry:a");
        assert_eq!(h.navigate_back(&cur), Some(r("everything")));
        assert!(!h.can_go_back());
        let cur = r("everything");
        assert_eq!(h.navigate_forward(&cur), Some(r("entry:a")));
        let cur = r("entry:a");
        assert_eq!(h.navigate_forward(&cur), Some(r("entry:b")));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn new_nav_clears_forward() {
        let mut h = NavHistory::new();
        h.record(r("everything"));
        h.record(r("entry:a"));
        h.navigate_back(&r("entry:a"));
        h.record(r("entry:c"));
        assert!(!h.can_go_forward());
    }

    #[test]
    fn suppressed_does_not_record() {
        let mut h = NavHistory::new();
        h.record(r("everything"));
        h.record(r("entry:a"));
        h.set_suppress(true);
        h.record(r("entry:b"));
        h.set_suppress(false);
        // Back stack still holds only `everything`; the suppressed move from
        // `entry:a` → `entry:b` just updated `previous`.
        assert_eq!(h.navigate_back(&r("entry:b")), Some(r("everything")));
    }

    #[test]
    fn caps_at_30() {
        let mut h = NavHistory::new();
        h.record(r("everything"));
        for i in 0..40 {
            h.record(r(&format!("entry:{i}")));
        }
        assert_eq!(h.back_len(), MAX_NAVIGATION_HISTORY);
        // Oldest entries were sliced off the tail side (slice keeps last 30).
        let target = h.navigate_back(&r("entry:39")).unwrap();
        assert_eq!(target, r("entry:38"));
    }

    #[test]
    fn same_route_does_not_duplicate() {
        let mut h = NavHistory::new();
        h.record(r("everything"));
        h.record(r("everything"));
        h.record(r("everything"));
        assert!(!h.can_go_back());
    }
}
