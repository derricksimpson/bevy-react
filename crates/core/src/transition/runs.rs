//! The devtools Animations tab's view of a node's transitions: every
//! channel's run in flight, under the `transition` key that armed it.

use crate::animations::Run;
use crate::ext::AnimationRuns;

use super::TransitionState;

impl AnimationRuns for TransitionState {
    /// A shared flight is one `sharedElement` line: the rect, the size
    /// flight, and the value channels its seed armed move together (the
    /// layout run first — its timeline is the flight's).
    fn visit_runs(&mut self, f: &mut dyn FnMut(&'static str, &mut Run)) {
        const SHARED: &str = "sharedElement";
        // The seed's channels, as `running_mask` counts them.
        let armed = if self.shared.active {
            self.shared.armed
        } else {
            0
        };
        let key = |own: &'static str, bit: u32| {
            if (armed >> bit) & 1 == 1 { SHARED } else { own }
        };
        let flight = |own: &'static str, flying: bool| if flying { SHARED } else { own };
        let layout = flight("layout", self.layout.shared_active());
        if let Some(run) = self.layout.run_mut() {
            f(layout, run);
        }
        let size = flight("size", self.shared.size.is_some());
        let channels = [
            (key("transform", 0), &mut self.translate_x.runner),
            (key("transform", 1), &mut self.translate_y.runner),
            (key("transform", 2), &mut self.scale.runner),
            (key("transform", 3), &mut self.scale_x.runner),
            (key("transform", 4), &mut self.scale_y.runner),
            (key("transform", 5), &mut self.rotate.runner),
            (key("opacity", 6), &mut self.opacity.runner),
            (key("borderRadius", 7), &mut self.border_radius.runner),
            (key("backgroundColor", 8), &mut self.color.runner),
            (key("filter", 9), &mut self.filter.channel.runner),
            (
                key("backdropFilter", 10),
                &mut self.backdrop_filter.channel.runner,
            ),
            (size, &mut self.width.runner),
            (size, &mut self.height.runner),
            (size, &mut self.max_width.runner),
            (size, &mut self.max_height.runner),
            ("morphFilter", &mut self.morph.runner),
        ];
        for (label, run) in channels {
            if let Some(run) = run {
                f(label, run);
            }
        }
        if let Some(run) = self.background_gradient.run_mut() {
            f(key("backgroundGradient", 11), run);
        }
        if let Some(run) = self.border_gradient.run_mut() {
            f(key("borderGradient", 12), run);
        }
        let label = key("transform3d", 13);
        for run in self.transform3d.runs_mut() {
            f(label, run);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::animations::{Driver, Easing};

    fn run() -> Option<Run> {
        Some(Run::new(
            Driver::Timing {
                to: 1.0,
                duration: 1.0,
                easing: Easing::Linear,
            },
            0.0,
        ))
    }

    fn labels(state: &mut TransitionState) -> Vec<&'static str> {
        let mut out = Vec::new();
        state.visit_runs(&mut |label, _| out.push(label));
        out
    }

    /// Lines carry the `transition` key; the bit each channel tags a shared
    /// flight by is its `running_mask` bit.
    #[test]
    fn runs_are_labelled_by_transition_key() {
        let mut state = TransitionState::default();
        assert!(labels(&mut state).is_empty(), "nothing in flight");
        state.scale.runner = run();
        state.width.runner = run();
        state.color.runner = run();
        assert_eq!(labels(&mut state), ["transform", "backgroundColor", "size"]);

        state.shared.active = true;
        state.shared.armed = state.running_mask();
        assert_eq!(
            labels(&mut state),
            ["sharedElement", "sharedElement", "size"],
            "the seed's channels fly together; size only with the size flight"
        );
    }
}
