//! A Snowplough pushes the Ground's snow and snow play off the far edge.

use crate::layer::SnowLayer;
use crate::simulation::Random;

pub(crate) const SPEED: f32 = 120.0;
pub(crate) const LENGTH: f32 = 124.0;
pub(crate) const THIN_LAYER: f32 = 1.5;
pub(crate) const MOUND_MAX: f32 = 40.0;
pub(crate) const DEEP_FOR: f64 = 60.0;
pub(crate) const COOLDOWN: f64 = 30.0 * 60.0;

/// A yellow tracked Snowplough, facing along its direction of travel.
#[derive(Clone, Debug)]
pub struct Snowplough {
    /// The back of the vehicle, in points along the Ground.
    pub x: f32,
    pub direction: f32,
    pub distance: f32,
    pub elapsed: f32,
    /// Snow touching the blade, for the small spray above it.
    pub snow_height: f32,
    /// Scraped snow area (pt²) carried out of sight, excluding the visible mound.
    pub carried: f64,
    /// Height added to each Ground column by the previous step's mound.
    pub(crate) mound: Vec<(usize, f64)>,
}

impl Snowplough {
    pub fn blade(&self) -> f32 {
        self.x + self.direction * LENGTH
    }

    /// Scrapes passed columns into the load, showing only a modest mound ahead of the blade.
    pub(crate) fn advance(&mut self, ground: &mut SnowLayer, dt: f32) -> f64 {
        self.distance += SPEED * dt;
        self.elapsed += dt;
        self.x += self.direction * SPEED * dt;
        let blade = self.blade();
        let off_edge = if self.direction > 0.0 { blade >= ground.right } else { blade <= ground.base().x };
        let width = ground.column_width as f64;
        for i in 0..ground.count() {
            if off_edge || (ground.center_x(i) - blade) * self.direction <= 0.0 {
                let before = ground.heights[i];
                ground.set(i, before.min(THIN_LAYER));
                self.carried += (before as f64 - ground.heights[i] as f64) * width;
            }
        }
        // Passed mound columns were scraped above. Reclaim only those still ahead, so reshaping
        // cannot add another mound on top of the old one, even at sub-column speeds.
        for (i, added) in self.mound.drain(..) {
            if i < ground.count() && !off_edge && (ground.center_x(i) - blade) * self.direction > 0.0 {
                let before = ground.heights[i];
                let after = (before as f64 - added).max(0.0) as f32;
                ground.packed[i] = (ground.packed[i] - (before - after)).max(0.0);
                ground.set(i, after);
                self.carried += (before as f64 - after as f64) * width;
            }
        }
        self.snow_height = 0.0;
        if off_edge {
            // The remaining mound, load and extra drawing column leave with the blade.
            return std::mem::take(&mut self.carried);
        }

        let height = (self.carried.sqrt() * 0.8).min(MOUND_MAX as f64) as f32;
        let length = 2.0 * height;
        // A rounded crest and toe, with a maximum forward slope of 0.75. Excess area
        // stays aboard instead of forcing the mound higher or further across the screen.
        for i in 0..ground.count() {
            let ahead = (ground.center_x(i) - blade) * self.direction;
            if ahead <= 0.0 || ahead >= length {
                continue;
            }
            let t = ahead / length;
            let added = height * (1.0 - t * t * (3.0 - 2.0 * t));
            let before = ground.heights[i];
            let after = before + added;
            let actual = after as f64 - before as f64;
            // Charge the actual f32 column change, so rounding cannot create or lose snow.
            if actual * width > self.carried {
                continue;
            }
            ground.heights[i] = after;
            ground.packed[i] += actual as f32;
            self.carried -= actual * width;
            self.mound.push((i, actual));
            self.snow_height = self.snow_height.max(actual as f32).min(MOUND_MAX);
        }
        0.0
    }
}

pub(crate) struct Plough {
    pub running: Option<Snowplough>,
    pub automatic: bool,
    pub main_screen: bool,
    deep_for: f64,
    since_run: f64,
    pub removed_area: f64,
}

impl Plough {
    pub fn new() -> Self {
        Self { running: None, automatic: true, main_screen: true, deep_for: 0.0, since_run: COOLDOWN, removed_area: 0.0 }
    }

