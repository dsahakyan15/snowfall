//! A rare Ground visitor. Each visit keeps one target, even if it is rescued.

use crate::geometry::V2;
use crate::play::{SnowballId, Snowman, Surface};
use crate::simulation::Random;

const WALK_SPEED: f32 = 55.0;
const LEAVE_SPEED: f32 = 65.0;
const MARGIN: f32 = 76.0;
const WIND_UP: f32 = 0.45;
const KICK: f32 = 0.18;
const CHEER: f32 = 1.6;
const CONFUSED: f32 = 3.2;
const COOLDOWN: f64 = 20.0 * 60.0;
// The quiet wait plus the cooldown averages about one visit per eligible hour.
const MEAN_WAIT: f64 = 40.0 * 60.0;

/// Values are also the procedural shader's columnCount uniform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum YetiState {
    Arriving = 0,
    WindingUp = 1,
    Kicking = 2,
    Cheering = 3,
    Confused = 4,
    Leaving = 5,
}

/// A furry visitor with no snow area and no input hit region.
#[derive(Clone, Copy, Debug)]
pub struct Yeti {
    /// Feet's horizontal center, in points along the Ground.
    pub x: f32,
    pub facing: f32,
    pub state: YetiState,
    pub state_time: f32,
    pub elapsed: f32,
    target: SnowballId,
    targeted_at: V2,
    entry_direction: f32,
}

impl Yeti {
    fn enter(man: &Snowman, width: f32) -> Self {
        let direction = if man.center.x <= width * 0.5 { 1.0 } else { -1.0 };
        Self {
            x: if direction > 0.0 { -MARGIN } else { width + MARGIN },
            facing: direction,
            state: YetiState::Arriving,
            state_time: 0.0,
            elapsed: 0.0,
            target: man.id,
            targeted_at: man.center,
            entry_direction: direction,
        }
    }

    fn transition(&mut self, state: YetiState) {
        self.state = state;
        self.state_time = 0.0;
    }

    fn pursuing(&self) -> bool {
        matches!(self.state, YetiState::Arriving | YetiState::WindingUp | YetiState::Kicking)
    }

    fn rescue(&mut self, id: SnowballId) {
        if self.target == id && self.pursuing() {
            self.transition(YetiState::Confused);
        }
    }

    /// Consume leftover time across poses so a slow frame cannot lengthen a visit.
    fn advance(&mut self, snowmen: &[Snowman], width: f32, dt: f32) -> (Option<SnowballId>, bool) {
        let target = snowmen.iter().find(|man| man.id == self.target);
        if self.pursuing()
            && target.is_none_or(|man| man.surface() != Some(Surface::Ground) || (man.center - self.targeted_at).length_squared() > 40.0 * 40.0)
        {
            self.transition(YetiState::Confused);
        }
        self.elapsed = (self.elapsed + dt) % 1000.0;
        let mut remaining = dt;
        let mut kicked = None;
        while remaining > 0.0 {
            let duration = match self.state {
                YetiState::Arriving => {
                    let man = target.expect("a missing target was rescued above");
                    let stop = man.center.x - self.entry_direction * (man.radii[0] + 24.0);
                    ((stop - self.x) * self.entry_direction).max(0.0) / WALK_SPEED
                }
                YetiState::WindingUp => (WIND_UP - self.state_time).max(0.0),
                YetiState::Kicking => (KICK - self.state_time).max(0.0),
                YetiState::Cheering => (CHEER - self.state_time).max(0.0),
                YetiState::Confused => (CONFUSED - self.state_time).max(0.0),
                YetiState::Leaving => {
                    let exit = if self.entry_direction > 0.0 { -MARGIN } else { width + MARGIN };
                    ((self.x - exit) * self.entry_direction).max(0.0) / LEAVE_SPEED
                }
            };
            let step = remaining.min(duration);
            match self.state {
                YetiState::Arriving => self.x += self.entry_direction * WALK_SPEED * step,
                YetiState::Leaving => self.x -= self.entry_direction * LEAVE_SPEED * step,
                _ => {}
            }
            self.state_time += step;
            remaining -= step;
            if self.state == YetiState::Confused {
                // Four deliberate glances, rather than a rapid flicker at the render rate.
                self.facing = self.entry_direction * if ((self.state_time / 0.8) as u32).is_multiple_of(2) { 1.0 } else { -1.0 };
            }
            if step < duration {
                break;
            }
            let next = match self.state {
                YetiState::Arriving => YetiState::WindingUp,
                YetiState::WindingUp => YetiState::Kicking,
                YetiState::Kicking => {
                    kicked = Some(self.target);
                    YetiState::Cheering
                }
                YetiState::Cheering | YetiState::Confused => {
                    self.facing = -self.entry_direction;
                    YetiState::Leaving
                }
                YetiState::Leaving => return (kicked, true),
            };
            self.transition(next);
        }
        (kicked, false)
    }
}

pub(crate) struct YetiVisits {
    pub running: Option<Yeti>,
    cooldown: f64,
    wait: Option<f64>,
    /// Visits must not change the wind's or snowball fights' sequence of random choices.
    random: Random,
}

impl YetiVisits {
    pub fn new(random: Random) -> Self {
        Self { running: None, cooldown: 0.0, wait: None, random }
    }

    #[cfg(test)]
    pub fn seed_random(&mut self, seed: u64) {
        self.random = Random::seeded(seed);
        self.wait = None;
    }

    pub fn send(&mut self, snowmen: &[Snowman], width: f32) {
        if self.running.is_some() {
            return;
        }
        // Start at the nearest edge of the nearest eligible Snowman on this screen.
        let nearest = snowmen.iter().filter(|man| man.surface() == Some(Surface::Ground)).min_by(|a, b| {
            let distance = |man: &Snowman| man.center.x.min(width - man.center.x);
            distance(a).total_cmp(&distance(b))
        });
        if let Some(man) = nearest {
            self.running = Some(Yeti::enter(man, width));
            self.cooldown = COOLDOWN;
            self.wait = None;
        }
    }

    /// An exponential wait consumes only eligible simulation time, without drawing random numbers each frame.
    pub fn wait(&mut self, snowmen: &[Snowman], width: f32, dt: f32) {
        if self.running.is_some() {
            return;
        }
        let available = (dt as f64 - self.cooldown).max(0.0);
        self.cooldown = (self.cooldown - dt as f64).max(0.0);
        if available <= 0.0 || !snowmen.iter().any(|man| man.surface() == Some(Surface::Ground)) {
            return;
        }
        let wait = self.wait.get_or_insert_with(|| -(self.random.next() as f64).max(1.0 / (1u32 << 24) as f64).ln() * MEAN_WAIT);
        *wait -= available;
        if *wait <= 0.0 {
            self.send(snowmen, width);
        }
    }

    pub fn advance(&mut self, snowmen: &[Snowman], width: f32, dt: f32) -> Option<(SnowballId, f32)> {
        let yeti = self.running.as_mut()?;
        let direction = yeti.entry_direction;
        let (kicked, left) = yeti.advance(snowmen, width, dt);
        if left {
            self.stop();
        }
        kicked.map(|id| (id, direction))
    }

    /// A grab or screen transfer counts even when it ends before the next simulation step.
    pub fn rescue(&mut self, id: SnowballId) {
        if let Some(yeti) = &mut self.running {
            yeti.rescue(id);
        }
    }

    pub fn stop(&mut self) {
        if self.running.take().is_some() {
            self.cooldown = COOLDOWN;
            self.wait = None;
        }
    }
}
