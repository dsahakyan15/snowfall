//! Snow taken from Piles and rolled into Snowballs, without any platform input or drawing code.

use std::f32::consts::{PI, TAU};
use std::sync::atomic::{AtomicU32, Ordering};

use crate::geometry::V2;
use crate::layer::SnowLayer;
use crate::simulation::{Random, WindowId};

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
const STACK_TOLERANCE: f32 = 1.1;
const HOP_SPEED: f32 = 95.0;
const HOP_GRAVITY: f32 = 500.0;
pub(crate) const IDLE_BEFORE_FIGHT: f64 = 180.0;
const THROW_INTERVAL: f32 = 1.15;
const WIND_UP: f32 = 0.2;
const FIGHT_THROWS: u8 = 9;
pub(crate) const FIGHT_BALL_RADIUS: f32 = 5.5;
const DENT_LIFE: f32 = 5.0;
const MAX_FIGHT_PUFFS: usize = 128;

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
    /// The ball immediately below this one in a Stack.
    pub stacked_on: Option<SnowballId>,
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

/// Three ordered Snowballs packed into one entity. Center is the bottom circle's center.
/// Positions, radii and hop displacement are in points, independent of display DPI.
#[derive(Clone, Copy, Debug)]
pub struct Snowman {
    pub id: SnowballId,
    pub center: V2,
    pub radii: [f32; 3],
    pub velocity: V2,
    /// Uses Resting, Held and Falling; a Snowman never rolls.
    pub state: SnowballState,
    pub hop_offset: f32,
    pub hop_velocity: f32,
    /// Keeps looking toward the other Snowman after a fight.
    pub facing: f32,
    /// Leans back and wobbles while pushed whole by the Snowplough.
    pub tilt: f32,
    pub(crate) dents: [Option<Dent>; 3],
    wind_up: f32,
    idle_for: f64,
    fight_hop: bool,
    support_origin: V2,
    grab_offset: V2,
}

impl Snowman {
    pub fn surface(&self) -> Option<Surface> {
        self.base_ball().surface()
    }

    pub fn area(&self) -> f32 {
        self.radii.iter().map(|r| PI * r * r).sum()
    }

    /// Drawing, grabbing and thrown-ball contacts all use these circles.
    pub fn circles(&self) -> [(V2, f32); 3] {
        let [a, b, c] = self.radii;
        let base = self.center + V2::new(0.0, self.hop_offset);
        let squash = 1.0 - 0.12 * self.wind_up;
        let lean = -self.facing * 4.0 * self.wind_up;
        let pivot = base - V2::new(0.0, a);
        let (s, c_) = self.tilt.sin_cos();
        [(base, a), (base + V2::new(lean, a + b * squash), b * squash), (base + V2::new(lean * 1.5, a + (2.0 * b + c) * squash), c * squash)].map(
            |(center, radius)| {
                let d = center - pivot;
                (pivot + V2::new(c_ * d.x - s * d.y, s * d.x + c_ * d.y), radius)
            },
        )
    }

    pub fn contains(&self, point: V2) -> bool {
        self.circles().iter().any(|&(center, r)| (point - center).length_squared() <= r * r)
    }

    pub fn map_to(&mut self, scale: (f32, f32), offset: (f32, f32)) {
        let mut base = self.base_ball();
        base.map_to(scale, offset);
        self.set_base(base);
        self.idle_for = 0.0;
        self.wind_up = 0.0;
    }

    fn base_ball(&self) -> Snowball {
        Snowball {
            id: self.id,
            center: self.center,
            radius: self.radii[0],
            velocity: self.velocity,
            state: self.state,
            angle: 0.0,
            stacked_on: None,
            support_origin: self.support_origin,
            pushed_at: 0.0,
            grab_offset: self.grab_offset,
        }
    }

    fn set_base(&mut self, base: Snowball) {
        self.center = base.center;
        self.velocity = base.velocity;
        self.state = base.state;
        self.support_origin = base.support_origin;
        self.grab_offset = base.grab_offset;
    }

