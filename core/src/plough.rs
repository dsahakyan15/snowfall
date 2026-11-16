//! A Snowplough spills the Ground's snow and pushes snow play off the far edge.

use crate::geometry::V2;
use crate::layer::SnowLayer;
use crate::simulation::{COLUMN_WIDTH, Random, Sprite};

pub(crate) const SPEED: f32 = 120.0;
pub(crate) const LENGTH: f32 = 124.0;
pub(crate) const THIN_LAYER: f32 = 1.5;
const BLADE_CAP: f32 = 26.0;
const BLOWER_CAP: f32 = 9.0;
const MAX_PARTICLES: usize = 160;
pub(crate) const DEEP_FOR: f64 = 60.0;
pub(crate) const COOLDOWN: f64 = 30.0 * 60.0;

/// Either a blade spilling snow over the edge, or a rotary blower with a chute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PloughKind {
    Blade,
    Blower,
}

impl PloughKind {
    pub fn mound_cap(self) -> f32 {
        match self {
            Self::Blade => BLADE_CAP,
            Self::Blower => BLOWER_CAP,
        }
    }
}

/// A yellow tracked Snowplough, facing along its direction of travel.
#[derive(Clone, Debug)]
pub struct Snowplough {
    pub kind: PloughKind,
    /// The back of the vehicle, in points along the Ground.
    pub x: f32,
    pub direction: f32,
    pub distance: f32,
    pub elapsed: f32,
    /// Height of the snow touching the blade.
    pub snow_height: f32,
    /// Snow in each Ground column of the previous step's mound.
    pub(crate) mound: Vec<(usize, f64)>,
}

impl Snowplough {
    pub fn blade(&self) -> f32 {
        self.x + self.direction * LENGTH
    }

    /// Scrapes the Ground into a small mound and spills whatever cannot fit.
    pub(crate) fn advance(&mut self, ground: &mut SnowLayer, dt: f32) -> f64 {
        self.distance += SPEED * dt;
        self.elapsed += dt;
        self.x += self.direction * SPEED * dt;
        let blade = self.blade();
        let off_edge = if self.direction > 0.0 { blade >= ground.right } else { blade <= ground.base().x };
        let width = ground.column_width as f64;
        let mut snow = 0.0;
        for i in 0..ground.count() {
            if off_edge || (ground.center_x(i) - blade) * self.direction <= 0.0 {
                let before = ground.heights[i];
                ground.set(i, before.min(THIN_LAYER));
                snow += (before as f64 - ground.heights[i] as f64) * width;
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
                snow += (before as f64 - after as f64) * width;
            }
        }
        self.snow_height = 0.0;
        if off_edge {
            // The remaining mound and extra drawing column leave with the blade.
            return snow;
        }

        // Include the snow already here, so deep Ground cannot raise the mound above the blade.
        // Work outwards from the blade as the mound grows into untouched columns.
        for step in 0..ground.count() {
            let i = if self.direction > 0.0 { step } else { ground.count() - 1 - step };
            let ahead = (ground.center_x(i) - blade) * self.direction;
            let reach = 2.0 * snow.sqrt().min(self.kind.mound_cap() as f64) as f32;
            if ahead > 0.0 && ahead < reach {
                snow += ground.heights[i] as f64 * width;
                ground.set(i, 0.0);
            }
        }
        let height = snow.sqrt().min(self.kind.mound_cap() as f64) as f32;
        let length = 2.0 * height;
        // A rounded crest and toe, with a maximum forward slope of 0.75.
        for i in 0..ground.count() {
            let ahead = (ground.center_x(i) - blade) * self.direction;
            if ahead <= 0.0 || ahead >= length {
                continue;
            }
            let t = ahead / length;
            let after = height * (1.0 - t * t * (3.0 - 2.0 * t));
            let actual = after as f64;
            // Charge the actual f32 column change, so rounding cannot create or lose snow.
            if actual * width > snow {
                continue;
            }
            ground.heights[i] = after;
            ground.packed[i] = after;
            snow -= actual * width;
            self.mound.push((i, actual));
            self.snow_height = self.snow_height.max(after);
        }
        snow
    }
}

pub(crate) struct Spill {
    kind: PloughKind,
    position: V2,
    velocity: V2,
    radius: f32,
    angle: f32,
    age: f32,
    life: f32,
    area: f64,
}

impl Spill {
    pub fn sprite(&self, opacity: f32) -> Sprite {
        let t = self.age / self.life;
        let (radius, fade) = match self.kind {
            PloughKind::Blade => {
                // Grow and fade above the Ground: the overlay ends at the taskbar's top.
                let edge = (self.position.y / (self.radius * 2.0)).clamp(0.0, 1.0);
                (self.radius * (1.0 + t * 0.25), (1.0 - t) * edge)
            }
            // Thrown snow is loose powder: soft white flakes, not packed lumps.
            PloughKind::Blower => {
                return Sprite {
                    position: self.position,
                    size: (self.radius + self.age * 3.0) * 2.0,
                    alpha: opacity * (1.0 - t) * 0.9,
                    soft: 0.6,
                    ..Sprite::default()
                };
            }
        };
        Sprite { position: self.position, size: radius * 2.0, alpha: opacity * fade, ball: 1.0, angle: self.angle, ..Sprite::default() }
    }
}

