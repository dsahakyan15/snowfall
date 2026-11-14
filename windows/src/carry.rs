//! Carrying a window's Pile across monitors. Each window's snow is kept by one screen, the one most of the window is
//! on; when that changes, as the window is dragged over, the Pile is handed to the new screen's snow. Where a window
//! straddles two displays, the other screen draws its part of the Pile, lent by the screen that keeps it, so there is
//! one Pile with no seam or second copy. Snow that falls on the window's part over there is gathered into that Pile.

use std::collections::HashMap;

use snowfall_core::simulation::WindowId;

use crate::screen::Screen;
use crate::tracker::Tracker;

/// How much more of a window, in physical pixels, has to be on another screen before its snow goes there: a window
/// resting across the boundary doesn't hand its snow back and forth.
const HYSTERESIS: f32 = 32.0;

/// Which screen keeps each window's snow, by display name.
#[derive(Default)]
pub struct Carry {
    keepers: HashMap<WindowId, String>,
}

impl Carry {
    /// Hands over the Piles of windows that moved to another screen, gathers what falls on a straddling window's other
    /// parts into its one Pile, and lends that Pile to the screens it reaches into. `now` wakes screens whose lent
    /// Piles changed.
    pub fn update(&mut self, screens: &mut [Screen], tracker: &Tracker, now: i64) {
        if screens.len() < 2 {
            self.keepers.clear();
            if let Some(screen) = screens.first_mut() {
                screen.simulation.set_borrowed(Vec::new());
            }
            return;
        }
        // Each window's visible width on each screen, in physical pixels.
        let mut spans: HashMap<WindowId, Vec<(usize, f32)>> = HashMap::new();
        for (index, screen) in screens.iter().enumerate() {
            let width = screen.simulation.width();
            let scale = screen.scale().0;
            for surface in screen.surfaces(tracker) {
                let visible = surface.frame.max_x().min(width) - surface.frame.min_x().max(0.0);
                if visible > 0.0 {
                    spans.entry(surface.id).or_default().push((index, visible / scale));
                }
            }
        }
        self.keepers.retain(|id, _| spans.contains_key(id));

        let mut lent: Vec<Vec<_>> = vec![Vec::new(); screens.len()];
        for (&id, candidates) in &spans {
            let previous = self.keepers.get(&id).and_then(|name| screens.iter().position(|screen| &screen.monitor.name == name));
            let keeper = keeper(candidates, previous);
            if let Some(previous) = previous
                && previous != keeper
                && let Some(pile) = screens[previous].simulation.take_pile(id)
            {
                screens[keeper].simulation.bring_pile(id, pile);
            }
            self.keepers.insert(id, screens[keeper].monitor.name.clone());
            for &(index, _) in candidates.iter().filter(|(index, _)| *index != keeper) {
                // What fell on the window's part here joins the one Pile, and that Pile is drawn here where it reaches.
                let (scale, offset) = screens[index].mapping_to(&screens[keeper]);
                if let Some(snow) = screens[index].simulation.drain_pile(id) {
                    let snow: Vec<_> = snow.into_iter().map(|(x, area)| (x * scale.0 + offset.0, area * scale.0 * scale.1)).collect();
                    screens[keeper].simulation.add_to_pile(id, &snow);
                }
                if let Some(view) = screens[keeper].simulation.view_pile(id) {
                    let (scale, offset) = screens[keeper].mapping_to(&screens[index]);
                    lent[index].push(view.mapped(scale, offset));
                }
            }
        }
        for (screen, lent) in screens.iter_mut().zip(lent) {
            if screen.simulation.set_borrowed(lent) {
                screen.wake(now);
            }
        }
    }
}

/// The screen that keeps a window's snow: the one most of it is on, unless the one keeping it now has nearly as much.
/// `candidates` pairs each screen the window is on with how much of it is there.
fn keeper(candidates: &[(usize, f32)], previous: Option<usize>) -> usize {
    let (best, widest) = candidates.iter().copied().fold((candidates[0].0, f32::MIN), |best, c| if c.1 > best.1 { c } else { best });
    match previous.and_then(|previous| candidates.iter().find(|c| c.0 == previous)) {
        Some(&(previous, width)) if width + HYSTERESIS >= widest => previous,
        _ => best,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_is_kept_by_the_screen_most_of_it_is_on() {
        assert_eq!(keeper(&[(0, 300.0), (1, 500.0)], None), 1);
        assert_eq!(keeper(&[(0, 800.0)], None), 0);
    }

    #[test]
    fn a_window_resting_across_the_boundary_stays_with_its_screen() {
        assert_eq!(keeper(&[(0, 390.0), (1, 410.0)], Some(0)), 0, "within the margin");
        assert_eq!(keeper(&[(0, 300.0), (1, 500.0)], Some(0)), 1, "clearly moved over");
        assert_eq!(keeper(&[(1, 500.0)], Some(0)), 1, "gone from the old screen");
    }
}