    pub fn send(&mut self, width: f32, random: &mut Random) {
        if !self.main_screen || self.running.is_some() {
            return;
        }
        let direction = if random.next() < 0.5 { 1.0 } else { -1.0 };
        self.running = Some(Snowplough {
            x: if direction > 0.0 { -LENGTH - 20.0 } else { width + LENGTH + 20.0 },
            direction,
            distance: 0.0,
            elapsed: 0.0,
            snow_height: 0.0,
            carried: 0.0,
            mound: Vec::new(),
        });
        self.since_run = 0.0;
        self.deep_for = 0.0;
    }

    pub fn wait(&mut self, ground: &SnowLayer, usual_depth: f32, dt: f32, random: &mut Random) {
        self.since_run += dt as f64;
        // The Ground's usual full depth is 40 pt (with small variations along it): 60% is 24 pt.
        // A full minute above that mean ignores short heaps; thirty minutes keeps visits rare.
        let mean = ground.heights.iter().sum::<f32>() / ground.count().max(1) as f32;
        if self.main_screen && self.automatic && self.running.is_none() && mean > usual_depth * 0.6 {
            self.deep_for += dt as f64;
        } else {
            self.deep_for = 0.0;
        }
        if self.deep_for >= DEEP_FOR && self.since_run >= COOLDOWN {
            self.send(ground.right, random);
        }
    }

    pub fn stop(&mut self) {
        self.running = None;
        self.deep_for = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::V2;

    #[test]
    fn deep_ground_passes_cap_the_mound_and_conserve_every_column_of_snow_both_ways() {
        let screen = 2560.0;
        for depth in [26.0, 60.0] {
            for direction in [1.0, -1.0] {
                // Include sub-column motion and steps crossing several columns at once.
                for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                    let mut ground = SnowLayer::new(V2::ZERO, 1281, 2.0, 40.0, false, (0.0, 0.0));
                    ground.right = screen;
                    for i in 0..ground.count() {
                        ground.set(i, depth);
                    }
                    let area = |ground: &SnowLayer| ground.heights.iter().map(|&h| h as f64 * ground.column_width as f64).sum::<f64>();
                    let before = area(&ground);
                    let mut plough = Snowplough {
                        x: if direction > 0.0 { -LENGTH - 20.0 } else { screen + LENGTH + 20.0 },
                        direction,
                        distance: 0.0,
                        elapsed: 0.0,
                        snow_height: 0.0,
                        carried: 0.0,
                        mound: Vec::new(),
                    };
                    let mut removed = 0.0;
                    let mut saw_cap = false;
                    while removed == 0.0 {
                        removed += plough.advance(&mut ground, dt);
                        let blade = plough.blade();
                        let off_edge = if direction > 0.0 { blade >= screen } else { blade <= 0.0 };
                        assert_eq!(area(&ground) + plough.carried + removed, before, "area changed: {depth}, {direction}, {dt}, {blade}");
                        assert!(plough.carried >= 0.0);
                        assert!(plough.snow_height <= MOUND_MAX);
                        saw_cap |= plough.snow_height > MOUND_MAX - 0.1;
                        for i in 0..ground.count() {
                            let ahead = (ground.center_x(i) - blade) * direction;
                            if off_edge || ahead <= 0.0 {
                                assert!(ground.heights[i] <= THIN_LAYER, "a stale mound was left behind the blade");
                            } else {
                                let added = ground.heights[i] - depth;
                                assert!((0.0..=MOUND_MAX + 0.0001).contains(&added), "uncapped mound: {added}");
                                if ahead >= MOUND_MAX * 2.0 {
                                    assert_eq!(ground.heights[i], depth, "the mound extends too far ahead");
                                }
                                if i > 0 && (ground.center_x(i - 1) - blade) * direction > 0.0 {
                                    let slope = (ground.heights[i] - ground.heights[i - 1]).abs() / ground.column_width;
                                    assert!(slope <= 0.8, "the mound's front is too steep: {slope}");
                                }
                            }
                        }
                        if !off_edge {
                            assert_eq!(removed, 0.0, "the load must stay aboard until the blade reaches the edge");
                        }
                        assert!(plough.distance < screen + 100.0, "the pass never finished");
                    }
                    assert!(saw_cap, "a deep Ground should build a visible mound");
                    assert!(area(&ground) / (screen as f64) < 2.0);
                    assert_eq!(plough.carried, 0.0);
                    assert!(plough.mound.is_empty());
                    assert_eq!(plough.snow_height, 0.0);
                }
            }
        }
    }
}
