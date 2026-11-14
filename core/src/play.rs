//! Snow taken from Piles and rolled into Snowballs, without any platform input or drawing code.

use std::f32::consts::{PI, TAU};

use crate::geometry::V2;
use crate::layer::SnowLayer;
use crate::simulation::WindowId;

const START_RADIUS: f32 = 6.0;
const MAX_RADIUS: f32 = 40.0;
const FORM_DEPTH: f32 = 14.0;
const MIN_SPEED: f32 = 12.0;
const MAX_BALLS: usize = 12;
const COOLDOWN: f64 = 0.8;
const FRICTION: f32 = 500.0;

/// The support under a Snowball. Window identities survive changes in the order of Piles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Ground,
    Ledge(WindowId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnowballState {
    Rolling { on: Surface },
    Resting { on: Surface },
}

#[derive(Clone, Copy, Debug)]
pub struct Snowball {
    pub center: V2,
    pub radius: f32,
    pub velocity: V2,
    pub state: SnowballState,
    /// Turns by the distance rolled divided by the radius, in radians.
    pub angle: f32,
    support_origin: V2,
    pushed_at: f64,
}

impl Snowball {
    pub fn surface(&self) -> Surface {
        match self.state {
            SnowballState::Rolling { on } | SnowballState::Resting { on } => on,
        }
    }

    pub fn area(&self) -> f32 {
        PI * self.radius * self.radius
    }
}

pub(crate) struct SnowPlay {
    pub balls: Vec<Snowball>,
    pub enabled: bool,
    next_spawn: f64,
}

impl SnowPlay {
    pub fn new() -> Self {
        Self { balls: Vec::new(), enabled: true, next_spawn: 0.0 }
    }

    /// Takes over the cursor's carving only where it is rolling a ball.
    pub fn sweep(&mut self, layer: &mut SnowLayer, on: Surface, point: V2, velocity: V2, dx: f32, clock: f64) -> bool {
        if !self.enabled || velocity.x.abs() < MIN_SPEED || dx == 0.0 {
            return false;
        }
        let Some(column) = layer.index(point.x) else { return false };
        if !layer.visible[column] {
            return false;
        }
        let nearby = self.balls.iter().position(|ball| ball.surface() == on && (point - ball.center).length() <= ball.radius + 18.0);
        let index = if let Some(index) = nearby {
            index
        } else {
            if self.balls.len() >= MAX_BALLS
                || clock < self.next_spawn
                || support_width(layer) < START_RADIUS * 2.0
                || layer.heights[column] < FORM_DEPTH
                || !deep_patch(layer, point.x)
                || point.y < layer.floor(column)
                || point.y > layer.surface(column) + 2.0
                || self.balls.iter().any(|ball| ball.state == SnowballState::Rolling { on })
            {
                return false;
            }
            let x = point.x.clamp(layer.base().x + START_RADIUS, layer.base().x + support_width(layer) - START_RADIUS);
            let area = take_snow(layer, x, START_RADIUS, PI * START_RADIUS * START_RADIUS);
            if area <= 0.0 {
                return false;
            }
            self.balls.push(Snowball {
                center: V2::new(x, layer.surface(column)),
                radius: (area / PI).sqrt(),
                velocity: V2::ZERO,
                state: SnowballState::Rolling { on },
                angle: 0.0,
                support_origin: layer.origin,
                pushed_at: clock,
            });
            self.next_spawn = clock + COOLDOWN;
            self.balls.len() - 1
        };
        let ball = &mut self.balls[index];
        ball.state = SnowballState::Rolling { on };
        ball.velocity = V2::new(velocity.x.clamp(-600.0, 600.0), 0.0);
        ball.pushed_at = clock;
        roll(ball, layer, dx);
        true
    }

    /// With the pointer gone, friction brings rolling to an exact stop. Resting balls only follow their support.
    pub fn advance(&mut self, layer: &mut SnowLayer, on: Surface, dt: f32, clock: f64) -> bool {
        let mut changed = false;
        for ball in self.balls.iter_mut().filter(|ball| ball.surface() == on) {
            let previous = ball.center;
            ball.center += layer.origin - ball.support_origin;
            ball.support_origin = layer.origin;
            if matches!(ball.state, SnowballState::Rolling { .. }) && ball.pushed_at != clock && dt > 0.0 {
                let speed = (ball.velocity.x.abs() - FRICTION * dt).max(0.0);
                ball.velocity.x = ball.velocity.x.signum() * speed;
                if speed <= 1.0 {
                    ball.velocity = V2::ZERO;
                    ball.state = SnowballState::Resting { on };
                } else {
                    roll(ball, layer, ball.velocity.x * dt);
                    changed = true;
                }
            }
            seat(ball, layer);
            changed |= (ball.center - previous).length_squared() > 0.0001;
        }
        changed
    }

    /// A disappearing or shedding Ledge breaks its balls back into snow on the Ground.
    pub fn break_on(&mut self, on: Surface, snow: &mut Vec<(f32, f32)>) {
        self.balls.retain(|ball| {
            if ball.surface() != on {
                return true;
            }
            snow.push((ball.area(), ball.center.x));
            false
        });
    }

    /// Returns balls to this Pile, including at its ends, without discarding any area.
    pub fn return_to(&mut self, on: Surface, layer: &mut SnowLayer) {
        self.balls.retain(|ball| {
            if ball.surface() != on {
                return true;
            }
            let first = layer.clamped_index(ball.center.x - ball.radius);
            let last = layer.clamped_index(ball.center.x + ball.radius);
            let height = ball.area() / ((last - first + 1) as f32 * layer.column_width);
            for i in first..=last {
                layer.set(i, layer.heights[i] + height);
            }
            false
        });
        layer.refresh_top();
    }
}

/// A broad patch must be deep already: a cursor ploughing a dusting into a narrow crest makes no ball.
fn deep_patch(layer: &SnowLayer, x: f32) -> bool {
    let first = layer.clamped_index(x - 32.0);
    let last = layer.clamped_index(x + 32.0);
    let depth: f32 = layer.heights[first..=last].iter().sum();
    depth >= FORM_DEPTH * (last - first + 1) as f32
}

fn seat(ball: &mut Snowball, layer: &SnowLayer) {
    let width = support_width(layer);
    let radius = ball.radius.min(width * 0.5);
    ball.center.x = ball.center.x.clamp(layer.base().x + radius, layer.base().x + width - radius);
    let at = (ball.center.x - layer.base().x) / layer.column_width - 0.5;
    // Match the smoothed surface drawn by the renderer.
    let surface = |i: usize| {
        layer.bed[i] + 0.25 * layer.heights[i.saturating_sub(1)] + 0.5 * layer.heights[i] + 0.25 * layer.heights[(i + 1).min(layer.count() - 1)]
    };
    let at = at.clamp(0.0, (layer.count() - 1) as f32);
    let i = at.floor() as usize;
    let height = surface(i) + (surface((i + 1).min(layer.count() - 1)) - surface(i)) * (at - i as f32);
    ball.center.y = layer.base().y + height + ball.radius;
}

fn roll(ball: &mut Snowball, layer: &mut SnowLayer, distance: f32) {
    let width = support_width(layer);
    let radius = ball.radius.min(width * 0.5);
    let start = ball.center.x;
    ball.center.x = (start + distance).clamp(layer.base().x + radius, layer.base().x + width - radius);
    let distance = ball.center.x - start;
    if distance != 0.0 {
        let budget = (PI * MAX_RADIUS * MAX_RADIUS - ball.area()).max(0.0);
        let taken = take_snow(layer, ball.center.x, ball.radius, (distance.abs() * ball.radius * 0.8).min(budget));
        ball.angle = (ball.angle - distance / ball.radius).rem_euclid(TAU);
        ball.radius = ((ball.area() + taken) / PI).sqrt().min(MAX_RADIUS);
    } else {
        ball.velocity = V2::ZERO;
        ball.state = SnowballState::Resting { on: ball.surface() };
    }
    seat(ball, layer);
}

/// The Ground has an extra column for drawing past the screen edge; balls stop at the actual edge.
fn support_width(layer: &SnowLayer) -> f32 {
    if layer.right > layer.origin.x { (layer.right - layer.origin.x).min(layer.width()) } else { layer.width() }
}

/// Removes only available, visible snow, and measures the area actually removed rather than the requested area.
fn take_snow(layer: &mut SnowLayer, x: f32, radius: f32, budget: f32) -> f32 {
    let first = layer.clamped_index(x - radius);
    let last = layer.clamped_index(x + radius);
    let available: f32 = (first..=last).filter(|&i| layer.visible[i]).map(|i| layer.heights[i] * layer.column_width).sum();
    let share = if available > 0.0 { (budget / available).min(1.0) } else { 0.0 };
    let mut taken = 0.0;
    for i in first..=last {
        if layer.visible[i] {
            let before = layer.heights[i];
            layer.set(i, before * (1.0 - share));
            taken += (before - layer.heights[i]) * layer.column_width;
        }
    }
    taken
}
