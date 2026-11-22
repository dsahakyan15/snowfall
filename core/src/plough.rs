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
pub(crate) const MAX_CLUMPS: usize = 24;
const SLUMP_REACH: f32 = 512.0;
const REPOSE: f32 = 1.0;
pub(crate) const DEEP_FOR: f64 = 60.0;
pub(crate) const COOLDOWN: f64 = 30.0 * 60.0;

/// Either a blade spilling snow over the edge, or a rotary blower with a chute.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PloughKind {
    Blade,
    Blower,
}

impl PloughKind {
    /// Snow above the blade or auger can collapse onto the machine.
    pub fn cut_height(self) -> f32 {
        match self {
            Self::Blade => 31.0,
            Self::Blower => 30.0,
        }
    }

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
    /// Smoothed snow area cleared per point travelled, independent of frame rate.
    pub load: f32,
    /// Height of the snow touching the blade.
    pub snow_height: f32,
    /// Each mound column's index, original Ground height, and added snow height.
    pub(crate) mound: Vec<(usize, f32, f64)>,
    pub(crate) clumps: [Option<RollingClump>; MAX_CLUMPS],
    /// Fractional spawn credit, with no snow area attached.
    pub(crate) clump_snow: f32,
}

impl Snowplough {
    pub fn blade(&self) -> f32 {
        self.x + self.direction * LENGTH
    }

    fn burden(&self) -> f32 {
        let cut = self.kind.cut_height();
        ((self.load - cut) / (95.0 - cut)).clamp(0.0, 1.0)
    }

    pub fn speed(&self) -> f32 {
        SPEED * (1.0 - 0.55 * self.burden())
    }

    /// Scrapes the Ground into a small mound and spills whatever cannot fit.
    pub(crate) fn advance(&mut self, ground: &mut SnowLayer, dt: f32) -> f64 {
        let previous = self.x;
        self.x += self.direction * self.speed() * dt;
        let travelled = (self.x - previous).abs();
        self.distance += travelled;
        self.elapsed += dt;
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
        let mut cleared = snow;
        // Passed mound columns were scraped above. Reclaim only those still ahead, so reshaping
        // cannot add another mound on top of the old one, even at sub-column speeds.
        for (i, original, added) in self.mound.drain(..) {
            if i < ground.count() && !off_edge && (ground.center_x(i) - blade) * self.direction > 0.0 {
                let before = ground.heights[i];
                let after = (before as f64 - added).max(original as f64).min(before as f64) as f32;
                ground.packed[i] = (ground.packed[i] - (before - after)).max(0.0);
                ground.set(i, after);
                snow += (before as f64 - after as f64) * width;
            }
        }
        self.snow_height = 0.0;
        let mut slumped = 0.0;
        let mut crest = None;
        if !off_edge {
            let toe = blade + self.direction * SLUMP_REACH;
            let start = ground.clamped_index(blade.min(toe));
            let end = ground.clamped_index(blade.max(toe));
            let share = 1.0 - (-9.0 * dt).exp();
            for i in start..=end {
                let ahead = (ground.center_x(i) - blade) * self.direction;
                if ahead <= 0.0 || ahead > SLUMP_REACH {
                    continue;
                }
                let before = ground.heights[i];
                let limit = self.kind.cut_height() + REPOSE * ahead;
                let excess = (before - limit).max(0.0);
                ground.set(i, before - excess * share);
                // Count the rounded column change, just like a scrape.
                slumped += (before as f64 - ground.heights[i] as f64) * width;
                if excess > 1.0 && crest.is_none_or(|top| ahead > top) {
                    crest = Some(ahead);
                }
            }
        }
        snow += slumped;
        cleared += slumped;
        if travelled > 0.0 {
            // Reclaimed mound snow is already carried load, not fresh work.
            self.load += (cleared as f32 / travelled - self.load) * (1.0 - (-dt / 0.3).exp());
        }
        if off_edge {
            // The remaining mound and extra drawing column leave with the blade.
            self.clumps.fill(None);
            return snow;
        }
        if snow <= 0.0 {
            self.roll_clumps(ground, dt, slumped, crest);
            return snow;
        }

        // A rounded crest and toe, with a maximum forward slope of 0.75.
        let profile = |ahead: f32, height: f32| {
            let length = 2.0 * height;
            if ahead <= 0.0 || ahead >= length {
                return 0.0;
            }
            let t = ahead / length;
            height * (1.0 - t * t * (3.0 - 2.0 * t))
        };
        let mut height = self.kind.mound_cap();
        let toe = blade + self.direction * 2.0 * height;
        let start = ground.clamped_index(blade.min(toe));
        let end = ground.clamped_index(blade.max(toe));
        let needed = |height| {
            (start..=end)
                .map(|i| {
                    let ahead = (ground.center_x(i) - blade) * self.direction;
                    (profile(ahead, height).max(ground.heights[i]) as f64 - ground.heights[i] as f64) * width
                })
                .sum::<f64>()
        };
        // Fit only the part above the untouched Ground to the scraped load. Counting the entire
        // profile would spill small scrapes before a bump could ever form on shallow Ground.
        if needed(height) > snow {
            let mut low = 0.0;
            let mut high = height;
            for _ in 0..24 {
                let middle = (low + high) * 0.5;
                if needed(middle) <= snow {
                    low = middle;
                } else {
                    high = middle;
                }
            }
            height = low;
        }
        let length = 2.0 * height;
        for i in start..=end {
            let ahead = (ground.center_x(i) - blade) * self.direction;
            if ahead <= 0.0 || ahead >= length {
                continue;
            }
            let original = ground.heights[i];
            let after = original.max(profile(ahead, height));
            let added = after as f64 - original as f64;
            // Charge the actual f32 column change, so rounding cannot create or lose snow.
            if added <= 0.0 || added * width > snow {
                continue;
            }
            ground.heights[i] = after;
            ground.packed[i] = after;
            snow -= added * width;
            self.mound.push((i, original, added));
            self.snow_height = self.snow_height.max(after);
        }
        self.roll_clumps(ground, dt, slumped, crest);
        snow
    }