    fn parts(&self) -> [Snowball; 3] {
        let circles = self.circles();
        std::array::from_fn(|i| Snowball { center: circles[i].0, radius: self.radii[i], ..self.base_ball() })
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Dent {
    pub direction: V2,
    pub radius: f32,
    pub remaining: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct FightBall {
    pub position: V2,
    pub velocity: V2,
    target: SnowballId,
}

pub(crate) struct FightPuff {
    pub position: V2,
    velocity: V2,
    pub size: f32,
    pub remaining: f32,
    pub life: f32,
}

#[derive(Clone, Copy)]
struct Fight {
    pair: [SnowballId; 2],
    elapsed: f32,
    throws: u8,
}

pub(crate) struct SnowPlay {
    pub balls: Vec<Snowball>,
    pub snowmen: Vec<Snowman>,
    pub enabled: bool,
    fight: Option<Fight>,
    pub projectiles: Vec<FightBall>,
    pub puffs: Vec<FightPuff>,
    next_spawn: f64,
}

impl SnowPlay {
    pub fn new() -> Self {
        Self {
            balls: Vec::new(),
            snowmen: Vec::new(),
            enabled: true,
            fight: None,
            projectiles: Vec::with_capacity(2),
            puffs: Vec::with_capacity(MAX_FIGHT_PUFFS),
            next_spawn: 0.0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.balls.is_empty() && self.snowmen.is_empty()
    }

    pub fn held(&self) -> bool {
        self.balls.iter().any(|b| b.state == SnowballState::Held) || self.snowmen.iter().any(|m| m.state == SnowballState::Held)
    }

    pub fn active(&self) -> bool {
        self.balls.iter().any(|b| matches!(b.state, SnowballState::Held | SnowballState::Falling))
            || self
                .snowmen
                .iter()
                .any(|m| matches!(m.state, SnowballState::Held | SnowballState::Falling) || m.hop_offset > 0.0 || m.hop_velocity != 0.0)
    }

    pub fn fighting(&self) -> bool {
        self.fight.is_some()
            || !self.projectiles.is_empty()
            || !self.puffs.is_empty()
            || self.snowmen.iter().any(|m| m.dents.iter().any(Option::is_some))
    }

    #[cfg(test)]
    pub fn fight_turn(&self) -> Option<([SnowballId; 2], u8)> {
        self.fight.map(|f| (f.pair, f.throws))
    }

    /// Being moved or touched starts the wait for another fight over again.
    pub fn disturb(&mut self, id: SnowballId) {
        if self.fight.is_some_and(|f| f.pair.contains(&id)) {
            self.end_fight();
        }
        if let Some(man) = self.snowmen.iter_mut().find(|m| m.id == id) {
            man.idle_for = 0.0;
            man.wind_up = 0.0;
        }
    }

    pub fn disturb_on(&mut self, on: Surface) {
        if self.fight.is_some_and(|f| self.snowmen.iter().any(|m| f.pair.contains(&m.id) && m.surface() == Some(on))) {
            self.end_fight();
        }
        for man in self.snowmen.iter_mut().filter(|m| m.surface() == Some(on)) {
            man.idle_for = 0.0;
            man.wind_up = 0.0;
        }
    }

    fn end_fight(&mut self) {
        if let Some(fight) = self.fight.take() {
            for man in self.snowmen.iter_mut().filter(|m| fight.pair.contains(&m.id)) {
                man.idle_for = 0.0;
                man.wind_up = 0.0;
            }
        }
        while let Some(ball) = self.projectiles.pop() {
            self.puff(ball.position, ball.velocity, 14);
        }
    }

    pub fn clear_fight(&mut self) {
        self.end_fight();
        self.puffs.clear();
    }

    /// Two undisturbed Snowmen pick the nearest partner on this screen.
    pub fn advance_fight(&mut self, layers: &[SnowLayer], width: f32, dt: f32, gravity: f32, random: &mut Random) -> bool {
        let changed = self.fighting();
        for man in &mut self.snowmen {
            for dent in &mut man.dents {
                if let Some(d) = dent {
                    d.remaining -= dt;
                    if d.remaining <= 0.0 {
                        *dent = None;
                    }
                }
            }
            if matches!(man.state, SnowballState::Resting { .. }) && man.hop_offset == 0.0 && man.hop_velocity == 0.0 {
                man.idle_for += dt as f64;
            } else {
                man.idle_for = 0.0;
            }
        }
        for puff in &mut self.puffs {
            puff.position += puff.velocity * dt + V2::new(0.0, -0.5 * gravity * dt * dt);
            puff.velocity.y -= gravity * dt;
            puff.remaining -= dt;
        }
        self.puffs.retain(|p| p.remaining > 0.0);

        if self.fight.is_some_and(|f| {
            f.pair.iter().any(|id| {
                !self.snowmen.iter().any(|m| {
                    m.id == *id && matches!(m.state, SnowballState::Resting { .. }) && (m.fight_hop || (m.hop_offset == 0.0 && m.hop_velocity == 0.0))
                })
            })
        }) {
            self.end_fight();
        }
        if !self.enabled {
            return changed;
        }
        if self.fight.is_none() {
            let mut nearest = None;
            let mut distance = f32::INFINITY;
            for (i, a) in self.snowmen.iter().enumerate().filter(|(_, m)| m.idle_for >= IDLE_BEFORE_FIGHT) {
                for b in self.snowmen[i + 1..].iter().filter(|m| m.idle_for >= IDLE_BEFORE_FIGHT) {
                    let apart = (a.center - b.center).length_squared();
                    if apart < distance {
                        nearest = Some([a.id, b.id]);
                        distance = apart;
                    }
                }
            }
            if let Some(pair) = nearest {
                let a = self.snowmen.iter().find(|m| m.id == pair[0]).unwrap().center;
                let b = self.snowmen.iter().find(|m| m.id == pair[1]).unwrap().center;
                let facing = if b.x >= a.x { 1.0 } else { -1.0 };
                for man in &mut self.snowmen {
                    if man.id == pair[0] {
                        man.facing = facing;
                    } else if man.id == pair[1] {
                        man.facing = -facing;
                    }
                }
                self.fight = Some(Fight { pair, elapsed: 0.0, throws: 0 });
                // The first wind-up begins now, rather than spending the last idle frame on a throw.
                return true;
            }
            return changed;
        }

        let steps = (dt / (1.0 / 240.0)).ceil().max(1.0) as usize;
        let step = dt / steps as f32;
        for _ in 0..steps {
            let Some(mut fight) = self.fight else { break };
            fight.elapsed += step;
            for man in self.snowmen.iter_mut().filter(|m| fight.pair.contains(&m.id)) {
                man.wind_up = 0.0;
            }
            if fight.throws < FIGHT_THROWS {
                let t = fight.elapsed - fight.throws as f32 * THROW_INTERVAL;
                let thrower = fight.pair[fight.throws as usize % 2];
                if t >= WIND_UP {
                    let target = fight.pair[(fight.throws as usize + 1) % 2];
                    self.throw(thrower, target, gravity, random);
                    fight.throws += 1;
                } else if t >= 0.0 {
                    let man = self.snowmen.iter_mut().find(|m| m.id == thrower).unwrap();
                    man.wind_up = (PI * t / WIND_UP).sin();
                }
            }
            self.fight = Some(fight);
            self.fly_fight_balls(layers, width, step, gravity, random);
            if fight.throws == FIGHT_THROWS && self.projectiles.is_empty() {
                self.end_fight();
            }
        }
        true
    }

    fn throw(&mut self, thrower: SnowballId, target: SnowballId, gravity: f32, random: &mut Random) {
        let man = self.snowmen.iter().find(|m| m.id == thrower).unwrap();
        let (middle, radius) = man.circles()[1];
        let position = middle + V2::new(man.facing * (radius + FIGHT_BALL_RADIUS), 0.0);
        let target_man = self.snowmen.iter().find(|m| m.id == target).unwrap();
        let circle = if random.next() < 0.65 { 1 } else { 2 };
        let mut aim = target_man.circles()[circle].0;
        if random.next() < 0.3 {
            aim.x += random.range(-70.0, 70.0);
        }
        let flight = random.range(0.85, 1.1);
        let velocity = (aim - position) / flight + V2::new(0.0, 0.5 * gravity * flight);
        self.projectiles.push(FightBall { position, velocity, target });
    }

    fn fly_fight_balls(&mut self, layers: &[SnowLayer], width: f32, dt: f32, gravity: f32, random: &mut Random) {
        let mut i = 0;
        while i < self.projectiles.len() {
            let ball = &mut self.projectiles[i];
            let from = ball.position;
            ball.position += ball.velocity * dt + V2::new(0.0, -0.5 * gravity * dt * dt);
            ball.velocity.y -= gravity * dt;
            let ball = *ball;
            let hit = self.snowmen.iter().position(|m| m.id == ball.target).and_then(|mi| {
                self.snowmen[mi].circles().iter().enumerate().find_map(|(ci, &(center, radius))| {
                    ((ball.position - center).length_squared() <= (radius + FIGHT_BALL_RADIUS).powi(2)).then_some((mi, ci, center, radius))
                })
            });
            let landed = ball.velocity.y < 0.0
                && layers.iter().enumerate().any(|(li, layer)| {
                    layer.index(ball.position.x).is_some_and(|column| {
                        let top = layer.surface(column);
                        (li == 0 || layer.heights[column] > 0.0)
                            && layer.visible[column]
                            && from.y - FIGHT_BALL_RADIUS >= top
                            && ball.position.y - FIGHT_BALL_RADIUS <= top
                    })
                });
            if let Some((mi, ci, center, radius)) = hit {
                let delta = ball.position - center;
                let direction = delta / delta.length().max(0.001);
                let man = &mut self.snowmen[mi];
                man.hop_velocity = HOP_SPEED;
                man.fight_hop = true;
                man.dents[ci] = Some(Dent { direction, radius: random.range(0.18, 0.28), remaining: DENT_LIFE });
                self.puff(center + direction * radius, ball.velocity, 26);
            } else if !landed
                && ball.position.x >= -FIGHT_BALL_RADIUS
                && ball.position.x <= width + FIGHT_BALL_RADIUS
                && ball.position.y >= -FIGHT_BALL_RADIUS
            {
                // The top stays open, so an arc above the screen can return to its target.
                i += 1;
                continue;
            } else {
                self.puff(ball.position, ball.velocity, 14);
            }
            self.projectiles.swap_remove(i);
        }
    }

    fn puff(&mut self, position: V2, velocity: V2, count: usize) {
        let direction = velocity / velocity.length().max(1.0);
        for i in 0..count.min(MAX_FIGHT_PUFFS - self.puffs.len()) {
            let angle = i as f32 * 2.399963;
            let spread = V2::new(angle.cos(), angle.sin());
            let life = 0.6 + (i * 7 % 13) as f32 / 12.0 * 0.7;
            self.puffs.push(FightPuff {
                position,
                velocity: direction * 65.0 + spread * (35.0 + (i * 17 % 71) as f32) + V2::new(0.0, 90.0),
                size: 2.0 + (i % 4) as f32 * 0.65,
                remaining: life,
                life,
            });
        }
    }

    /// Takes over the cursor's carving only where it is rolling a ball.
    pub fn sweep(&mut self, layer: &mut SnowLayer, on: Surface, point: V2, velocity: V2, dx: f32, clock: f64) -> bool {
        if !self.enabled || self.held() || velocity.x.abs() < MIN_SPEED || dx == 0.0 {
            return false;
        }
        let Some(column) = layer.index(point.x) else { return false };
        if !layer.visible[column] {
            return false;
        }
        let nearby = self
            .balls
            .iter()
            .position(|ball| ball.stacked_on.is_none() && ball.surface() == Some(on) && (point - ball.center).length() <= ball.radius + 18.0);
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
                stacked_on: None,
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
        let mut disturbed = false;
        for ball in self.balls.iter_mut().filter(|ball| ball.surface() == Some(on) && ball.stacked_on.is_none()) {
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
        for man in self.snowmen.iter_mut().filter(|m| m.surface() == Some(on)) {
            man.tilt = 0.0;
            let before = man.center;
            man.center += layer.origin - man.support_origin;
            man.support_origin = layer.origin;
            let mut base = man.base_ball();
            seat(&mut base, layer);
            man.set_base(base);
            if (man.center - before).length_squared() > 0.0001 {
                man.idle_for = 0.0;
                man.wind_up = 0.0;
                disturbed |= self.fight.is_some_and(|f| f.pair.contains(&man.id));
            }
            if dt > 0.0 && (man.hop_velocity != 0.0 || man.hop_offset > 0.0) {
                man.hop_offset += man.hop_velocity * dt - 0.5 * HOP_GRAVITY * dt * dt;
                man.hop_velocity -= HOP_GRAVITY * dt;
                if man.hop_offset <= 0.0 {
                    man.hop_offset = 0.0;
                    man.hop_velocity = 0.0;
                    man.fight_hop = false;
                }
                changed = true;
            }
            // Like resting balls, ignore sub-pixel drift while a shaken Ledge settles.
            changed |= (man.center - before).length_squared() > 0.0001;
        }
        if disturbed {
            self.end_fight();
        }
        self.follow_stacks();
        changed
    }

    /// Pushes only snow play resting on the Ground; grabbing one frees it at once.
    pub fn plough(&mut self, plough: &crate::plough::Snowplough, ground: &SnowLayer, width: f32) -> f64 {
        let blade = plough.blade();
        let direction = plough.direction;
        let touches = |x: f32, radius: f32| {
            let ahead = (x - blade) * direction;
            ahead < radius + 4.0 && ahead > -crate::plough::LENGTH
        };
        let gone = |x: f32, radius: f32| if direction > 0.0 { x - radius > width } else { x + radius < 0.0 };
        let mut removed = 0.0;
        for ball in self.balls.iter_mut().filter(|b| b.surface() == Some(Surface::Ground)) {
            if touches(ball.center.x, ball.radius) {
                let x = blade + direction * (ball.radius + 4.0);
                ball.angle -= (x - ball.center.x) / ball.radius;
                ball.center.x = x;
                ball.center.y = seat_height(ground, x) + ball.radius;
                ball.velocity = V2::ZERO;
                ball.state = SnowballState::Resting { on: Surface::Ground };
                ball.stacked_on = None;
                if gone(x, ball.radius) {
                    removed += ball.area() as f64;
                }
            }
        }
        self.balls.retain(|b| b.surface() != Some(Surface::Ground) || !gone(b.center.x, b.radius));
        let pushed: Vec<_> =
            self.snowmen.iter().filter(|m| m.surface() == Some(Surface::Ground) && touches(m.center.x, m.radii[0])).map(|m| m.id).collect();
        for id in pushed {
            self.disturb(id);
            let man = self.snowmen.iter_mut().find(|m| m.id == id).unwrap();
            let radius = man.radii[0];
            man.center.x = blade + direction * (radius + 4.0);
            man.center.y = seat_height(ground, man.center.x) + radius;
            man.tilt = direction * (0.06 + (plough.elapsed * 14.0).sin() * 0.07);
            if gone(man.center.x, radius) {
                removed += man.area() as f64;
            }
        }
        self.snowmen.retain(|m| m.surface() != Some(Surface::Ground) || !gone(m.center.x, m.radii[0]));
        removed
    }

    /// Detaching either ball dissolves its Stack without creating or losing snow.
    fn follow_stacks(&mut self) {
        for i in 0..self.balls.len() {
            let Some(id) = self.balls[i].stacked_on else { continue };
            let target = self.balls.iter().find(|b| b.id == id).copied();
            if let Some(target) = target.filter(|b| matches!(b.state, SnowballState::Resting { .. })) {
                self.balls[i].center = target.center + V2::new(0.0, target.radius + self.balls[i].radius);
                self.balls[i].state = target.state;
                self.balls[i].support_origin = target.support_origin;
            } else {
                self.balls[i].stacked_on = None;
                self.balls[i].state = SnowballState::Falling;
            }
        }
    }

    fn form_snowmen(&mut self) {
        if !self.enabled {
            return;
        }
        loop {
            let triple = self.balls.iter().find_map(|bottom| {
                if bottom.stacked_on.is_some() || !matches!(bottom.state, SnowballState::Resting { .. }) {
                    return None;
                }
                let middle = self.balls.iter().find(|b| b.stacked_on == Some(bottom.id) && b.radius <= bottom.radius)?;
                let top = self.balls.iter().find(|b| b.stacked_on == Some(middle.id) && b.radius <= middle.radius)?;
                Some([*bottom, *middle, *top])
            });
            let Some([bottom, middle, top]) = triple else { break };
            self.snowmen.push(Snowman {
                id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
                center: bottom.center,
                radii: [bottom.radius, middle.radius, top.radius],
                velocity: V2::ZERO,
                state: bottom.state,
                hop_offset: 0.0,
                hop_velocity: 0.0,
                facing: 1.0,
                tilt: 0.0,
                dents: [None; 3],
                wind_up: 0.0,
                idle_for: 0.0,
                fight_hop: false,
                support_origin: bottom.support_origin,
                grab_offset: V2::ZERO,
            });
            self.balls.retain(|b| ![bottom.id, middle.id, top.id].contains(&b.id));
        }
    }

    /// Fast window motion starts one brief visual hop; a resting Snowman has no animation clock.
    pub fn jolt_on(&mut self, on: Surface, speed: f32) {
        if speed <= HARD_HIT {
            return;
        }
        self.disturb_on(on);
        for man in self.snowmen.iter_mut().filter(|m| m.surface() == Some(on)) {
            man.fight_hop = false;
            if man.hop_offset == 0.0 && man.hop_velocity == 0.0 {
                man.hop_velocity = HOP_SPEED;
            }
        }
    }

    /// A disappearing or shedding Ledge lets its balls fall from where they were.
    pub fn shed_on(&mut self, on: Surface, velocity: V2) {
        self.disturb_on(on);
        for ball in self.balls.iter_mut().filter(|ball| ball.surface() == Some(on)) {
            ball.state = SnowballState::Falling;
            ball.velocity = velocity;
            ball.stacked_on = None;
        }
        for man in self.snowmen.iter_mut().filter(|m| m.surface() == Some(on)) {
            man.center.y += man.hop_offset;
            man.hop_offset = 0.0;
            man.hop_velocity = 0.0;
            man.state = SnowballState::Falling;
            man.velocity = velocity;
        }
    }

    pub fn grab(&mut self, id: SnowballId, point: V2) -> bool {
        if !self.enabled || self.held() {
            return false;
        }
        if let Some(man) = self.snowmen.iter_mut().find(|m| m.id == id) {
            if !man.contains(point) {
                return false;
            }
            man.center.y += man.hop_offset;
            man.hop_offset = 0.0;
            man.hop_velocity = 0.0;
            man.tilt = 0.0;
            man.grab_offset = man.center - point;
            man.state = SnowballState::Held;
            man.velocity = V2::ZERO;
            self.disturb(id);
            return true;
        }
        let Some(ball) = self.balls.iter_mut().find(|ball| ball.id == id) else { return false };
        if (point - ball.center).length_squared() > ball.radius * ball.radius {
            return false;
        }
        ball.stacked_on = None;
        ball.grab_offset = ball.center - point;
        ball.state = SnowballState::Held;
        ball.velocity = V2::ZERO;
        true
    }

    pub fn move_held(&mut self, point: V2) {
        for man in self.snowmen.iter_mut().filter(|m| m.state == SnowballState::Held) {
            man.center = point + man.grab_offset;
        }
        for ball in self.balls.iter_mut().filter(|ball| ball.state == SnowballState::Held) {
            ball.center = point + ball.grab_offset;
        }
    }

    pub fn release(&mut self, velocity: V2) {
        for man in self.snowmen.iter_mut().filter(|m| m.state == SnowballState::Held) {
            man.state = SnowballState::Falling;
            man.velocity = release_velocity(velocity);
        }
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
        if dt <= 0.0
            || (!self.balls.iter().any(|b| b.state == SnowballState::Falling) && !self.snowmen.iter().any(|m| m.state == SnowballState::Falling))
        {
            return broken;
        }
        let radius = self.balls.iter().map(|b| b.radius).chain(self.snowmen.iter().flat_map(|m| m.radii)).fold(START_RADIUS, f32::min).max(0.5);
        let speed = self.balls.iter().map(|b| b.velocity.length()).chain(self.snowmen.iter().map(|m| m.velocity.length())).fold(0.0, f32::max)
            + (gravity + wind.abs()) * dt;
        let steps = (dt / (radius / (2.0 * speed.max(1.0))).min(1.0 / 240.0)).ceil().max(1.0) as usize;
        let step = dt / steps as f32;
        let mut gone = vec![false; self.balls.len()];
        let mut previous = vec![V2::ZERO; self.balls.len()];
        let mut men_gone = vec![false; self.snowmen.len()];
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
            for (mi, man) in self.snowmen.iter_mut().enumerate() {
                if men_gone[mi] {
                    continue;
                }
                if man.state == SnowballState::Falling {
                    let mut base = man.base_ball();
                    let from = base.center.y - base.radius;
                    let acceleration = V2::new(wind * WIND_PUSH * START_RADIUS / base.radius, -gravity);
                    base.center += base.velocity * step + acceleration * (0.5 * step * step);
                    base.velocity += acceleration * step;
                    bounce_at_edges(&mut base, width);
                    land(&mut base, from, layers, ids, true);
                    man.set_base(base);
                }
                if man.state == SnowballState::Held {
                    continue;
                }
                for (bi, ball) in self.balls.iter_mut().enumerate() {
                    if gone[bi] || ball.state != SnowballState::Falling {
                        continue;
                    }
                    for (center, radius) in man.circles() {
                        let delta = center - ball.center;
                        let distance = delta.length();
                        if distance > radius + ball.radius {
                            continue;
                        }
                        let normal = if distance > 0.001 { delta / distance } else { V2::new(1.0, 0.0) };
                        let closing = (ball.velocity - man.velocity).dot(normal);
                        if closing > HARD_HIT {
                            gone[bi] = true;
                            men_gone[mi] = true;
                            broken.push(*ball);
                            broken.extend(man.parts());
                        } else {
                            ball.center -= normal * (radius + ball.radius - distance + 0.01);
                            if closing > 0.0 {
                                ball.velocity -= normal * (closing * 1.15);
                            }
                        }
                        break;
                    }
                    if men_gone[mi] {
                        break;
                    }
                }
            }
            for a in 0..self.balls.len() {
                for b in a + 1..self.balls.len() {
                    if gone[a] || gone[b] {
                        continue;
                    }
                    let a_occupied = self.balls.iter().any(|ball| ball.stacked_on == Some(self.balls[a].id));
                    let b_occupied = self.balls.iter().any(|ball| ball.stacked_on == Some(self.balls[b].id));
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
                        soft_contact(b_ball, a_ball, -normal, width, layers, ids, self.enabled && !a_occupied);
                    } else if b_ball.surface().is_some() {
                        soft_contact(a_ball, b_ball, normal, width, layers, ids, self.enabled && !b_occupied);
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
                if land(ball, from, layers, ids, false) {
                    gone[i] = true;
                    broken.push(*ball);
                }
            }
        }
        let mut i = 0;
        self.balls.retain(|_| {
            let keep = !gone[i];
            i += 1;
            keep
        });
        let mut i = 0;
        self.snowmen.retain(|_| {
            let keep = !men_gone[i];
            i += 1;
            keep
        });
        self.follow_stacks();
        self.form_snowmen();
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

    /// Called for settings and clearing only; monitor carry transfers Snowmen intact.
    pub fn return_snowmen_to(&mut self, on: Surface, layer: &mut SnowLayer) {
        self.snowmen.retain(|man| {
            if man.surface() != Some(on) {
                return true;
            }
            let first = layer.clamped_index(man.center.x - man.radii[0]);
            let last = layer.clamped_index(man.center.x + man.radii[0]);
            let height = man.area() / ((last - first + 1) as f32 * layer.column_width);
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

fn soft_contact(ball: &mut Snowball, target: &Snowball, normal: V2, width: f32, layers: &[SnowLayer], ids: &[WindowId], enabled: bool) {
    if normal.y < -0.5 {
        let support = match target.surface() {
            Some(Surface::Ledge(id)) => ids.iter().position(|&known| known == id).map(|i| &layers[i]),
            _ => None,
        };
        land_on_ball(ball, target, width, support, enabled);
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

/// Soft drops stack within 10% radius tolerance. Promotion requires non-increasing radii.
/// Bigger-on-smaller drops keep sliding off, picking the inward side at screen edges.
fn land_on_ball(ball: &mut Snowball, target: &Snowball, width: f32, support: Option<&SnowLayer>, enabled: bool) {
    if enabled && matches!(target.state, SnowballState::Resting { .. }) && ball.radius <= target.radius * STACK_TOLERANCE {
        ball.center = target.center + V2::new(0.0, target.radius + ball.radius);
        ball.state = target.state;
        ball.velocity = V2::ZERO;
        ball.support_origin = target.support_origin;
        ball.stacked_on = Some(target.id);
        return;
    }
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

/// Returns whether a ball broke; Snowmen always survive surface landings.
fn land(ball: &mut Snowball, from: f32, layers: &[SnowLayer], ids: &[WindowId], safe: bool) -> bool {
    let to = ball.center.y - ball.radius;
    let mut hit = None;
    let mut highest = -f32::MAX;
    for (li, layer) in layers.iter().enumerate() {
        let Some(column) = layer.index(ball.center.x) else { continue };
        // Do not clamp a landing back into a neighbouring ball at a Ledge end.
        // A ball without room for its footprint falls past the end instead.
        if li != 0 && (ball.center.x - ball.radius < layer.base().x || ball.center.x + ball.radius > layer.base().x + support_width(layer)) {
            continue;
        }
        let surface = seat_height(layer, ball.center.x);
        let visible = layer.visible[column]
            && !layer.occluders.iter().any(|b| ball.center.x >= b[0] && ball.center.x < b[2] && surface >= b[1] && surface < b[3]);
        if (li == 0 || (visible && ball.velocity.y <= 0.0 && from >= layer.previous_base_y + layer.bed[column])) && to <= surface && surface > highest
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
        if !safe && impact > HARD_LANDING {
            return true;
        } else {
            let on = if li == 0 { Surface::Ground } else { Surface::Ledge(ids[li]) };
            ball.state = SnowballState::Resting { on };
            ball.velocity = V2::ZERO;
            ball.support_origin = layers[li].origin;
            seat(ball, &layers[li]);
        }
    }
    false
}
