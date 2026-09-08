//! Centralized motion primitives for the TUI.
//!
//! Reduced motion keeps a static activity bullet and plain text instead of
//! time-varying spinner or shimmer effects.

use std::time::Duration;
use std::time::Instant;

use ratatui::style::Stylize;
use ratatui::text::Span;

#[path = "shimmer.rs"]
mod shimmer;

use shimmer::shimmer_spans;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MotionMode {
    Animated,
    Reduced,
}

impl MotionMode {
    pub(crate) fn from_animations_enabled(animations_enabled: bool) -> Self {
        if animations_enabled {
            Self::Animated
        } else {
            Self::Reduced
        }
    }
}

pub(crate) fn activity_indicator(
    start_time: Option<Instant>,
    motion_mode: MotionMode,
) -> Span<'static> {
    match motion_mode {
        MotionMode::Animated => animated_activity_indicator(start_time),
        MotionMode::Reduced => "•".into(),
    }
}

pub(crate) fn shimmer_text(text: &str, motion_mode: MotionMode) -> Vec<Span<'static>> {
    match motion_mode {
        MotionMode::Animated => shimmer_spans(text),
        MotionMode::Reduced => {
            if text.is_empty() {
                Vec::new()
            } else {
                vec![text.to_string().into()]
            }
        }
    }
}

fn animated_activity_indicator(start_time: Option<Instant>) -> Span<'static> {
    let elapsed = start_time.map(|st| st.elapsed()).unwrap_or_default();
    activity_indicator_for_elapsed(elapsed)
}

fn activity_indicator_for_elapsed(elapsed: Duration) -> Span<'static> {
    if (elapsed.as_millis() / 600).is_multiple_of(2) {
        "•".bold()
    } else {
        "•".into()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn reduced_motion_activity_indicator_uses_static_bullet() {
        assert_eq!(
            activity_indicator(/*start_time*/ None, MotionMode::Reduced),
            Span::from("•")
        );
    }

    #[test]
    fn reduced_motion_shimmer_text_is_plain_text() {
        assert_eq!(
            shimmer_text("Loading", MotionMode::Reduced),
            vec!["Loading".into()]
        );
        assert_eq!(
            shimmer_text("", MotionMode::Reduced),
            Vec::<Span<'static>>::new()
        );
    }

    #[test]
    fn activity_indicator_uses_stable_geometry_for_motion_and_reduced_motion() {
        let states = [
            (
                "animated emphasized",
                activity_indicator_for_elapsed(Duration::ZERO),
            ),
            (
                "animated resting",
                activity_indicator_for_elapsed(Duration::from_millis(700)),
            ),
            (
                "reduced static",
                activity_indicator(/*start_time*/ None, MotionMode::Reduced),
            ),
        ];

        insta::assert_debug_snapshot!("activity_indicator_motion_modes", states);
    }
}