    /// Packed visual crumbs follow the slope; their snow is already in the load.
    fn roll_clumps(&mut self, ground: &SnowLayer, dt: f32, slumped: f64, crest: Option<f32>) {
        let blade = self.blade();
        let drag = (-0.6 * dt).exp();
        let surface = |x| ground.base().y + crate::layer::sample(&ground.heights, (x - ground.base().x) / ground.column_width - 0.5);
        for slot in &mut self.clumps {
            if let Some(clump) = slot {
                let ahead = (clump.position.x - blade) * self.direction;
                let slope =
                    ((surface(clump.position.x + self.direction * 2.0) - surface(clump.position.x - self.direction * 2.0)) / 4.0).clamp(0.0, 2.0);
                clump.speed = (clump.speed + 80.0 * slope / (1.0 + slope * slope) * dt) * drag;
                let rolled = clump.speed * dt;
                clump.position.x -= self.direction * rolled;
                clump.position.y = surface(clump.position.x) + clump.radius;
                clump.angle += self.direction * rolled / clump.radius;
                clump.age += dt;
                if ahead <= rolled + clump.radius || clump.age >= 1.3 {
                    *slot = None;
                }
            }
        }
        self.clump_snow = (self.clump_snow + (slumped as f32 / 100.0).min(36.0 * dt)).min(3.0);
        let Some(crest) = crest else { return };
        for (i, slot) in self.clumps.iter_mut().enumerate() {
            if self.clump_snow < 1.0 {
                break;
            }
            if slot.is_some() {
                continue;
            }
            // Stagger crumbs without taking random draws from the Blade's spill.
            let phase = (self.elapsed * 17.0 + i as f32 * 0.618).fract();
            let radius = 2.0 + phase * 3.5;
            let ahead = (crest - phase * 8.0).max(radius);
            let x = blade + self.direction * ahead;
            let y = surface(x) + radius;
            *slot =
                Some(RollingClump { position: V2::new(x, y), speed: 20.0 + phase * 20.0, radius, angle: phase * std::f32::consts::TAU, age: 0.0 });
            self.clump_snow -= 1.0;
        }
    }
}

/// No area: slumping has already transferred this snow to the blade.
#[derive(Clone, Copy, Debug)]
pub(crate) struct RollingClump {
    position: V2,
    speed: f32,
    radius: f32,
    angle: f32,
    age: f32,
}