pub(crate) struct Plough {
    pub running: Option<Snowplough>,
    pub particles: Vec<Spill>,
    pub automatic: bool,
    pub main_screen: bool,
    deep_for: f64,
    since_run: f64,
    pub removed_area: f64,
}

impl Plough {
    pub fn new() -> Self {
        Self {
            running: None,
            particles: Vec::with_capacity(MAX_PARTICLES),
            automatic: true,
            main_screen: true,
            deep_for: 0.0,
            since_run: COOLDOWN,
            removed_area: 0.0,
        }
    }

    pub fn send(&mut self, width: f32, random: &mut Random) {
        if !self.main_screen || self.running.is_some() {
            return;
        }
        let direction = if random.next() < 0.5 { 1.0 } else { -1.0 };
        let kind = if random.next() < 0.5 { PloughKind::Blade } else { PloughKind::Blower };
        self.running = Some(Snowplough {
            kind,
            x: if direction > 0.0 { -LENGTH - 20.0 } else { width + LENGTH + 20.0 },
            direction,
            distance: 0.0,
            elapsed: 0.0,
            snow_height: 0.0,
            mound: Vec::with_capacity((BLADE_CAP * 2.0 / COLUMN_WIDTH).ceil() as usize + 1),
        });
        self.since_run = 0.0;
        self.deep_for = 0.0;
    }

