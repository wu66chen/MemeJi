use crate::focus::CapturedFocus;

#[derive(Default)]
pub struct PickerSession {
    generation: u64,
    active: bool,
    copying: bool,
    focus: Option<CapturedFocus>,
}

impl PickerSession {
    pub fn begin(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.active = true;
        self.copying = false;
        self.focus = None;
        self.generation
    }

    pub fn set_focus(&mut self, generation: u64, focus: Option<CapturedFocus>) -> bool {
        if !self.is_current(generation) {
            return false;
        }
        self.focus = focus;
        true
    }

    pub fn is_current(&self, generation: u64) -> bool {
        self.active && self.generation == generation
    }

    pub fn is_copying(&self) -> bool {
        self.active && self.copying
    }

    pub fn is_pending(&self) -> bool {
        self.active && !self.copying
    }

    pub fn begin_copy(&mut self) -> Option<(u64, Option<CapturedFocus>)> {
        if !self.active || self.copying {
            return None;
        }
        self.copying = true;
        Some((self.generation, self.focus.clone()))
    }

    pub fn finish_copy(&mut self, generation: u64, completed: bool) {
        if self.is_current(generation) {
            self.copying = false;
            if completed {
                self.active = false;
                self.focus = None;
            }
        }
    }

    pub fn cancel(&mut self) -> Option<CapturedFocus> {
        self.generation = self.generation.wrapping_add(1);
        self.active = false;
        self.copying = false;
        self.focus.take()
    }

    pub fn cancel_on_blur(&mut self) -> bool {
        if !self.active || self.copying {
            return false;
        }
        self.cancel();
        true
    }
}

#[cfg(test)]
mod tests {
    use super::PickerSession;

    #[test]
    fn cancel_invalidates_the_old_copy_and_a_new_summon_stays_active() {
        let mut session = PickerSession::default();
        let old = session.begin();
        assert!(session.begin_copy().is_some());
        session.cancel();
        let next = session.begin();
        session.finish_copy(old, true);
        assert_ne!(old, next);
        assert!(session.is_current(next));
    }

    #[test]
    fn duplicate_copy_is_rejected_and_blur_cannot_cancel_a_copy() {
        let mut session = PickerSession::default();
        let generation = session.begin();
        assert!(session.begin_copy().is_some());
        assert!(session.begin_copy().is_none());
        assert!(!session.cancel_on_blur());
        assert!(session.is_current(generation));
        session.finish_copy(generation, true);
        assert!(!session.is_current(generation));
    }

    #[test]
    fn blur_dismisses_an_idle_picker() {
        let mut session = PickerSession::default();
        let generation = session.begin();
        assert!(session.cancel_on_blur());
        assert!(!session.is_current(generation));
    }

    #[test]
    fn failed_copy_can_retry_without_losing_the_active_picker() {
        let mut session = PickerSession::default();
        let generation = session.begin();
        assert!(session.begin_copy().is_some());
        session.finish_copy(generation, false);
        assert!(session.is_current(generation));
        assert!(session.begin_copy().is_some());
    }

    #[test]
    fn repeated_shortcut_can_cancel_a_pending_summon() {
        let mut session = PickerSession::default();
        let generation = session.begin();
        assert!(session.is_pending());
        session.cancel();
        assert!(!session.is_current(generation));
        assert!(!session.is_pending());
    }
}