impl RollingClump {
    pub fn sprite(&self, opacity: f32) -> Sprite {
        Sprite {
            position: self.position,
            size: self.radius * 2.0,
            alpha: opacity * ((1.3 - self.age) / 0.3).clamp(0.0, 1.0),
            ball: 1.0,
            angle: self.angle,
            ..Sprite::default()
        }
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
            load: 0.0,
            snow_height: 0.0,
            mound: Vec::with_capacity((BLADE_CAP * 2.0 / COLUMN_WIDTH).ceil() as usize + 1),
            clumps: [None; MAX_CLUMPS],
            clump_snow: 0.0,
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
        let previous = plough.x;
        let removed = plough.advance(ground, dt);
        let speed = if dt > 0.0 { (plough.x - previous).abs() / dt } else { 0.0 };
        self.removed_area += removed;
        let off_edge = if plough.direction > 0.0 { plough.blade() >= ground.right } else { plough.blade() <= ground.base().x };
        if removed <= 0.0 || off_edge {
            return;
        }
        let kind = plough.kind;
        let direction = plough.direction;
        let blade = plough.blade();
        let top = plough.snow_height;
        let boost = if kind == PloughKind::Blower { 1.0 + 1.2 * plough.burden() } else { 1.0 };
        // Share large scrapes among a bounded number of lumps, with no snow waiting unseen.
        let count = (removed * boost as f64 / random.range(30.0, 78.0) as f64).ceil().clamp(1.0, 12.0 * boost as f64) as usize;
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
                    V2::new(direction * (speed + random.range(-25.0, 25.0)), random.range(170.0, 240.0) * boost),
                    random.range(1.5, 2.8) * boost,
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
                first.radius = first.radius.hypot(other.radius).min(if first.kind == PloughKind::Blade { 8.0 } else { 2.8 * boost });
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
                let previous = plough.running.as_ref().unwrap().x;
                plough.advance(&mut ground, 1.0 / 120.0, &mut random);
                assert!(plough.particles.len() <= MAX_PARTICLES);
                assert_eq!(plough.particles.capacity(), capacity);
                let area: f64 = plough.particles.iter().map(|p| p.area).sum();
                assert!((area - plough.removed_area).abs() < 0.000001, "merging must keep every scrape visible");
                let p = plough.particles.last().unwrap();
                assert_eq!(p.kind, kind);
                // Sub-column steps can leave every new column ahead of the face untouched.
                // Check launch parameters only when this step actually scraped and spilled snow.
                if p.age > 0.0 {
                    continue;
                }
                match kind {
                    PloughKind::Blade => {
                        assert!((3.0..=8.0).contains(&p.radius));
                        assert!((0.5..=0.8).contains(&p.life));
                        assert!((10.0..=60.0).contains(&p.velocity.x.abs()));
                        assert!(p.position.y >= 12.0);
                    }
                    PloughKind::Blower => {
                        let boost = 1.0 + 1.2 * plough.running.as_ref().unwrap().burden();
                        assert!((1.5 * boost..=2.8 * boost).contains(&p.radius));
                        assert!((170.0 * boost..=240.0 * boost).contains(&p.velocity.y));
                        // Thrown along with the machine, give or take a little.
                        let speed = (plough.running.as_ref().unwrap().x - previous).abs() * 120.0;
                        assert!((p.velocity.x.abs() - speed).abs() <= 25.0);
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
    fn snow_ahead_of_the_blade_never_falls_below_its_original_or_slump_limit() {
        let screen = 2560.0;
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for depth in [8.0, 26.0, 60.0] {
                for direction in [1.0, -1.0] {
                    for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                        let mut ground = SnowLayer::new(V2::ZERO, 1281, COLUMN_WIDTH, 40.0, false, (0.0, 0.0));
                        ground.right = screen;
                        for i in 0..ground.count() {
                            ground.set(i, depth);
                        }
                        let original = ground.heights.clone();
                        let mut plough = Snowplough {
                            kind,
                            x: if direction > 0.0 { -LENGTH - 20.0 } else { screen + LENGTH + 20.0 },
                            direction,
                            distance: 0.0,
                            elapsed: 0.0,
                            load: 0.0,
                            snow_height: 0.0,
                            mound: Vec::new(),
                            clumps: [None; MAX_CLUMPS],
                            clump_snow: 0.0,
                        };
                        loop {
                            plough.advance(&mut ground, dt);
                            let blade = plough.blade();
                            let off_edge = if direction > 0.0 { blade >= screen } else { blade <= 0.0 };
                            for (i, &before) in original.iter().enumerate() {
                                let height = ground.heights[i];
                                if off_edge || (ground.center_x(i) - blade) * direction <= 0.0 {
                                    assert!(height <= THIN_LAYER + 0.00001, "a stale mound was left behind the blade");
                                } else {
                                    assert!(
                                        height >= before.min(kind.cut_height() + (ground.center_x(i) - blade) * direction) - 0.00001,
                                        "trench ahead of blade: {kind:?}, depth {depth}, direction {direction}, dt {dt}, blade {blade}, column {i}: {height} < {before}"
                                    );
                                    assert!(
                                        height <= before.max(kind.mound_cap()) + 0.00001,
                                        "snow ahead exceeded both the original height and mound cap"
                                    );
                                }
                            }
                            assert!(plough.distance < screen + LENGTH * 3.0, "the pass never finished");
                            if off_edge {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn ground_passes_cap_only_added_mounds_and_conserve_every_column_of_snow() {
        let screen = 2560.0;
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for depth in [8.0, 26.0, 60.0] {
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
                            load: 0.0,
                            snow_height: 0.0,
                            mound: Vec::with_capacity(27),
                            clumps: [None; MAX_CLUMPS],
                            clump_snow: 0.0,
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
                            for &(i, original, added) in &plough.mound {
                                assert_eq!(original, depth, "reshaping must preserve the original Ground height");
                                assert!(ground.heights[i] <= kind.mound_cap(), "uncapped mound: {}", ground.heights[i]);
                                assert_eq!(ground.heights[i] as f64, original as f64 + added);
                                assert!((ground.center_x(i) - blade) * direction > 0.0);
                            }
                            assert_eq!(plough.mound.capacity(), capacity, "reshaping the mound must reuse its storage");
                            assert!(plough.distance < screen + LENGTH * 3.0, "the pass never finished");
                            if off_edge {
                                break;
                            }
                        }
                        if depth < kind.mound_cap() {
                            assert!(saw_cap, "shallow Ground should build a capped mound: {kind:?}, {depth}, {direction}, {dt}, max {maximum}");
                        } else {
                            assert_eq!(maximum, 0.0, "deep Ground should have no added mound");
                        }
                        assert!(saw_spill, "excess snow must leave before the far edge");
                        assert!(area(&ground) / (screen as f64) < 2.0);
                        assert!(plough.mound.is_empty());
                        assert_eq!(plough.snow_height, 0.0);
                    }
                }
            }
        }
    }

    fn scene(kind: PloughKind, direction: f32, depth: f32) -> (SnowLayer, Plough, Random) {
        let mut ground = SnowLayer::new(V2::ZERO, 801, COLUMN_WIDTH, 40.0, false, (0.0, 0.0));
        ground.right = 1600.0;
        for i in 0..ground.count() {
            ground.set(i, depth);
            ground.packed[i] = depth;
        }
        let mut random = Random::seeded(1);
        let mut plough = Plough::new();
        plough.send(ground.right, &mut random);
        let machine = plough.running.as_mut().unwrap();
        machine.kind = kind;
        machine.direction = direction;
        machine.x = if direction > 0.0 { -LENGTH - 20.0 } else { ground.right + LENGTH + 20.0 };
        (ground, plough, random)
    }

    fn area(ground: &SnowLayer) -> f64 {
        ground.heights.iter().map(|&h| h as f64 * ground.column_width as f64).sum()
    }

    #[test]
    fn tall_faces_slump_without_trenches_and_conserve_area_at_every_step() {
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for direction in [1.0, -1.0] {
                for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                    for depth in [95.0, 140.0] {
                        let (mut ground, mut plough, mut random) = scene(kind, direction, 4.0);
                        for i in 0..ground.count() {
                            if (320.0..1280.0).contains(&ground.center_x(i)) {
                                ground.set(i, depth);
                                ground.packed[i] = depth;
                            }
                        }
                        let original = ground.heights.clone();
                        let before = area(&ground);
                        let capacity = plough.running.as_ref().unwrap().mound.capacity();
                        let mut saw_clumps = false;
                        let mut filled_clumps = false;
                        let mut saw_face = false;
                        let mut saw_gradual = false;
                        loop {
                            plough.advance(&mut ground, dt, &mut random);
                            let machine = plough.running.as_ref().unwrap();
                            let blade = machine.blade();
                            let passed = if direction > 0.0 { blade >= ground.right } else { blade <= 0.0 };
                            assert_eq!(area(&ground) + plough.removed_area, before, "{kind:?}, {direction}, {dt}, {depth}");
                            assert_eq!(machine.mound.capacity(), capacity);
                            assert!(plough.particles.len() <= MAX_PARTICLES);
                            let clumps = machine.clumps.iter().flatten().count();
                            assert!(clumps <= MAX_CLUMPS);
                            saw_clumps |= clumps > 0;
                            filled_clumps |= clumps == MAX_CLUMPS;
                            for (i, &original) in original.iter().enumerate() {
                                let ahead = (ground.center_x(i) - blade) * direction;
                                let height = ground.heights[i];
                                assert!(ground.packed[i] <= height);
                                if passed || ahead <= 0.0 {
                                    assert!(height <= THIN_LAYER);
                                    continue;
                                }
                                let limit = kind.cut_height() + REPOSE * ahead;
                                assert!(height >= original.min(limit) - 0.00001, "a slump dug a trench");
                                saw_gradual |= original == depth && height > limit + 1.0 && height < original - 1.0;
                                // A moving limit leaves at most speed / 9 of lag, about 6 pt under load.
                                if (384.0..1216.0).contains(&blade) {
                                    assert!(
                                        height <= original.min(limit + 8.0).max(kind.mound_cap()) + 0.0001,
                                        "wall: {kind:?}, {direction}, {dt}, {depth}, blade {blade}, ahead {ahead}, height {height}, limit {limit}"
                                    );
                                    saw_face |= original == depth && ahead < 20.0;
                                }
                            }
                            assert!(machine.elapsed < 40.0, "pass never finished");
                            if passed {
                                assert_eq!(clumps, 0, "crumbs disappear at the blade's exit");
                                break;
                            }
                        }
                        assert!(saw_clumps && saw_face && saw_gradual);
                        if depth == 140.0 {
                            assert!(filled_clumps, "heavy slumping should exercise the fixed capacity");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn load_slows_the_machine_but_not_shallow_passes_or_track_distance() {
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for direction in [1.0, -1.0] {
                for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                    for depth in [4.0, kind.cut_height(), 95.0, 140.0] {
                        let (mut ground, mut plough, mut random) = scene(kind, direction, depth);
                        let start = plough.running.as_ref().unwrap().x;
                        let mut integrated = 0.0;
                        for _ in 0..(8.0_f32 / dt).round() as usize {
                            let machine = plough.running.as_ref().unwrap();
                            let previous = machine.x;
                            let speed = machine.speed();
                            assert!((SPEED * 0.45 - 0.0001..=SPEED).contains(&speed));
                            if depth <= kind.cut_height() {
                                assert_eq!(speed, SPEED, "shallow load: {kind:?}, {direction}, {dt}, {depth}, {}", machine.load);
                            } else if machine.elapsed > 4.0 {
                                assert!((speed / SPEED - 0.45).abs() < 0.05, "speed: {kind:?}, {direction}, {dt}, {depth}, {}", speed / SPEED);
                            }
                            plough.advance(&mut ground, dt, &mut random);
                            let machine = plough.running.as_ref().unwrap();
                            let travelled = (machine.x - previous).abs();
                            assert!((travelled - speed * dt).abs() < 0.0002);
                            integrated += travelled;
                            assert_eq!(machine.distance, integrated);
                            assert!((machine.distance - (machine.x - start).abs()).abs() < 0.02);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn load_and_slump_remain_consistent_across_frame_rates() {
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for direction in [1.0, -1.0] {
                for depth in [95.0, 140.0] {
                    let mut reference: Option<(f32, f32, f32)> = None;
                    for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                        let (mut ground, mut plough, mut random) = scene(kind, direction, depth);
                        for _ in 0..(6.0_f32 / dt).round() as usize {
                            plough.advance(&mut ground, dt, &mut random);
                        }
                        let machine = plough.running.as_ref().unwrap();
                        let at = (machine.blade() + direction * 20.0) / ground.column_width - 0.5;
                        let height = crate::layer::sample(&ground.heights, at);
                        if let Some((distance, speed, face)) = reference {
                            assert!((machine.distance - distance).abs() < 4.0, "frame rate changed travel");
                            assert!((machine.speed() - speed).abs() < SPEED * 0.04, "frame rate changed settled speed");
                            assert!((height - face).abs() < 2.0, "frame rate changed the slope");
                        } else {
                            reference = Some((machine.distance, machine.speed(), height));
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn blower_launches_larger_faster_and_denser_powder_under_load() {
        for direction in [1.0, -1.0] {
            for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                let mut samples = Vec::new();
                for depth in [20.0, 95.0] {
                    let (mut ground, mut plough, mut random) = scene(PloughKind::Blower, direction, depth);
                    let mut launches = 0;
                    let mut removed = 0.0;
                    let mut vertical = 0.0;
                    let mut radius = 0.0;
                    for _ in 0..(6.0_f32 / dt).round() as usize {
                        let machine = plough.running.as_ref().unwrap();
                        let previous = machine.x;
                        let sample = machine.elapsed > 4.0;
                        let before = plough.removed_area;
                        // Measure launches before the independently tested capacity merging.
                        plough.particles.clear();
                        plough.advance(&mut ground, dt, &mut random);
                        if sample {
                            removed += plough.removed_area - before;
                            let speed = (plough.running.as_ref().unwrap().x - previous).abs() / dt;
                            for p in plough.particles.iter().filter(|p| p.age == 0.0) {
                                launches += 1;
                                vertical += p.velocity.y;
                                radius += p.radius;
                                assert!((p.velocity.x * direction - speed).abs() <= 25.0001);
                            }
                        }
                    }
                    assert!(launches > 0);
                    samples.push((vertical / launches as f32, radius / launches as f32, launches as f64 / removed));
                }
                assert!(samples[1].0 > samples[0].0 * 1.8, "vertical boost: {samples:?}");
                assert!(samples[1].1 > samples[0].1 * 1.8, "radius boost: {samples:?}");
                assert!(samples[1].2 > samples[0].2, "density boost: {samples:?}");
            }
        }
    }

    #[test]
    fn rolling_clumps_follow_the_surface_fade_and_never_carry_area() {
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for direction in [1.0, -1.0] {
                for dt in [1.0 / 120.0, 1.0 / 60.0, 1.0 / 24.0] {
                    let (mut ground, mut plough, mut random) = scene(kind, direction, 140.0);
                    let machine = plough.running.as_mut().unwrap();
                    machine.x = 800.0 - direction * LENGTH;
                    machine.load = 140.0;
                    let before = area(&ground);
                    // Nothing expires in this short window, so every removed point must be in spill.
                    for _ in 0..(0.3_f32 / dt).ceil() as usize {
                        plough.advance(&mut ground, dt, &mut random);
                        let carried: f64 = plough.particles.iter().map(|p| p.area).sum();
                        assert!((carried - plough.removed_area).abs() < 0.000001);
                        assert_eq!(area(&ground) + plough.removed_area, before);
                    }
                    let machine = plough.running.as_mut().unwrap();
                    let clump = machine.clumps.iter().flatten().next().copied().expect("a tall face sheds crumbs");
                    assert!((2.0..=5.5).contains(&clump.radius));
                    assert_eq!(clump.sprite(0.7).ball, 1.0);
                    assert_eq!(clump.sprite(0.7).alpha, 0.7);
                    let mut saw_fade = false;
                    // Advance only the visuals: no work remains to spawn replacements.
                    for _ in 0..(1.4 / dt).ceil() as usize {
                        let previous = machine.clumps;
                        machine.roll_clumps(&ground, dt, 0.0, None);
                        for (old, new) in previous.iter().zip(&machine.clumps) {
                            if let (Some(old), Some(new)) = (old, new) {
                                assert!((old.position.x - new.position.x) * direction > 0.0);
                                let surface = crate::layer::sample(&ground.heights, new.position.x / ground.column_width - 0.5);
                                assert_eq!(new.position.y, surface + new.radius);
                                assert!(new.sprite(0.7).alpha <= old.sprite(0.7).alpha);
                                saw_fade |= new.sprite(0.7).alpha < old.sprite(0.7).alpha;
                                assert_ne!(new.angle, old.angle);
                            }
                        }
                    }
                    assert!(machine.clumps.iter().all(Option::is_none));
                    assert!(saw_fade, "crumbs should fade near the end of their life");
                    assert_eq!(area(&ground) + plough.removed_area, before);
                }
            }
        }
    }
}