    pub fn advance(&mut self, ground: &mut SnowLayer, dt: f32, random: &mut Random) {
        for particle in &mut self.particles {
            let gravity = if particle.kind == PloughKind::Blade { 160.0 } else { 300.0 };
            particle.position += particle.velocity * dt + V2::new(0.0, -0.5 * gravity * dt * dt);
            particle.velocity.y -= gravity * dt;
            particle.angle += dt * 6.0;
            particle.age += dt;
        }
        self.particles.retain(|p| p.age < p.life && p.position.y + p.radius > 0.0);

        let Some(plough) = &mut self.running else { return };
        let removed = plough.advance(ground, dt);
        self.removed_area += removed;
        let off_edge = if plough.direction > 0.0 { plough.blade() >= ground.right } else { plough.blade() <= ground.base().x };
        if removed <= 0.0 || off_edge {
            return;
        }
        let kind = plough.kind;
        let direction = plough.direction;
        let blade = plough.blade();
        let top = plough.snow_height;
        // Share large scrapes among a bounded number of lumps, with no snow waiting unseen.
        let count = (removed / random.range(30.0, 78.0) as f64).ceil().clamp(1.0, 12.0) as usize;
        for _ in 0..count {
            let area = removed / count as f64;
            let (position, velocity, radius, life) = match kind {
                PloughKind::Blade => (
                    V2::new(blade + direction * random.range(2.0, 22.0), top.max(12.0) + random.range(0.0, 4.0)),
                    V2::new(direction * random.range(10.0, 60.0), random.range(10.0, 30.0)),
                    (area.sqrt() as f32 * 0.9).clamp(3.0, 8.0),
                    random.range(0.5, 0.8),
                ),
                PloughKind::Blower => (
                    V2::new(blade - direction * 38.0, 82.5),
                    // It moves on with the machine, so the plume rises over the chute instead of trailing onto the roof.
                    V2::new(direction * (SPEED + random.range(-25.0, 25.0)), random.range(170.0, 240.0)),
                    random.range(1.5, 2.8),
                    2.0,
                ),
            };
            if self.particles.len() == MAX_PARTICLES {
                // Merge neighbouring arrivals so fresh snow can still emerge at the blade or chute.
                let same = self.particles.iter().enumerate().skip(1).find(|(_, p)| p.kind == self.particles[0].kind).map(|(i, _)| i);
                let (index, next) = same.map_or((1, 2), |next| (0, next));
                let other = self.particles.swap_remove(next);
                let first = &mut self.particles[index];
                let share = (other.area / (first.area + other.area)) as f32;
                first.position += (other.position - first.position) * share;
                first.velocity += (other.velocity - first.velocity) * share;
                first.radius = first.radius.hypot(other.radius).min(if first.kind == PloughKind::Blade { 8.0 } else { 2.8 });
                first.age = first.age.min(other.age);
                first.life = first.life.max(other.life);
                first.area += other.area;
            }
            self.particles.push(Spill { kind, position, velocity, radius, angle: random.range(0.0, std::f32::consts::TAU), age: 0.0, life, area });
        }
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
        self.particles.clear();
        self.deep_for = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heavy_spill_merges_without_losing_area_or_growing_its_storage() {
        for seed in [1, 2 << 33] {
            let mut random = Random::seeded(seed);
            let mut ground = SnowLayer::new(V2::ZERO, 1281, COLUMN_WIDTH, 40.0, false, (0.0, 0.0));
            ground.right = 2560.0;
            for i in 0..ground.count() {
                ground.set(i, 10_000.0);
            }
            let mut plough = Plough::new();
            plough.send(ground.right, &mut random);
            let machine = plough.running.as_mut().unwrap();
            machine.x = 200.0 - machine.direction * LENGTH;
            let kind = machine.kind;
            let capacity = plough.particles.capacity();
            for _ in 0..30 {
                plough.advance(&mut ground, 1.0 / 120.0, &mut random);
                assert!(plough.particles.len() <= MAX_PARTICLES);
                assert_eq!(plough.particles.capacity(), capacity);
                let area: f64 = plough.particles.iter().map(|p| p.area).sum();
                assert!((area - plough.removed_area).abs() < 0.000001, "merging must keep every scrape visible");
                let p = plough.particles.last().unwrap();
                assert_eq!(p.kind, kind);
                match kind {
                    PloughKind::Blade => {
                        assert!((3.0..=8.0).contains(&p.radius));
                        assert!((0.5..=0.8).contains(&p.life));
                        assert!((10.0..=60.0).contains(&p.velocity.x.abs()));
                        assert!(p.position.y >= 12.0);
                    }
                    PloughKind::Blower => {
                        assert!((1.5..=2.8).contains(&p.radius));
                        assert!((170.0..=240.0).contains(&p.velocity.y));
                        // Thrown along with the machine, give or take a little.
                        assert!((p.velocity.x.abs() - SPEED).abs() <= 25.0);
                        assert_eq!(p.position.y, 82.5);
                    }
                }
            }
            assert_eq!(plough.particles.len(), MAX_PARTICLES, "the pass should exercise merging");
            let p = &plough.particles[0];
            let before = p.sprite(1.0);
            plough.running = None;
            plough.advance(&mut ground, 0.05, &mut random);
            let after = plough.particles[0].sprite(1.0);
            assert!(after.size > before.size && after.alpha < before.alpha);
            // Only packed lumps tumble; blown powder has no turn to show.
            assert!(kind == PloughKind::Blower || after.angle != before.angle);
            for _ in 0..240 {
                plough.advance(&mut ground, 1.0 / 120.0, &mut random);
            }
            assert!(plough.particles.is_empty());
        }
    }

    #[test]
    fn deep_ground_passes_cap_the_mound_and_conserve_every_column_of_snow() {
        let screen = 2560.0;
        for kind in [PloughKind::Blade, PloughKind::Blower] {
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
                            kind,
                            x: if direction > 0.0 { -LENGTH - 20.0 } else { screen + LENGTH + 20.0 },
                            direction,
                            distance: 0.0,
                            elapsed: 0.0,
                            snow_height: 0.0,
                            mound: Vec::with_capacity(27),
                        };
                        let capacity = plough.mound.capacity();
                        let mut removed = 0.0;
                        let mut saw_cap = false;
                        let mut maximum = 0.0_f32;
                        let mut saw_spill = false;
                        loop {
                            removed += plough.advance(&mut ground, dt);
                            let blade = plough.blade();
                            let off_edge = if direction > 0.0 { blade >= screen } else { blade <= 0.0 };
                            assert_eq!(area(&ground) + removed, before, "area changed: {kind:?}, {depth}, {direction}, {dt}, {blade}");
                            assert!(plough.snow_height <= kind.mound_cap());
                            saw_cap |= plough.snow_height > kind.mound_cap() - 0.5;
                            maximum = maximum.max(plough.snow_height);
                            saw_spill |= !off_edge && removed > 0.0;
                            for i in 0..ground.count() {
                                let ahead = (ground.center_x(i) - blade) * direction;
                                if off_edge || ahead <= 0.0 {
                                    assert!(ground.heights[i] <= THIN_LAYER, "a stale mound was left behind the blade");
                                }
                            }
                            for &(i, height) in &plough.mound {
                                assert!(height <= kind.mound_cap() as f64, "uncapped mound: {height}");
                                assert_eq!(ground.heights[i] as f64, height);
                                assert!((ground.center_x(i) - blade) * direction > 0.0);
                            }
                            assert_eq!(plough.mound.capacity(), capacity, "reshaping the mound must reuse its storage");
                            assert!(plough.distance < screen + LENGTH * 3.0, "the pass never finished");
                            if off_edge {
                                break;
                            }
                        }
                        assert!(saw_cap, "deep Ground should build a visible mound: {kind:?}, {depth}, {direction}, {dt}, max {maximum}");
                        assert!(saw_spill, "excess snow must leave before the far edge");
                        assert!(area(&ground) / (screen as f64) < 2.0);
                        assert!(plough.mound.is_empty());
                        assert_eq!(plough.snow_height, 0.0);
                    }
                }
            }
        }
    }
}
