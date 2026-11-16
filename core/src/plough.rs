//! A Snowplough pushes the Ground's snow and snow play off the far edge.

use crate::layer::SnowLayer;
use crate::simulation::Random;

pub(crate) const SPEED: f32 = 120.0;
pub(crate) const LENGTH: f32 = 124.0;
pub(crate) const THIN_LAYER: f32 = 1.5;
pub(crate) const DEEP_FOR: f64 = 60.0;
pub(crate) const COOLDOWN: f64 = 30.0 * 60.0;

/// A yellow tracked Snowplough, facing along its direction of travel.
#[derive(Clone, Copy, Debug)]
pub struct Snowplough {
    /// The back of the vehicle, in points along the Ground.
    pub x: f32,
    pub direction: f32,
    pub distance: f32,
    pub elapsed: f32,
    /// Snow touching the blade, for the small spray above it.
    pub snow_height: f32,
}

impl Snowplough {
    pub fn blade(&self) -> f32 {
        self.x + self.direction * LENGTH
    }

    /// Scrapes each passed column and heaps its snow in the columns ahead of the blade.
    pub(crate) fn advance(&mut self, ground: &mut SnowLayer, dt: f32) -> f64 {
        self.distance += SPEED * dt;
        self.elapsed += dt;
        self.x += self.direction * SPEED * dt;
        let blade = self.blade();
        let mut area = 0.0_f64;
        let mut edge = None;
        for i in 0..ground.count() {
            if (ground.center_x(i) - blade) * self.direction <= 0.0 {
                let before = ground.heights[i];
                ground.set(i, before.min(THIN_LAYER));
                area += (before - ground.heights[i]) as f64 * ground.column_width as f64;
            } else if (self.direction > 0.0 && edge.is_none()) || self.direction < 0.0 {
                edge = Some(i);
            }
        }
        self.snow_height = 0.0;
        if let Some(edge) = edge {
            let removed = heap_ahead(ground, area, edge, self.direction as i32);
            self.snow_height = ground.heights[edge];
            removed
        } else {
            // Only crossing the far screen edge removes snow. The extra drawing column goes too.
            area
        }
    }
}

/// The mound keeps its slope at the screen edge; its overhang falls away instead of piling into a spike.
fn heap_ahead(ground: &mut SnowLayer, area: f64, edge: usize, direction: i32) -> f64 {
    if area <= 0.0 {
        return 0.0;
    }
    let width = ground.column_width as f64;
    // The same angle of repose as an ordinary Pile, falling away from the blade.
    let slope = 0.8 * width;
    let reach = if direction > 0 { ground.count() - edge } else { edge + 1 };
    let index = |k| if direction > 0 { edge + k } else { edge - k };
    let overhang = |level: f64| {
        let first = (level - slope * reach as f64).max(0.0);
        let count = (first / slope).ceil();
        count * first - slope * count * (count - 1.0) * 0.5
    };
    let needed =
        |level: f64| (0..reach).map(|k| (level - slope * k as f64 - ground.heights[index(k)] as f64).max(0.0)).sum::<f64>() + overhang(level);
    let amount = area / width;
    let mut low = 0.0;
    let mut high = ground.heights[edge] as f64 + amount;
    for _ in 0..32 {
        let middle = (low + high) * 0.5;
        if needed(middle) < amount {
            low = middle;
        } else {
            high = middle;
        }
    }
    let scale = amount / needed(high);
    let removed = overhang(high) * scale * width;
    for k in 0..reach {
        let i = index(k);
        let added = ((high - slope * k as f64 - ground.heights[i] as f64).max(0.0) * scale) as f32;
        ground.heights[i] += added;
        ground.packed[i] += added;
    }
    removed
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
