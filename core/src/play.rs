//! Snow taken from Piles and rolled into Snowballs, without any platform input or drawing code.

use std::f32::consts::{PI, TAU};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::geometry::V2;
use crate::layer::SnowLayer;
use crate::simulation::WindowId;

const START_RADIUS: f32 = 6.0;
const MAX_RADIUS: f32 = 40.0;
const FORM_DEPTH: f32 = 14.0;
const MIN_SPEED: f32 = 12.0;
const MAX_BALLS: usize = 12;
/// One long sweep rolls one or two balls, not a row of them.
const COOLDOWN: f64 = 2.5;
const FRICTION: f32 = 500.0;
/// Packed balls withstand a short drop (up to about 160 points at the engine's gravity).
const HARD_LANDING: f32 = 900.0;
const HARD_HIT: f32 = 350.0;
const EDGE_BOUNCE: f32 = 0.2;
/// Compare loose grains' 0.8 drag toward half the wind speed. A compact ball catches less air,
/// and its drag per unit mass falls with radius. There is no velocity damping in free flight.
const WIND_PUSH: f32 = 0.2;

/// Shared by screens so a ball keeps its identity when carried between simulations.
static NEXT_ID: AtomicU32 = AtomicU32::new(1);
pub type SnowballId = u32;

/// The support under a Snowball. Window identities survive changes in the order of Piles.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Ground,
    Ledge(WindowId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnowballState {
    Rolling {
        on: Surface,
    },
    Resting {
        on: Surface,
    },
    Held,
    /// Free flight, including a throw, a drop, or snow shed from a Ledge.
    Falling,
}

#[derive(Clone, Copy, Debug)]
pub struct Snowball {
    pub id: SnowballId,
    pub center: V2,
    pub radius: f32,
    pub velocity: V2,
    pub state: SnowballState,
    /// Turns by horizontal distance divided by the radius, rolling or in free flight, in radians.
    pub angle: f32,
    support_origin: V2,
    pushed_at: f64,
    grab_offset: V2,
}

impl Snowball {
    pub fn surface(&self) -> Option<Surface> {
        match self.state {
            SnowballState::Rolling { on } | SnowballState::Resting { on } => Some(on),
            SnowballState::Held | SnowballState::Falling => None,
        }
    }

    /// Maps places and motion to another screen; radius and snow area stay in points.
    pub fn map_to(&mut self, scale: (f32, f32), offset: (f32, f32)) {
        self.center = V2::new(self.center.x * scale.0 + offset.0, self.center.y * scale.1 + offset.1);
        self.velocity = V2::new(self.velocity.x * scale.0, self.velocity.y * scale.1);
        self.grab_offset = V2::new(self.grab_offset.x * scale.0, self.grab_offset.y * scale.1);
        self.support_origin = V2::new(self.support_origin.x * scale.0 + offset.0, self.support_origin.y * scale.1 + offset.1);
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
        if !self.enabled || self.balls.iter().any(|ball| ball.state == SnowballState::Held) || velocity.x.abs() < MIN_SPEED || dx == 0.0 {
            return false;
        }
        let Some(column) = layer.index(point.x) else { return false };
        if !layer.visible[column] {
            return false;
        }
        let nearby = self.balls.iter().position(|ball| ball.surface() == Some(on) && (point - ball.center).length() <= ball.radius + 18.0);
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
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                center: V2::new(x, layer.surface(column)),
                radius: (area / PI).sqrt(),
                velocity: V2::ZERO,
                state: SnowballState::Rolling { on },
                angle: 0.0,
                support_origin: layer.origin,
                pushed_at: clock,
                grab_offset: V2::ZERO,
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
        for ball in self.balls.iter_mut().filter(|ball| ball.surface() == Some(on)) {
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

    /// A disappearing or shedding Ledge lets its balls fall from where they were.
    pub fn shed_on(&mut self, on: Surface, velocity: V2) {
        for ball in self.balls.iter_mut().filter(|ball| ball.surface() == Some(on)) {
            ball.state = SnowballState::Falling;
            ball.velocity = velocity;
        }
    }

    pub fn grab(&mut self, id: SnowballId, point: V2) -> bool {
        if !self.enabled || self.balls.iter().any(|ball| ball.state == SnowballState::Held) {
            return false;
        }
        let Some(ball) = self.balls.iter_mut().find(|ball| ball.id == id) else { return false };
        if (point - ball.center).length_squared() > ball.radius * ball.radius {
            return false;
        }
        ball.grab_offset = ball.center - point;
        ball.state = SnowballState::Held;
        ball.velocity = V2::ZERO;
        true
    }

    pub fn move_held(&mut self, point: V2) {
        for ball in self.balls.iter_mut().filter(|ball| ball.state == SnowballState::Held) {
            ball.center = point + ball.grab_offset;
        }
    }

    pub fn release(&mut self, velocity: V2) {
        for ball in self.balls.iter_mut().filter(|ball| ball.state == SnowballState::Held) {
            ball.state = SnowballState::Falling;
            ball.velocity = release_velocity(velocity);
        }
    }

    /// Integrates ballistic flight exactly for each small step. Short steps keep fast, small balls
    /// from tunnelling through circle targets, independently of the display's refresh rate.
    /// Side edges rebound with most horizontal speed lost; the top is open so throws return.
    /// Returns broken balls for Air to turn into grains, keeping physics separate from rendering.
    pub fn fly(&mut self, layers: &[SnowLayer], ids: &[WindowId], width: f32, dt: f32, gravity: f32, wind: f32) -> Vec<Snowball> {
        let mut broken = Vec::new();
        if dt <= 0.0 || !self.balls.iter().any(|b| b.state == SnowballState::Falling) {
            return broken;
        }
        let radius = self.balls.iter().map(|b| b.radius).fold(START_RADIUS, f32::min).max(0.5);
        let speed = self.balls.iter().map(|b| b.velocity.length()).fold(0.0, f32::max) + (gravity + wind.abs()) * dt;
        let steps = (dt / (radius / (2.0 * speed.max(1.0))).min(1.0 / 240.0)).ceil().max(1.0) as usize;
        let step = dt / steps as f32;
        let mut gone = vec![false; self.balls.len()];
        let mut previous = vec![V2::ZERO; self.balls.len()];
        for _ in 0..steps {
            for (i, ball) in self.balls.iter_mut().enumerate() {
                previous[i] = ball.center;
                if gone[i] || ball.state != SnowballState::Falling {
                    continue;
                }
                let acceleration = V2::new(wind * WIND_PUSH * START_RADIUS / ball.radius, -gravity);
                let travel = ball.velocity * step + acceleration * (0.5 * step * step);
                ball.center += travel;
                ball.velocity += acceleration * step;
                ball.angle = (ball.angle - travel.x / ball.radius).rem_euclid(TAU);
                bounce_at_edges(ball, width);
            }
            // Circle targets can later include each of the three constituent balls of a Snowman.
            for a in 0..self.balls.len() {
                for b in a + 1..self.balls.len() {
                    if gone[a] || gone[b] {
                        continue;
                    }
                    let (left, right) = self.balls.split_at_mut(b);
                    let (a_ball, b_ball) = (&mut left[a], &mut right[0]);
                    if a_ball.state == SnowballState::Held
                        || b_ball.state == SnowballState::Held
                        || (a_ball.state != SnowballState::Falling && b_ball.state != SnowballState::Falling)
                    {
                        continue;
                    }
                    let delta = b_ball.center - a_ball.center;
                    let distance = delta.length();
                    let reach = a_ball.radius + b_ball.radius;
                    if distance > reach {
                        continue;
                    }
                    let normal = if distance > 0.001 { delta / distance } else { V2::new(1.0, 0.0) };
                    let closing = (a_ball.velocity - b_ball.velocity).dot(normal);
                    if closing > HARD_HIT {
                        gone[a] = true;
                        gone[b] = true;
                        broken.extend([*a_ball, *b_ball]);
                    } else if a_ball.surface().is_some() {
                        soft_contact(b_ball, a_ball, -normal, width, layers, ids);
                    } else if b_ball.surface().is_some() {
                        soft_contact(a_ball, b_ball, normal, width, layers, ids);
                    } else {
                        // Two airborne balls share the separation and a heavily damped normal impulse.
                        let share = b_ball.area() / (a_ball.area() + b_ball.area());
                        let overlap = reach - distance + 0.01;
                        a_ball.center -= normal * (overlap * share);
                        b_ball.center += normal * (overlap * (1.0 - share));
                        if closing > 0.0 {
                            a_ball.velocity -= normal * (closing * 1.15 * share);
                            b_ball.velocity += normal * (closing * 1.15 * (1.0 - share));
                        }
                    }
                }
            }
            for (i, ball) in self.balls.iter_mut().enumerate() {
                if gone[i] || ball.state != SnowballState::Falling {
                    continue;
                }
                let from = previous[i].y - ball.radius;
                let to = ball.center.y - ball.radius;
                let mut hit = None;
                let mut highest = -f32::MAX;
                for (li, layer) in layers.iter().enumerate() {
                    let Some(column) = layer.index(ball.center.x) else { continue };
                    // Do not clamp a landing back into a neighbouring ball at a Ledge end.
                    // A ball without room for its footprint falls past the end instead.
                    if li != 0
                        && (ball.center.x - ball.radius < layer.base().x || ball.center.x + ball.radius > layer.base().x + support_width(layer))
                    {
                        continue;
                    }
                    let surface = seat_height(layer, ball.center.x);
                    let visible = layer.visible[column]
                        && !layer.occluders.iter().any(|b| ball.center.x >= b[0] && ball.center.x < b[2] && surface >= b[1] && surface < b[3]);
                    if (li == 0 || (visible && ball.velocity.y <= 0.0 && from >= layer.previous_base_y + layer.bed[column]))
                        && to <= surface
                        && surface > highest
                    {
                        hit = Some(li);
                        highest = surface;
                    }
                }
                if let Some(li) = hit {
                    // Impact is relative to the supporting Pile, so a moving Ledge can meet a ball.
                    let surface_velocity = layers[li].track_velocity + layers[li].offset_velocity;
                    let impact = (surface_velocity.y - ball.velocity.y).max(0.0);
                    ball.center.y = highest + ball.radius;
                    if impact > HARD_LANDING {
                        gone[i] = true;
                        broken.push(*ball);
                    } else {
                        let on = if li == 0 { Surface::Ground } else { Surface::Ledge(ids[li]) };
                        ball.state = SnowballState::Resting { on };
                        ball.velocity = V2::ZERO;
                        ball.support_origin = layers[li].origin;
                        seat(ball, &layers[li]);
                    }
                }
            }
        }
        let mut i = 0;
        self.balls.retain(|_| {
            let keep = !gone[i];
            i += 1;
            keep
        });
        broken
    }

    /// Returns balls to this Pile, including at its ends, without discarding any area.
    pub fn return_to(&mut self, on: Surface, layer: &mut SnowLayer) {
        self.balls.retain(|ball| {
            if ball.surface() != Some(on) {
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

fn release_velocity(velocity: V2) -> V2 {
    V2::new(velocity.x.clamp(-1500.0, 1500.0), velocity.y.clamp(-1500.0, 1500.0))
}

fn bounce_at_edges(ball: &mut Snowball, width: f32) {
    let margin = ball.radius.min(width * 0.5);
    if ball.center.x < margin {
        ball.center.x = margin;
        if ball.velocity.x < 0.0 {
            ball.velocity.x *= -EDGE_BOUNCE;
        }
    } else if ball.center.x > width - margin {
        ball.center.x = width - margin;
        if ball.velocity.x > 0.0 {
            ball.velocity.x *= -EDGE_BOUNCE;
        }
    }
}

fn soft_contact(ball: &mut Snowball, target: &Snowball, normal: V2, width: f32, layers: &[SnowLayer], ids: &[WindowId]) {
    if normal.y < -0.5 {
        let support = match target.surface() {
            Some(Surface::Ledge(id)) => ids.iter().position(|&known| known == id).map(|i| &layers[i]),
            _ => None,
        };
        land_on_ball(ball, target, width, support);
        return;
    }
    let overlap = ball.radius + target.radius - (ball.center - target.center).length();
    ball.center -= normal * (overlap + 0.01);
    let closing = (ball.velocity - target.velocity).dot(normal);
    if closing > 0.0 {
        ball.velocity -= normal * (closing * 1.15);
    }
    bounce_at_edges(ball, width);
}

/// #38 will replace sliding off with stacking. Until then a soft drop clears the target's
/// circle sideways, picking the inward side at screen edges, and falls beside it without growing.
fn land_on_ball(ball: &mut Snowball, target: &Snowball, width: f32, support: Option<&SnowLayer>) {
    let reach = ball.radius + target.radius + 0.1;
    let direction = if ball.center.x < target.center.x { -1.0 } else { 1.0 };
    let preferred = target.center.x + direction * reach;
    let other = target.center.x - direction * reach;
    let margin = ball.radius.min(width * 0.5);
    let fits = |x| {
        x >= margin
            && x <= width - margin
            && support.is_none_or(|layer| x - ball.radius >= layer.base().x && x + ball.radius <= layer.base().x + support_width(layer))
    };
    ball.center.x = if fits(preferred) {
        preferred
    } else if fits(other) {
        other
    } else {
        preferred.clamp(margin, width - margin)
    };
    ball.velocity = V2::new(0.0, ball.velocity.y.min(0.0));
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
    ball.center.y = seat_height(layer, ball.center.x) + ball.radius;
}

fn seat_height(layer: &SnowLayer, x: f32) -> f32 {
    let at = (x - layer.base().x) / layer.column_width - 0.5;
    // Match the smoothed surface drawn by the renderer.
    let surface = |i: usize| {
        layer.bed[i] + 0.25 * layer.heights[i.saturating_sub(1)] + 0.5 * layer.heights[i] + 0.25 * layer.heights[(i + 1).min(layer.count() - 1)]
    };
    let at = at.clamp(0.0, (layer.count() - 1) as f32);
    let i = at.floor() as usize;
    let height = surface(i) + (surface((i + 1).min(layer.count() - 1)) - surface(i)) * (at - i as f32);
    layer.base().y + height
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
        ball.state = SnowballState::Resting { on: ball.surface().expect("rolling ball has a support") };
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
