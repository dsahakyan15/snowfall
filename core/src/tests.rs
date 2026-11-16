use std::collections::HashSet;

use crate::geometry::{Rect, V2};

use crate::layer::SnowLayer;
use crate::play::{SnowPlay, Snowball, SnowballState, Surface};
use crate::plough::PloughKind;
use crate::simulation::{Pace, SnowSimulation, Snowfall, Sprite, Wind, WindowSurface};

const DT: f32 = 1.0 / 60.0;
const AWAY: V2 = V2::new(-10_000.0, -10_000.0);

/// Isolates snow play from snowfall and its RNG with a Pile of known area and gentle ends.
fn snowball_pile(depth: f32) -> SnowSimulation {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Off);
    simulation.step(DT, play_cursor(200.0, 300.0 + depth * 0.5));
    let mut pile = SnowLayer::new(V2::new(100.0, 300.0), 500, 2.0, 14.0, true, (0.0, 0.0));
    for i in 0..pile.count() {
        let x = (i as f32 + 0.5) * 2.0;
        pile.set(i, depth.min(x * 0.3).min((1000.0 - x) * 0.3));
    }
    pile.refresh_top();
    simulation.bring_pile(7, pile);
    simulation.update_windows(&[surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    simulation
}

/// The engine's cursor has the same small offset as the visible pointer.
fn play_cursor(x: f32, y: f32) -> V2 {
    V2::new(x - 4.0, y + 8.0)
}

fn roll_snowball(simulation: &mut SnowSimulation, depth: f32) {
    for i in 1..=35 {
        simulation.step(DT, play_cursor(200.0 + i as f32 * 3.0, 300.0 + depth * 0.5));
    }
}

#[test]
fn snowballs_form_in_deep_piles_and_grow_by_the_snow_taken() {
    let mut simulation = snowball_pile(20.0);
    let before = simulation.snow_area();
    simulation.step(DT, play_cursor(203.0, 310.0));
    assert_eq!(simulation.snowballs().len(), 1);
    let initial = simulation.snowballs()[0];
    let pile_before: f32 = piles(&simulation).iter().map(|(_, area)| area).sum();
    for i in 2..=35 {
        simulation.step(DT, play_cursor(200.0 + i as f32 * 3.0, 310.0));
    }
    assert_eq!(simulation.snowballs().len(), 1);
    let ball = &simulation.snowballs()[0];
    assert!(ball.radius > initial.radius && initial.radius >= 6.0);
    assert_ne!(ball.angle, initial.angle);
    assert!(ball.radius <= 40.0);
    let taken = pile_before - piles(&simulation).iter().map(|(_, area)| area).sum::<f32>();
    let growth = ball.area() - initial.area();
    assert!((growth - taken).abs() < taken * 0.02, "growth {growth}, taken {taken}");
    assert!((simulation.snow_area() - before).abs() < before * 0.0001, "snow area changed: {before} -> {}", simulation.snow_area());
    assert_eq!(simulation.sprite_count(), 0, "rolling should not spray snow as well");
}

#[test]
fn snowballs_need_deep_snow_and_the_setting_on() {
    let mut shallow = snowball_pile(5.0);
    roll_snowball(&mut shallow, 5.0);
    assert!(shallow.snowballs().is_empty());
    let mut disabled = snowball_pile(20.0);
    disabled.set_snowballs(false);
    roll_snowball(&mut disabled, 20.0);
    assert!(disabled.snowballs().is_empty());
}

#[test]
fn snowballs_rest_and_allow_the_simulation_to_become_idle() {
    let mut simulation = snowball_pile(20.0);
    roll_snowball(&mut simulation, 20.0);
    let rolling_x = simulation.snowballs()[0].center.x;
    run(&mut simulation, 30.0, AWAY);
    let ball = &simulation.snowballs()[0];
    assert!(ball.center.x > rolling_x, "friction should allow a short roll after release");
    assert_eq!(ball.velocity, V2::ZERO);
    assert_eq!(ball.state, SnowballState::Resting { on: Surface::Ledge(7) });
    assert!(!simulation.is_empty());
    for _ in 0..120 {
        assert!(!simulation.step(DT, AWAY), "a resting ball kept the simulation drawing");
    }
}

#[test]
fn resting_snowballs_ride_with_their_window() {
    let mut simulation = snowball_pile(20.0);
    roll_snowball(&mut simulation, 20.0);
    run(&mut simulation, 30.0, AWAY);
    let before = simulation.snowballs()[0].center;
    // A gentle desktop slide carries the Pile exactly, without shaking it.
    simulation.update_windows(&[surface(7, 125.0, 312.0, 1000.0)], 0.2, &HashSet::new(), &HashSet::new(), true);
    assert!(simulation.step(DT, AWAY));
    let ball = &simulation.snowballs()[0];
    assert!((ball.center.x - before.x - 25.0).abs() < 0.01);
    assert!((ball.center.y - before.y - 12.0).abs() < 0.05);
    assert_eq!(ball.state, SnowballState::Resting { on: Surface::Ledge(7) });
}

#[test]
fn disabling_snowballs_returns_their_area_to_the_pile() {
    let mut simulation = snowball_pile(20.0);
    roll_snowball(&mut simulation, 20.0);
    let before = simulation.snow_area();
    assert!(!simulation.snowballs().is_empty());
    simulation.set_snowballs(false);
    assert!(simulation.snowballs().is_empty());
    assert!((simulation.snow_area() - before).abs() < 0.05);
    assert!(simulation.step(DT, AWAY), "returning the snow needs a redraw");
}

#[test]
fn snowballs_on_a_disappearing_or_jumping_window_fall_then_break_on_hard_landing() {
    for jumps in [false, true] {
        let mut simulation = snowball_pile(20.0);
        roll_snowball(&mut simulation, 20.0);
        run(&mut simulation, 30.0, AWAY);
        // Isolate the ball's conservation from the existing powder and Pile shedding rules.
        let ball = simulation.take_snowball(simulation.snowballs()[0].id).unwrap();
        simulation.drain_pile(7);
        simulation.bring_snowball(ball);
        let before = simulation.snow_area();
        let windows = if jumps { vec![surface(7, 1300.0, 300.0, 1000.0)] } else { vec![] };
        simulation.update_windows(&windows, 1.0 / 120.0, &HashSet::new(), &HashSet::new(), false);
        assert_eq!(simulation.snowballs()[0].state, SnowballState::Falling);
        // Shedding a Pile rounds away sub-grain amounts; the ball itself keeps its area.
        assert!((simulation.snow_area() - before).abs() < before * 0.0001, "snow area {before} -> {}", simulation.snow_area());
        run(&mut simulation, 3.0, AWAY);
        assert!(simulation.snowballs().is_empty());
        assert!((simulation.snow_area() - before).abs() < before * 0.001);
    }
}

fn resting_ball() -> SnowSimulation {
    let mut simulation = snowball_pile(20.0);
    roll_snowball(&mut simulation, 20.0);
    run(&mut simulation, 30.0, AWAY);
    simulation
}

#[test]
fn grabbed_snowballs_fall_onto_the_ledge_under_them_or_the_ground() {
    for (x, on) in [(500.0, Surface::Ledge(7)), (1200.0, Surface::Ground)] {
        let mut simulation = resting_ball();
        let ball = simulation.snowballs()[0];
        assert!(simulation.grab(ball.id, ball.center));
        simulation.move_held(V2::new(x, if on == Surface::Ground { 100.0 } else { 400.0 }));
        assert!(simulation.step(DT, AWAY));
        simulation.release(V2::ZERO);
        assert!(simulation.step(DT, AWAY));
        assert_eq!(simulation.snowballs()[0].state, SnowballState::Falling);
        run(&mut simulation, 3.0, AWAY);
        let dropped = simulation.snowballs()[0];
        assert_eq!(dropped.id, ball.id);
        assert_eq!(dropped.area(), ball.area());
        assert_eq!(dropped.state, SnowballState::Resting { on });
        assert!((dropped.center.x - x).abs() < 0.1);
        assert!(!simulation.step(DT, AWAY));
    }
}

#[test]
fn held_snowballs_keep_the_grab_offset_ignore_windows_and_take_no_snow() {
    let mut simulation = resting_ball();
    let ball = simulation.snowballs()[0];
    let offset = V2::new(2.0, 3.0);
    assert_eq!(simulation.snowball_at(ball.center), Some(ball.id));
    assert_eq!(simulation.snowball_at(ball.center + V2::new(ball.radius + 1.0, 0.0)), None);
    assert!(!simulation.grab(ball.id, AWAY));
    assert!(simulation.grab(ball.id, ball.center - offset));
    let before = simulation.snow_area();
    for i in 0..60 {
        let point = V2::new(400.0 + i as f32, 310.0);
        simulation.move_held(point);
        simulation.update_windows(&[surface(7, 125.0, 312.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), true);
        assert!(simulation.step(DT, point));
        let held = simulation.snowballs()[0];
        assert_eq!(held.state, SnowballState::Held);
        assert_eq!(held.center, point + offset);
        assert_eq!(held.radius, ball.radius);
        assert_eq!(simulation.snowballs().len(), 1);
    }
    assert!((simulation.snow_area() - before).abs() < 0.05);
    simulation.update_windows(&[], DT, &HashSet::new(), &HashSet::new(), false);
    simulation.step(DT, AWAY);
    assert_eq!(simulation.snowballs()[0].state, SnowballState::Held);
}

#[test]
fn held_snowballs_transfer_between_simulations_without_changing_identity_or_area() {
    let mut from = resting_ball();
    let mut to = resting_ball();
    let original = from.snowballs()[0];
    assert_ne!(original.id, to.snowballs()[0].id);
    assert!(from.grab(original.id, original.center - V2::new(3.0, 2.0)));
    let before = from.snow_area() + to.snow_area();
    let mut ball = from.take_snowball(original.id).unwrap();
    assert!(from.take_snowball(original.id).is_none());
    ball.map_to((0.75, 0.75), (-100.0, 20.0));
    to.bring_snowball(ball);
    to.move_held(V2::new(500.0, 600.0));
    let transferred = to.snowballs().iter().find(|ball| ball.id == original.id).unwrap();
    assert_eq!(transferred.center, V2::new(502.25, 601.5));
    assert_eq!(transferred.area(), original.area());
    assert_eq!(transferred.angle, original.angle);
    assert_eq!(transferred.state, SnowballState::Held);
    assert!((from.snow_area() + to.snow_area() - before).abs() < 0.05);
    // A short drop exercises carrying; high drops now break on impact (#37).
    to.move_held(V2::new(500.0, 400.0));
    to.release(V2::ZERO);
    run(&mut to, 3.0, AWAY);
    assert_eq!(to.snowballs().iter().find(|ball| ball.id == original.id).unwrap().state, SnowballState::Resting { on: Surface::Ledge(7) });
}

#[test]
fn falling_snowballs_use_the_highest_visible_ledge_and_clamp_to_the_screen() {
    let mut simulation = resting_ball();
    let ball = simulation.snowballs()[0];
    let mut hidden = surface(8, 100.0, 350.0, 1000.0);
    // Cover this Ledge without adding another supporting edge above it.
    hidden.blockers.push(Rect { x: 200.0, y: 330.0, width: 500.0, height: 100.0 });
    simulation.update_windows(&[hidden, surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    assert!(simulation.grab(ball.id, ball.center));
    simulation.move_held(V2::new(500.0, 450.0));
    simulation.release(V2::ZERO);
    run(&mut simulation, 2.0, AWAY);
    assert_eq!(simulation.snowballs()[0].state, SnowballState::Resting { on: Surface::Ledge(7) });
    simulation.update_windows(&[surface(8, 100.0, 350.0, 1000.0), surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    let ball = simulation.snowballs()[0];
    assert!(simulation.grab(ball.id, ball.center));
    simulation.move_held(V2::new(500.0, 450.0));
    simulation.release(V2::ZERO);
    run(&mut simulation, 2.0, AWAY);
    assert_eq!(simulation.snowballs()[0].state, SnowballState::Resting { on: Surface::Ledge(8) });
    for x in [-500.0, 2000.0] {
        let ball = simulation.snowballs()[0];
        assert!(simulation.grab(ball.id, ball.center));
        simulation.move_held(V2::new(x, 100.0));
        simulation.release(V2::new(x, -200.0));
        run(&mut simulation, 3.0, AWAY);
        let ball = simulation.snowballs()[0];
        assert_eq!(ball.state, SnowballState::Resting { on: Surface::Ground });
        assert!(ball.center.x >= ball.radius && ball.center.x + ball.radius <= simulation.width());
    }
}

#[test]
fn a_snowball_on_an_empty_ledge_is_shed_by_a_fling_with_the_windows_velocity() {
    let mut simulation = resting_ball();
    let ball = simulation.take_snowball(simulation.snowballs()[0].id).unwrap();
    // Isolate the ball: ordinary powder left from rolling and settling may lose area off screen.
    simulation = bare_play();
    simulation.update_windows(&[surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    simulation.bring_snowball(ball);
    let before = ball.area();
    let mut fell = false;
    // A steady fast downward drag pulls the support out from under the ball without a jump.
    for i in 1..=20 {
        simulation.update_windows(&[surface(7, 100.0, 300.0 - i as f32 * 40.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        simulation.step(DT, AWAY);
        if simulation.snowballs()[0].state == SnowballState::Falling {
            assert_ne!(simulation.snowballs()[0].velocity, V2::ZERO);
            fell = true;
            break;
        }
    }
    assert!(fell, "an empty Pile must not prevent a ball from falling with a fling");
    simulation.update_windows(&[], DT, &HashSet::new(), &HashSet::new(), false);
    run(&mut simulation, 2.0, AWAY);
    assert!(simulation.snowballs().is_empty(), "a fast shed breaks on the Ground");
    assert!((simulation.snow_area() - before).abs() < 0.05);
}

#[test]
fn disabling_snowballs_or_clearing_returns_held_and_falling_snow() {
    for held in [false, true] {
        let mut simulation = resting_ball();
        let ball = simulation.snowballs()[0];
        let before = simulation.snow_area();
        assert!(simulation.grab(ball.id, ball.center));
        simulation.move_held(V2::new(700.0, 650.0));
        if !held {
            simulation.release(V2::ZERO);
        }
        simulation.set_snowballs(false);
        assert!(simulation.snowballs().is_empty());
        assert!((simulation.snow_area() - before).abs() < 0.05);
        simulation.step(DT, AWAY);
        assert!((simulation.snow_area() - before).abs() < 0.05);
        simulation.blow_away(1.0, true);
        run(&mut simulation, 5.0, AWAY);
        assert!(simulation.is_empty());
    }
}

#[test]
fn snowballs_return_to_a_pile_carried_between_screens() {
    let mut simulation = snowball_pile(20.0);
    roll_snowball(&mut simulation, 20.0);
    let before = simulation.snow_area();
    let pile = simulation.take_pile(7).unwrap();
    assert!(simulation.snowballs().is_empty());
    let carried = pile.heights.iter().sum::<f32>() * pile.column_width;
    assert!((carried + simulation.snow_area() - before).abs() < 0.05);
}

#[test]
fn snowballs_fade_with_opacity_and_clear_with_the_piles() {
    let mut simulation = snowball_pile(20.0);
    roll_snowball(&mut simulation, 20.0);
    simulation.set_opacity(0.4);
    let mut sprites = vec![Sprite::default(); simulation.snowballs().len()];
    assert_eq!(simulation.write_snowballs(&mut sprites), 1);
    assert_eq!(sprites[0].ball, 1.0);
    assert_eq!(sprites[0].alpha, 0.4);
    assert_eq!(sprites[0].angle, simulation.snowballs()[0].angle);
    simulation.blow_away(1.0, true);
    assert!(simulation.snowballs().is_empty());
    run(&mut simulation, 5.0, AWAY);
    assert!(simulation.is_empty());
}

#[test]
fn snowballs_roll_on_the_ground_stop_at_its_edge_and_keep_the_radius_cap() {
    let mut simulation = snowball_pile(20.0);
    // Route a known Pile to the Ground through the engine's overflow path, without falling flakes or RNG.
    let snow = simulation.drain_pile(7).unwrap();
    simulation.add_to_pile(-1, &snow);
    simulation.update_windows(&[], DT, &HashSet::new(), &HashSet::new(), false);
    simulation.step(DT, play_cursor(200.0, 10.0));
    let before = simulation.snow_area();
    for i in 1..=355 {
        simulation.step(DT, play_cursor(200.0 + i as f32 * 3.0, 10.0));
    }
    assert_eq!(simulation.snowballs().len(), 1);
    run(&mut simulation, 30.0, AWAY);
    let ball = &simulation.snowballs()[0];
    assert!((ball.radius - 40.0).abs() < 0.01);
    assert!(ball.center.x + ball.radius <= simulation.width());
    assert_eq!(ball.state, SnowballState::Resting { on: Surface::Ground });
    assert!((simulation.snow_area() - before).abs() < before * 0.0001);
    assert!(!simulation.step(DT, AWAY));
}

/// Makes a ball through the normal Pile path, then isolates it from snowfall and random Pile shapes.
fn flight_ball(center: V2, radius: f32) -> Snowball {
    let mut source = snowball_pile(20.0);
    source.step(DT, play_cursor(203.0, 310.0));
    let mut ball = source.take_snowball(source.snowballs()[0].id).unwrap();
    ball.center = center;
    ball.radius = radius;
    ball.state = SnowballState::Falling;
    ball.velocity = V2::ZERO;
    ball
}

fn bare_play() -> SnowSimulation {
    SnowSimulation::new(1280.0, 800.0, Snowfall::Off)
}

fn ground_ball(simulation: &mut SnowSimulation, x: f32, radius: f32) -> Snowball {
    let ball = flight_ball(V2::new(x, radius + 1.0), radius);
    simulation.bring_snowball(ball);
    simulation.step(0.05, AWAY);
    *simulation.snowballs().iter().find(|b| b.id == ball.id).unwrap()
}

/// Check conservation both in flight and after every grain has returned to a Pile, then check idle.
fn settle_play(simulation: &mut SnowSimulation, area: f32) {
    for _ in 0..1800 {
        let changed = simulation.step(DT, AWAY);
        assert!((simulation.snow_area() - area).abs() < area * 0.01, "snow area {area} -> {}", simulation.snow_area());
        if !changed && simulation.sprite_count() == 0 {
            assert_eq!(simulation.pace(), Pace::Calm);
            for _ in 0..10 {
                assert!(!simulation.step(DT, AWAY), "settled snow kept drawing");
            }
            return;
        }
    }
    panic!("snow never settled: {} sprites, {:?}", simulation.sprite_count(), simulation.snowballs());
}

#[test]
fn a_thrown_ball_is_ballistic_and_spins_at_different_frame_rates() {
    // Explicit still air isolates the integrator from the ambient breeze, even at Wind::None.
    let ground = SnowLayer::new(V2::ZERO, 641, 2.0, 40.0, false, (0.0, 0.0));
    for rate in [30, 60, 144, 240] {
        let dt = 1.0 / rate as f32;
        let mut play = SnowPlay::new();
        let ball = flight_ball(V2::new(300.0, 600.0), 20.0);
        play.balls.push(ball);
        assert!(play.grab(ball.id, ball.center));
        play.release(V2::new(300.0, 400.0));
        for frame in 1..=rate / 3 {
            assert!(play.fly(std::slice::from_ref(&ground), &[0], 1280.0, dt, 2500.0, 0.0).is_empty());
            let t = frame as f32 * dt;
            let expected = ball.center + V2::new(300.0 * t, 400.0 * t - 1250.0 * t * t);
            assert!((play.balls[0].center - expected).length() < 0.02, "{rate} Hz: {:?} vs {expected:?}", play.balls[0].center);
            assert!((play.balls[0].velocity.y - (400.0 - 2500.0 * t)).abs() < 0.03);
        }
        assert_ne!(play.balls[0].angle, ball.angle);
    }
}

#[test]
fn air_pushes_thrown_balls_downwind_and_larger_balls_catch_less_wind() {
    let displacement = |wind, radius| {
        let mut simulation = bare_play();
        simulation.set_wind(wind);
        let ball = flight_ball(V2::new(400.0, 600.0), radius);
        simulation.bring_snowball(ball);
        assert!(simulation.grab(ball.id, ball.center));
        simulation.release(V2::new(300.0, 400.0));
        run(&mut simulation, 0.3, AWAY);
        simulation.snowballs()[0].center.x
    };
    let still = displacement(Wind::None, 6.0);
    let right = displacement(Wind::StrongRight, 6.0) - still;
    let left = displacement(Wind::StrongLeft, 6.0) - still;
    assert!(right > 1.0 && left < -1.0, "wind moved the ball by {left} / {right}");
    let larger = displacement(Wind::StrongRight, 24.0) - displacement(Wind::None, 24.0);
    assert!((right / larger - 4.0).abs() < 0.03);
}

#[test]
fn hard_ball_hits_break_both_and_conserve_snow_until_idle() {
    for state in [SnowballState::Resting { on: Surface::Ground }, SnowballState::Rolling { on: Surface::Ground }, SnowballState::Falling] {
        let mut simulation = bare_play();
        let target = ground_ball(&mut simulation, 500.0, 20.0);
        let mut target = simulation.take_snowball(target.id).unwrap();
        target.state = state;
        if state == SnowballState::Falling {
            target.center.y += 100.0;
        } else if matches!(state, SnowballState::Rolling { .. }) {
            target.velocity.x = -100.0;
        }
        simulation.bring_snowball(target);
        let thrown = flight_ball(target.center + V2::new(-65.0, 5.0), 20.0);
        simulation.bring_snowball(thrown);
        assert!(simulation.grab(thrown.id, thrown.center));
        let area = simulation.snow_area();
        simulation.release(V2::new(1200.0, 0.0));
        run(&mut simulation, 0.05, AWAY);
        assert!(simulation.snowballs().is_empty(), "target state {state:?}");
        assert!(simulation.sprite_count() >= 12, "no visible burst");
        settle_play(&mut simulation, area);
    }
}

#[test]
fn fast_small_balls_do_not_tunnel_at_a_slow_frame_rate() {
    let mut simulation = bare_play();
    ground_ball(&mut simulation, 500.0, 6.0);
    let ball = flight_ball(V2::new(455.0, 10.0), 6.0);
    simulation.bring_snowball(ball);
    assert!(simulation.grab(ball.id, ball.center));
    let area = simulation.snow_area();
    simulation.release(V2::new(1500.0, 0.0));
    simulation.step(0.05, AWAY);
    assert!(simulation.snowballs().is_empty());
    settle_play(&mut simulation, area);
}

#[test]
fn circle_contacts_use_closing_speed_and_leave_held_balls_alone() {
    let mut simulation = bare_play();
    let target = ground_ball(&mut simulation, 500.0, 20.0);
    let mut play = SnowPlay::new();
    // A fast tangential motion is not a hard normal impact.
    let mut ball = flight_ball(V2::new(461.0, 20.0), 20.0);
    ball.velocity = V2::new(80.0, 1500.0);
    play.balls.extend([ball, target]);
    assert!(play.fly(&[], &[], 1280.0, 1.0 / 240.0, 2500.0, 0.0).is_empty());
    assert!((play.balls[0].center - play.balls[1].center).length() >= 39.99);

    let mut held = target;
    held.state = SnowballState::Held;
    ball.center = V2::new(450.0, 20.0);
    ball.velocity = V2::new(1200.0, 0.0);
    play.balls = vec![ball, held];
    assert!(play.fly(&[], &[], 1280.0, 0.04, 2500.0, 0.0).is_empty());
    assert!((play.balls[0].center.x - 498.0).abs() < 0.01);
    assert_eq!(play.balls[1].state, SnowballState::Held);
}

#[test]
fn a_soft_contact_between_two_airborne_balls_separates_them() {
    let mut play = SnowPlay::new();
    let mut left = flight_ball(V2::new(480.0, 500.0), 20.0);
    let mut right = flight_ball(V2::new(520.0, 500.0), 20.0);
    left.velocity = V2::new(100.0, 0.0);
    right.velocity = V2::new(-100.0, 0.0);
    play.balls.extend([left, right]);
    assert!(play.fly(&[], &[], 1280.0, DT, 2500.0, 0.0).is_empty());
    let (a, b) = (play.balls[0], play.balls[1]);
    assert!((a.center - b.center).length() >= a.radius + b.radius);
    assert!(a.velocity.x < 0.0 && b.velocity.x > 0.0);
    assert!((a.velocity.x + b.velocity.x).abs() < 0.01, "normal impulse changed momentum");
}

#[test]
fn short_drops_rest_on_ground_and_ledges_without_breaking() {
    for on in [Surface::Ground, Surface::Ledge(7)] {
        let mut simulation = bare_play();
        let base = if on == Surface::Ground { 0.0 } else { 300.0 };
        if on != Surface::Ground {
            simulation.update_windows(&[surface(7, 100.0, base, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        }
        let ball = flight_ball(V2::new(500.0, base + 20.0 + 15.0), 20.0);
        simulation.bring_snowball(ball);
        assert!(simulation.grab(ball.id, ball.center));
        simulation.release(V2::ZERO);
        settle_play(&mut simulation, ball.area());
        assert_eq!(simulation.snowballs().len(), 1);
        assert_eq!(simulation.snowballs()[0].state, SnowballState::Resting { on });
        assert_eq!(simulation.snowballs()[0].area(), ball.area());
        assert_eq!(simulation.sprite_count(), 0);
    }
}

#[test]
fn hard_landings_break_into_snow_on_ground_and_ledges_including_screen_edges() {
    for (x, on) in [(500.0, Surface::Ground), (20.0, Surface::Ground), (1260.0, Surface::Ground), (500.0, Surface::Ledge(7))] {
        let mut simulation = bare_play();
        let base = if on == Surface::Ground { 0.0 } else { 300.0 };
        if on != Surface::Ground {
            simulation.update_windows(&[surface(7, 100.0, base, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        }
        let ball = flight_ball(V2::new(x, base + 50.0), 20.0);
        simulation.bring_snowball(ball);
        assert!(simulation.grab(ball.id, ball.center));
        simulation.release(V2::new(0.0, -1200.0));
        simulation.step(0.05, AWAY);
        assert!(simulation.snowballs().is_empty());
        assert!(simulation.sprite_count() > 0);
        assert!((simulation.snow_area() - ball.area()).abs() < 0.01);
        let settled: f32 = piles(&simulation).iter().map(|(_, area)| area).sum();
        assert!((ball.area() * 0.6..ball.area() * 0.7).contains(&settled), "most snow should already form a mound: {settled}");
        settle_play(&mut simulation, ball.area());
        if on != Surface::Ground {
            assert!(on_window(&simulation) > ball.area() * 0.95);
        }
    }
}

#[test]
fn snowballs_broken_in_midair_make_a_mound_on_the_highest_visible_ledge_below() {
    for hidden in [false, true] {
        let mut simulation = bare_play();
        simulation.seed_random(37);
        let mut upper = surface(8, 100.0, 500.0, 1000.0);
        if hidden {
            upper.blockers.push(Rect { x: 100.0, y: 490.0, width: 1000.0, height: 30.0 });
        }
        simulation.update_windows(
            &[surface(9, 100.0, 750.0, 1000.0), surface(7, 100.0, 300.0, 1000.0), upper],
            DT,
            &HashSet::new(),
            &HashSet::new(),
            false,
        );
        let target = flight_ball(V2::new(500.0, 650.0), 20.0);
        let thrown = flight_ball(V2::new(435.0, 650.0), 20.0);
        simulation.bring_snowball(target);
        simulation.bring_snowball(thrown);
        assert!(simulation.grab(thrown.id, thrown.center));
        simulation.release(V2::new(1200.0, 0.0));
        simulation.step(0.05, AWAY);
        assert!(simulation.snowballs().is_empty());
        let area = target.area() + thrown.area();
        let expected_top = if hidden { 300.0 } else { 500.0 };
        for (origin, settled) in piles(&simulation) {
            if origin.y == expected_top {
                assert!((settled - area * 0.65).abs() < 0.01);
            } else {
                assert_eq!(settled, 0.0);
            }
        }
        settle_play(&mut simulation, area);
    }
}

#[test]
fn repeated_snowball_breaks_make_broad_mounds_including_at_screen_edges() {
    let mut simulation = SnowSimulation::new(2048.0, 1120.0, Snowfall::Off);
    simulation.seed_random(37);
    let radius = 40.0;
    let mut area = 0.0;
    let mut buffer = vec![0.0; simulation.column_count()];
    // Repeated hard landings at both edges and the middle, with no snowfall or Melt.
    // Outward velocity also exercises kept grains rescued from beyond the screen.
    for round in 0..10 {
        let (x, vx) = match round % 3 {
            0 => (radius, -1500.0),
            1 => (simulation.width() - radius, 1500.0),
            _ => (simulation.width() * 0.5, 0.0),
        };
        let ball = flight_ball(V2::new(x, 500.0), radius);
        simulation.bring_snowball(ball);
        area += ball.area();
        assert!(simulation.grab(ball.id, ball.center));
        simulation.release(V2::new(vx, -1500.0));
        for _ in 0..600 {
            simulation.step(DT, AWAY);
            assert!((simulation.snow_area() - area).abs() < area * 0.0001);
        }
        assert!(simulation.snowballs().is_empty());
        assert_eq!(simulation.sprite_count(), 0);
        assert!((simulation.snow_area() - area).abs() < area * 0.0001);
    }
    let ground = &simulation.write_layers(&mut buffer)[0];
    let heights = &buffer[ground.offset..ground.offset + ground.count];
    let mean = heights.iter().sum::<f32>() / heights.len() as f32;
    let max = heights.iter().copied().fold(0.0, f32::max);
    println!("10 broken Snowballs: Ground max {max:.2}, mean {mean:.2}, radius {radius:.2}");
    assert!(max <= mean + 2.5 * radius, "Snowball spike: max {max:.2}, mean {mean:.2}, radius {radius:.2}");
}

#[test]
fn two_soft_dropped_balls_stack_without_overlap_even_at_screen_edges() {
    for x in [500.0, 20.0, 1260.0] {
        let mut simulation = bare_play();
        ground_ball(&mut simulation, x, 20.0);
        let ball = flight_ball(V2::new(x, 65.0), 20.0);
        simulation.bring_snowball(ball);
        assert!(simulation.grab(ball.id, ball.center));
        let area = simulation.snow_area();
        simulation.release(V2::ZERO);
        settle_play(&mut simulation, area);
        assert_eq!(simulation.snowballs().len(), 2);
        let (a, b) = (simulation.snowballs()[0], simulation.snowballs()[1]);
        assert_eq!(b.stacked_on, Some(a.id));
        assert_eq!(b.center.x, a.center.x);
        assert!(simulation.snowmen().is_empty());
        assert_eq!(a.state, SnowballState::Resting { on: Surface::Ground });
        assert_eq!(b.state, a.state);
        assert!((a.center - b.center).length() >= a.radius + b.radius - 0.05);
        for ball in [a, b] {
            assert!(ball.center.x >= ball.radius && ball.center.x + ball.radius <= simulation.width());
        }
    }
}

#[test]
fn held_and_flying_balls_stay_interactive_then_return_to_idle() {
    let mut simulation = bare_play();
    let ball = ground_ball(&mut simulation, 500.0, 20.0);
    run(&mut simulation, 1.0, AWAY);
    assert_eq!(simulation.pace(), Pace::Calm);
    assert!(simulation.grab(ball.id, ball.center));
    for _ in 0..120 {
        assert!(simulation.step(1.0 / 144.0, AWAY));
        assert_eq!(simulation.pace(), Pace::Interactive);
    }
    simulation.move_held(V2::new(500.0, 80.0));
    simulation.release(V2::new(200.0, 100.0));
    assert!(simulation.step(1.0 / 144.0, AWAY));
    assert!(simulation.snowballs()[0].center.x > 500.0, "release did not move on the next frame");
    assert_eq!(simulation.pace(), Pace::Interactive);
    settle_play(&mut simulation, ball.area());
}

#[test]
fn a_larger_soft_drop_near_a_ledge_end_slides_inward_without_overlap() {
    let mut simulation = bare_play();
    simulation.update_windows(&[surface(7, 100.0, 300.0, 450.0)], DT, &HashSet::new(), &HashSet::new(), false);
    let target = flight_ball(V2::new(500.0, 321.0), 20.0);
    simulation.bring_snowball(target);
    simulation.step(0.05, AWAY);
    assert_eq!(simulation.snowballs()[0].state, SnowballState::Resting { on: Surface::Ledge(7) });
    let ball = flight_ball(V2::new(500.0, 375.0), 30.0);
    simulation.bring_snowball(ball);
    let area = simulation.snow_area();
    settle_play(&mut simulation, area);
    assert_eq!(simulation.snowballs().len(), 2);
    let (a, b) = (simulation.snowballs()[0], simulation.snowballs()[1]);
    assert_eq!(b.state, a.state);
    assert!(b.center.x < a.center.x);
    assert!((a.center - b.center).length() >= a.radius + b.radius - 0.05);
}

#[test]
fn side_edges_bounce_softly_and_the_top_stays_open() {
    for (x, vx) in [(21.0, -1000.0), (1259.0, 1000.0)] {
        let mut simulation = bare_play();
        let ball = flight_ball(V2::new(x, 700.0), 20.0);
        simulation.bring_snowball(ball);
        assert!(simulation.grab(ball.id, ball.center));
        simulation.release(V2::new(vx, 1500.0));
        simulation.step(DT, AWAY);
        assert!(simulation.snowballs()[0].velocity.x * vx < 0.0);
        assert!((simulation.snowballs()[0].velocity.x.abs() - vx.abs() * 0.2).abs() < 0.1);
        run(&mut simulation, 0.2, AWAY);
        assert!(simulation.snowballs()[0].center.y > simulation.height());
        settle_play(&mut simulation, ball.area());
        assert!(simulation.snowballs().is_empty(), "the return from above the top is a hard landing");
    }
}

/// Build through collisions, rather than constructing a Snowman directly.
fn build_snowman(simulation: &mut SnowSimulation, y: f32) -> crate::play::Snowman {
    let mut top = y;
    for radius in [24.0, 18.0, 12.0] {
        let ball = flight_ball(V2::new(500.0, top + radius + 1.0), radius);
        simulation.bring_snowball(ball);
        let area = simulation.snow_area();
        settle_play(simulation, area);
        top += 2.0 * radius;
    }
    assert!(simulation.snowballs().is_empty());
    assert_eq!(simulation.snowmen().len(), 1);
    simulation.snowmen()[0]
}

fn fight_scene(xs: &[f32]) -> SnowSimulation {
    let mut simulation = bare_play();
    for &x in xs {
        let mut source = bare_play();
        let id = build_snowman(&mut source, 0.0).id;
        let mut man = source.take_snowman(id).unwrap();
        man.center.x = x;
        simulation.bring_snowman(man);
    }
    simulation.seed_random(40);
    simulation
}

fn wait_for_fight(simulation: &mut SnowSimulation, rate: usize) {
    let dt = 1.0 / rate as f32;
    for _ in 1..180 * rate {
        simulation.step(dt, AWAY);
        assert!(simulation.fight_turn().is_none(), "the Snowmen fought before three minutes of idle time");
        assert_eq!(simulation.pace(), Pace::Calm);
    }
    assert!(simulation.step(dt, AWAY));
    assert!(simulation.fight_turn().is_some(), "a calm screen must wake for a fight within one frame");
    assert_eq!(simulation.pace(), Pace::Lively);
}

#[test]
fn idle_snowmen_throw_nine_times_in_turn_without_losing_any_snow() {
    for rate in [24, 30, 144] {
        let mut simulation = fight_scene(&[300.0, 900.0]);
        let area = simulation.snow_area();
        let radii: Vec<_> = simulation.snowmen().iter().map(|m| m.radii).collect();
        wait_for_fight(&mut simulation, rate);
        let pair = simulation.fight_turn().unwrap().0;
        let mut throws = 0;
        let mut elapsed = 0.0;
        let mut hits = 0;
        let dt = 1.0 / rate as f32;
        while simulation.fight_turn().is_some() {
            assert!(simulation.step(dt, AWAY));
            elapsed += dt;
            assert!(elapsed < 12.0, "the fight never ended");
            assert_eq!(simulation.snowmen().len(), 2);
            assert!((simulation.snow_area() - area).abs() < 0.001);
            assert_eq!(simulation.snowmen().iter().map(|m| m.radii).collect::<Vec<_>>(), radii);
            if let Some((actual_pair, count)) = simulation.fight_turn() {
                assert_eq!(actual_pair, pair, "only one pair fights at a time");
                if count != throws {
                    assert_eq!(count, throws + 1);
                    let expected = 0.2 + throws as f32 * 1.15;
                    assert!((elapsed - expected).abs() < dt * 2.0, "throw {count} at {elapsed}, expected {expected}");
                    let mut sprites = vec![Sprite::default(); simulation.sprite_count()];
                    simulation.write_sprites(&mut sprites);
                    let projectile = sprites.iter().filter(|s| s.ball == 1.0).min_by(|a, b| a.position.y.total_cmp(&b.position.y)).unwrap();
                    let thrower = &simulation.snowmen()[throws as usize % 2];
                    assert!((projectile.position.x - thrower.center.x).abs() < 70.0, "the wrong Snowman threw");
                    assert_eq!(projectile.size, 11.0);
                    throws = count;
                }
            }
            hits += simulation.snowmen().iter().flat_map(|m| m.dents).filter(Option::is_some).count();
        }
        assert_eq!(throws, 9);
        assert!(hits > 0);
        assert!(elapsed > 9.4 && elapsed < 11.5);
        assert_eq!(simulation.snowmen()[0].facing, 1.0);
        assert_eq!(simulation.snowmen()[1].facing, -1.0);
        run(&mut simulation, 6.0, AWAY);
        assert_eq!(simulation.sprite_count(), 0);
        assert_eq!(simulation.pace(), Pace::Calm);
        assert!(!simulation.step(DT, AWAY));
        assert!(simulation.snowmen().iter().all(|m| m.dents.iter().all(Option::is_none)));
        assert_eq!(simulation.snowmen()[1].facing, -1.0);
        assert!((simulation.snow_area() - area).abs() < 0.001);
        // Both wait three more minutes after the last projectile resolves.
        run(&mut simulation, 173.0, AWAY);
        assert!(simulation.fight_turn().is_none());
        run(&mut simulation, 2.0, AWAY);
        assert!(simulation.fight_turn().is_some());
    }
}

#[test]
fn grabbing_either_fighter_stops_the_fight_and_restarts_the_wait() {
    for index in 0..2 {
        let mut simulation = fight_scene(&[300.0, 900.0]);
        wait_for_fight(&mut simulation, 30);
        run(&mut simulation, 0.4, AWAY);
        let man = simulation.snowmen()[index];
        assert!(simulation.grab(man.id, man.circles()[2].0));
        assert!(simulation.fight_turn().is_none(), "grabbing must stop the fight before the next step");
        simulation.move_held(man.circles()[2].0 + V2::new(30.0, 0.0));
        run(&mut simulation, 181.0, AWAY);
        assert!(simulation.fight_turn().is_none(), "a Held Snowman cannot join a fight");
        simulation.release(V2::ZERO);
        run(&mut simulation, 1.0, AWAY);
        assert!(matches!(simulation.snowmen()[index].state, SnowballState::Resting { .. }));
        run(&mut simulation, 178.0, AWAY);
        assert!(simulation.fight_turn().is_none());
        run(&mut simulation, 2.0, AWAY);
        assert!(simulation.fight_turn().is_some());
    }
}

#[test]
fn one_snowman_on_each_screen_never_fights() {
    let mut a = fight_scene(&[300.0]);
    let mut b = fight_scene(&[900.0]);
    for _ in 0..200 * 24 {
        a.step(1.0 / 24.0, AWAY);
        b.step(1.0 / 24.0, AWAY);
        assert!(a.fight_turn().is_none() && b.fight_turn().is_none());
    }
    assert_eq!(a.pace(), Pace::Calm);
    assert_eq!(b.pace(), Pace::Calm);
}

#[test]
fn only_the_nearest_idle_pair_fights_and_carrying_either_away_stops_it() {
    let mut simulation = fight_scene(&[150.0, 800.0, 950.0]);
    wait_for_fight(&mut simulation, 30);
    assert_eq!(simulation.fight_turn().unwrap().0, [simulation.snowmen()[1].id, simulation.snowmen()[2].id]);
    let bystander = simulation.snowmen()[0];
    assert!(simulation.grab(bystander.id, bystander.center));
    simulation.move_held(bystander.center + V2::new(0.0, 10.0));
    simulation.release(V2::ZERO);
    run(&mut simulation, 0.5, AWAY);
    assert!(simulation.fight_turn().is_some(), "moving a bystander must leave the fighters alone");
    let id = simulation.snowmen()[2].id;
    let man = simulation.take_snowman(id).unwrap();
    assert!(simulation.fight_turn().is_none());
    let mut other = bare_play();
    other.bring_snowman(man);
    simulation.step(DT, AWAY);
    other.step(DT, AWAY);
    assert!(simulation.fight_turn().is_none() && other.fight_turn().is_none());
}

#[test]
fn fight_dents_shrink_and_disappear_five_seconds_after_the_hit() {
    let mut simulation = fight_scene(&[300.0, 900.0]);
    wait_for_fight(&mut simulation, 30);
    let (id, circle, dent) = loop {
        simulation.step(DT, AWAY);
        let hit = simulation.snowmen().iter().find_map(|m| m.dents.iter().enumerate().find_map(|(i, d)| d.map(|d| (m.id, i, d))));
        if let Some(hit) = hit {
            break hit;
        }
        assert!(simulation.fight_turn().is_some(), "the seeded fight should hit a Snowman");
    };
    assert_eq!(dent.remaining, 5.0);
    assert!((0.18..=0.28).contains(&dent.radius));
    assert!((dent.direction.length() - 1.0).abs() < 0.001);
    let target = *simulation.snowmen().iter().find(|m| m.id == id).unwrap();
    assert!(target.hop_velocity > 0.0);
    // Stop later hits replacing this dent; dents heal even while a Snowman is held.
    assert!(simulation.grab(id, target.center + V2::new(0.0, target.hop_offset)));
    run(&mut simulation, 2.5, AWAY);
    let target = simulation.snowmen().iter().find(|m| m.id == id).unwrap();
    assert!((target.dents[circle].unwrap().remaining - 2.5).abs() < DT * 2.0);
    let mut sprites = [Sprite::default(); 6];
    simulation.write_snowmen(&mut sprites);
    let index = simulation.snowmen().iter().position(|m| m.id == id).unwrap() * 3 + circle;
    assert!((sprites[index].lump - 0.5).abs() < 0.01);
    assert_eq!(sprites[index].tint, [dent.direction.x, dent.direction.y, dent.radius]);
    run(&mut simulation, 2.5 + DT * 2.0, AWAY);
    assert!(simulation.snowmen().iter().find(|m| m.id == id).unwrap().dents[circle].is_none());
}

#[test]
fn a_winding_up_snowman_draws_and_grabs_the_same_circles() {
    let mut simulation = fight_scene(&[300.0, 900.0]);
    wait_for_fight(&mut simulation, 30);
    let before = simulation.snowmen()[0];
    simulation.step(0.1, AWAY);
    let winding = simulation.snowmen()[0];
    let mut sprites = [Sprite::default(); 6];
    assert_eq!(simulation.write_snowmen(&mut sprites), 6);
    for (sprite, (center, radius)) in sprites.iter().zip(winding.circles()) {
        assert_eq!(sprite.position, center);
        assert_eq!(sprite.size, radius * 2.0);
        assert_eq!(simulation.snowman_at(center), Some(winding.id));
    }
    assert_eq!(winding.area(), before.area());
    assert!(winding.circles()[2].0.x < before.circles()[2].0.x);
    assert!(winding.circles()[2].1 < before.circles()[2].1);
    assert!(simulation.grab(winding.id, winding.circles()[2].0));
    assert!(simulation.fight_turn().is_none());

    // A real thrown ball can still break a fighter, returning its full unsquashed area.
    let mut simulation = fight_scene(&[300.0, 900.0]);
    wait_for_fight(&mut simulation, 30);
    simulation.step(0.1, AWAY);
    let (center, radius) = simulation.snowmen()[0].circles()[2];
    let mut thrown = flight_ball(center - V2::new(radius + 10.0, 0.0), 6.0);
    thrown.velocity = V2::new(1200.0, 0.0);
    simulation.bring_snowball(thrown);
    let area = simulation.snow_area();
    simulation.step(DT, AWAY);
    assert_eq!(simulation.snowmen().len(), 1);
    assert!(simulation.fight_turn().is_none());
    assert!((simulation.snow_area() - area).abs() < 0.01);
}

#[test]
fn fight_balls_ignore_empty_windows_and_hits_and_misses_make_cosmetic_puffs() {
    let mut saw_hit = false;
    let mut saw_miss = false;
    for seed in [33, 40] {
        let mut simulation = fight_scene(&[300.0, 900.0]);
        // An empty window crosses the arc, but only Piles and the Ground stop these balls.
        simulation.update_windows(&[surface(7, 100.0, 200.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        simulation.seed_random(seed);
        wait_for_fight(&mut simulation, 30);
        // Look at the first resolution, before any other puff exists.
        let mut puff_count = 0;
        for _ in 0..480 {
            simulation.step(1.0 / 240.0, AWAY);
            let mut sprites = vec![Sprite::default(); simulation.sprite_count()];
            simulation.write_sprites(&mut sprites);
            for sprite in &sprites {
                assert_eq!(simulation.snowball_at(sprite.position), None, "a fight effect must not be grabbable");
            }
            puff_count = sprites.iter().filter(|s| s.ball == 0.0).count();
            if puff_count > 0 {
                let hit = simulation.snowmen().iter().any(|m| m.dents.iter().any(Option::is_some));
                assert_eq!(puff_count, if hit { 26 } else { 14 });
                saw_hit |= hit;
                saw_miss |= !hit;
                break;
            }
        }
        assert!(puff_count > 0);
        assert_eq!(piles(&simulation).iter().map(|(_, area)| area).sum::<f32>(), 0.0);
    }
    assert!(saw_hit && saw_miss, "the seeded throws must exercise both hits and misses");
}

#[test]
fn a_fighters_ledge_moving_or_disappearing_ends_the_fight_at_once() {
    for disappears in [false, true] {
        let mut simulation = fight_scene(&[300.0, 900.0]);
        simulation.update_windows(&[surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        let mut man = simulation.take_snowman(simulation.snowmen()[0].id).unwrap();
        man.center.y += 300.0;
        man.state = SnowballState::Falling;
        simulation.bring_snowman(man);
        run(&mut simulation, 1.0, AWAY);
        assert_eq!(simulation.snowmen()[1].surface(), Some(Surface::Ledge(7)));
        run(&mut simulation, 180.0, AWAY);
        assert!(simulation.fight_turn().is_some());
        if disappears {
            simulation.update_windows(&[], DT, &HashSet::new(), &HashSet::new(), false);
        } else {
            simulation.update_windows(&[surface(7, 101.0, 300.0, 1000.0)], 0.5, &HashSet::new(), &HashSet::new(), false);
        }
        assert!(simulation.fight_turn().is_none());
    }
}

#[test]
fn disabling_snowballs_and_clearing_snow_remove_fight_effects() {
    for clear in [false, true] {
        let mut simulation = fight_scene(&[300.0, 900.0]);
        wait_for_fight(&mut simulation, 30);
        run(&mut simulation, 0.4, AWAY);
        assert!(simulation.sprite_count() > 0);
        if clear {
            simulation.blow_away(1.0, true);
        } else {
            let area = simulation.snow_area();
            simulation.set_snowballs(false);
            assert!((simulation.snow_area() - area).abs() < 0.01, "snow area {area} -> {}", simulation.snow_area());
        }
        assert!(simulation.fight_turn().is_none());
        assert!(simulation.snowmen().is_empty());
        assert_eq!(simulation.sprite_count(), 0);
    }
}

#[test]
fn three_ordered_balls_form_one_snowman_with_their_exact_area_and_a_new_id() {
    let mut simulation = bare_play();
    let bottom = ground_ball(&mut simulation, 500.0, 24.0);
    let middle = flight_ball(V2::new(500.0, 67.0), 18.0);
    simulation.bring_snowball(middle);
    settle_play(&mut simulation, bottom.area() + middle.area());
    assert_eq!(simulation.snowballs()[1].stacked_on, Some(bottom.id));
    assert!(simulation.snowmen().is_empty());
    let top = flight_ball(V2::new(500.0, 97.0), 12.0);
    simulation.bring_snowball(top);
    let area = bottom.area() + middle.area() + top.area();
    settle_play(&mut simulation, area);
    assert!(simulation.snowballs().is_empty());
    let man = simulation.snowmen()[0];
    assert_eq!(man.radii, [24.0, 18.0, 12.0]);
    assert!(![bottom.id, middle.id, top.id].contains(&man.id));
    assert!((man.area() - area).abs() < 0.01);
    assert_eq!(man.surface(), Some(Surface::Ground));
}

#[test]
fn unordered_balls_cannot_form_a_snowman_even_within_the_stack_tolerance() {
    for radius in [21.0, 30.0] {
        let mut simulation = bare_play();
        ground_ball(&mut simulation, 500.0, 20.0);
        let middle = flight_ball(V2::new(500.0, 41.0 + radius), radius);
        simulation.bring_snowball(middle);
        let area = simulation.snow_area();
        settle_play(&mut simulation, area);
        let middle = simulation.snowballs()[1];
        assert_eq!(middle.stacked_on.is_some(), radius <= 22.0);
        let top = flight_ball(middle.center + V2::new(0.0, radius + 12.0 + 1.0), 12.0);
        simulation.bring_snowball(top);
        let area = simulation.snow_area();
        settle_play(&mut simulation, area);
        assert!(simulation.snowmen().is_empty());
        assert_eq!(simulation.snowballs().len(), 3);
    }
}

#[test]
fn snowmen_ride_their_ledge_hop_on_fast_motion_then_become_idle() {
    let mut simulation = bare_play();
    simulation.update_windows(&[surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    let man = build_snowman(&mut simulation, 300.0);
    simulation.update_windows(&[surface(7, 125.0, 312.0, 1000.0)], 0.2, &HashSet::new(), &HashSet::new(), true);
    simulation.step(DT, AWAY);
    let moved = simulation.snowmen()[0];
    assert!((moved.center - man.center - V2::new(25.0, 12.0)).length() < 0.05);
    let mut hopped = false;
    for i in 0..20 {
        let left = if i % 2 == 0 { 145.0 } else { 105.0 };
        simulation.update_windows(&[surface(7, left, 312.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        simulation.step(DT, AWAY);
        hopped |= simulation.snowmen()[0].hop_offset > 0.0;
    }
    assert!(hopped, "fast back-and-forth motion should trigger a hop");
    settle_play(&mut simulation, man.area());
    let man = simulation.snowmen()[0];
    assert_eq!(man.state, SnowballState::Resting { on: Surface::Ledge(7) });
    assert_eq!((man.hop_offset, man.hop_velocity), (0.0, 0.0));
}

#[test]
fn snowmen_survive_closed_jumping_and_shedding_windows_and_land_on_ground() {
    for event in 0..3 {
        let mut simulation = bare_play();
        simulation.update_windows(&[surface(7, 100.0, 500.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
        let man = build_snowman(&mut simulation, 500.0);
        match event {
            0 => simulation.update_windows(&[], DT, &HashSet::new(), &HashSet::new(), false),
            1 => simulation.update_windows(&[surface(7, 900.0, 750.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false),
            _ => {
                // Same vertical fling used to test balls on empty Ledges.
                for i in 1..=15 {
                    simulation.update_windows(&[surface(7, 100.0, 500.0 - i as f32 * 40.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
                    simulation.step(DT, AWAY);
                    if simulation.snowmen()[0].state == SnowballState::Falling {
                        break;
                    }
                }
            }
        }
        assert_eq!(simulation.snowmen()[0].state, SnowballState::Falling);
        simulation.update_windows(&[], DT, &HashSet::new(), &HashSet::new(), false);
        settle_play(&mut simulation, man.area());
        assert_eq!(simulation.snowmen()[0].state, SnowballState::Resting { on: Surface::Ground });
        assert_eq!(simulation.snowmen()[0].id, man.id);
    }
}

#[test]
fn every_snowman_circle_can_be_grabbed_carried_between_dpi_spaces_and_dropped() {
    for circle in 0..3 {
        let mut from = bare_play();
        let mut to = bare_play();
        let man = build_snowman(&mut from, 0.0);
        let point = man.circles()[circle].0 + V2::new(2.0, 1.0);
        assert_eq!(from.snowman_at(point), Some(man.id));
        assert_eq!(from.snowman_at(man.center + V2::new(25.0, 0.0)), None);
        assert!(from.grab(man.id, point));
        assert!(!from.grab(man.id, point));
        let mut carried = from.take_snowman(man.id).unwrap();
        carried.map_to((2.0 / 3.0, 2.0 / 3.0), (100.0, 50.0));
        to.bring_snowman(carried);
        let pointer = V2::new(650.0, 450.0);
        to.move_held(pointer);
        let held = to.snowmen()[0];
        let offset = man.center - point;
        assert!((held.center - pointer - offset * (2.0 / 3.0)).length() < 0.001);
        assert_eq!(held.id, man.id);
        assert_eq!(held.radii, man.radii);
        assert_eq!(held.area(), man.area());
        assert!(from.snowmen().is_empty());
        to.release(V2::new(120.0, 20.0));
        settle_play(&mut to, man.area());
        assert_eq!(to.snowmen()[0].surface(), Some(Surface::Ground));
    }
}

#[test]
fn hard_thrown_ball_hits_break_every_snowman_circle_and_conserve_snow() {
    for circle in 0..3 {
        for dt in [1.0 / 30.0, 1.0 / 144.0] {
            let mut simulation = bare_play();
            let man = build_snowman(&mut simulation, 0.0);
            let (center, radius) = man.circles()[circle];
            let mut thrown = flight_ball(center - V2::new(radius + 10.0, 0.0), 6.0);
            thrown.velocity = V2::new(1200.0, 0.0);
            simulation.bring_snowball(thrown);
            let area = simulation.snow_area();
            simulation.step(dt, AWAY);
            assert!(simulation.snowmen().is_empty());
            assert!(simulation.snowballs().is_empty());
            assert!(simulation.sprite_count() > 0);
            settle_play(&mut simulation, area);
        }
    }
}

#[test]
fn snowman_sprites_and_grabbing_share_circles_opacity_and_occluders() {
    let mut simulation = bare_play();
    simulation.update_windows(&[surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    let man = build_snowman(&mut simulation, 300.0);
    simulation.set_opacity(0.4);
    let mut sprites = [Sprite::default(); 3];
    assert_eq!(simulation.write_snowmen(&mut sprites), 3);
    for (sprite, (center, r)) in sprites.iter().zip(man.circles()) {
        assert_eq!(sprite.position, center);
        assert_eq!(sprite.size, r * 2.0);
        assert_eq!(sprite.alpha, 0.4);
    }
    assert_eq!(sprites.map(|s| s.ball), [1.0, 2.0, 3.0]);
    let head = man.circles()[2].0;
    let mut ledge = surface(7, 100.0, 300.0, 1000.0);
    ledge.blockers.push(Rect { x: head.x - 20.0, y: head.y - 20.0, width: 40.0, height: 40.0 });
    simulation.update_windows(&[ledge], DT, &HashSet::new(), &HashSet::new(), false);
    assert_eq!(simulation.snowman_at(head), None);
    assert!(!simulation.grab(man.id, head));
    assert_eq!(simulation.snowman_at(man.center), Some(man.id));
}

#[test]
fn disabling_or_clearing_snowmen_returns_all_their_snow_in_every_state() {
    for state in 0..3 {
        let mut simulation = bare_play();
        let man = build_snowman(&mut simulation, 0.0);
        if state > 0 {
            assert!(simulation.grab(man.id, man.center));
            simulation.move_held(V2::new(600.0, 400.0));
            if state == 2 {
                simulation.release(V2::ZERO);
            }
        }
        simulation.set_snowballs(false);
        assert!(!simulation.grab(man.id, man.center));
        assert!(simulation.snowmen().is_empty());
        settle_play(&mut simulation, man.area());
    }
    let mut simulation = bare_play();
    build_snowman(&mut simulation, 0.0);
    simulation.blow_away(1.0, true);
    assert!(simulation.snowmen().is_empty());
}
#[test]
fn grabbing_either_ball_detaches_a_stack_and_keeps_both_balls() {
    for grab_top in [false, true] {
        let mut simulation = bare_play();
        let bottom = ground_ball(&mut simulation, 500.0, 24.0);
        let top = flight_ball(V2::new(500.0, 67.0), 18.0);
        simulation.bring_snowball(top);
        let area = simulation.snow_area();
        settle_play(&mut simulation, area);
        let target = simulation.snowballs()[usize::from(grab_top)];
        assert!(simulation.grab(target.id, target.center));
        simulation.move_held(target.center + V2::new(150.0, 80.0));
        simulation.step(DT, AWAY);
        assert!(simulation.snowballs().iter().all(|ball| ball.stacked_on.is_none()));
        simulation.release(V2::ZERO);
        settle_play(&mut simulation, area);
        assert_eq!(simulation.snowballs().len(), 2);
        assert!(simulation.snowballs().iter().any(|ball| ball.id == bottom.id));
        assert!(simulation.snowmen().is_empty());
    }
}

#[test]
fn a_snowman_transfers_with_its_ledge_without_turning_back_into_a_pile() {
    let mut from = bare_play();
    from.update_windows(&[surface(7, 100.0, 300.0, 1000.0)], DT, &HashSet::new(), &HashSet::new(), false);
    let man = build_snowman(&mut from, 300.0);
    let mut carried = from.take_snowman(man.id).unwrap();
    let pile = from.take_pile(7).unwrap();
    assert!(from.snowmen().is_empty());
    assert_eq!(from.snow_area(), 0.0);
    carried.map_to((2.0 / 3.0, 2.0 / 3.0), (100.0, 50.0));
    let mut to = bare_play();
    let left = 100.0 * (2.0 / 3.0) + 100.0;
    to.update_windows(&[surface(7, left, 250.0, 1000.0 * (2.0 / 3.0))], DT, &HashSet::new(), &HashSet::new(), false);
    to.bring_pile(7, pile);
    to.bring_snowman(carried);
    settle_play(&mut to, man.area());
    let arrived = to.snowmen()[0];
    assert_eq!(arrived.id, man.id);
    assert_eq!(arrived.radii, man.radii);
    assert!((arrived.center.x - (man.center.x * (2.0 / 3.0) + 100.0)).abs() < 0.01);
    assert!((arrived.center.y - 274.0).abs() < 0.05);
    to.update_windows(&[surface(7, left + 15.0, 256.0, 1000.0 * (2.0 / 3.0))], 0.2, &HashSet::new(), &HashSet::new(), true);
    to.step(DT, AWAY);
    assert!((to.snowmen()[0].center - arrived.center - V2::new(15.0, 6.0)).length() < 0.05);
}

#[test]
fn snowmen_survive_soft_ball_contacts_and_held_snowmen_ignore_hits() {
    for held in [false, true] {
        let mut simulation = bare_play();
        let man = build_snowman(&mut simulation, 0.0);
        if held {
            assert!(simulation.grab(man.id, man.center));
        }
        let (center, r) = man.circles()[1];
        let mut ball = flight_ball(center - V2::new(r + 7.0, 0.0), 6.0);
        ball.velocity = V2::new(if held { 1200.0 } else { 200.0 }, 0.0);
        simulation.bring_snowball(ball);
        let area = simulation.snow_area();
        simulation.step(DT, AWAY);
        assert_eq!(simulation.snowmen().len(), 1);
        assert_eq!(simulation.snowballs().len(), 1);
        assert!((simulation.snow_area() - area).abs() < 0.01);
    }
}

#[test]
fn disabled_snow_play_cannot_stack_existing_balls() {
    let mut source = bare_play();
    let bottom = ground_ball(&mut source, 500.0, 24.0);
    let top = flight_ball(V2::new(500.0, 67.0), 18.0);
    let mut play = SnowPlay::new();
    play.enabled = false;
    play.balls.extend([bottom, top]);
    let mut ground = SnowLayer::new(V2::ZERO, 641, 2.0, 40.0, false, (0.0, 0.0));
    ground.right = 1280.0;
    assert!(play.fly(&[ground], &[0], 1280.0, 0.1, 2500.0, 0.0).is_empty());
    assert!(play.balls.iter().all(|ball| ball.stacked_on.is_none()));
    assert!(play.snowmen.is_empty());
}

fn run(simulation: &mut SnowSimulation, seconds: f32, cursor: V2) {
    for _ in 0..(seconds / DT) as usize {
        simulation.step(DT, cursor);
    }
}

fn surface(id: isize, x: f32, top: f32, width: f32) -> WindowSurface {
    WindowSurface { id, frame: Rect { x, y: top - 300.0, width, height: 300.0 }, blockers: Vec::new(), corner_radius: 8.0, corner_exponent: 2.5 }
}

/// Snow area per layer, from the smoothed surfaces the renderer gets.
fn piles(simulation: &SnowSimulation) -> Vec<(V2, f32)> {
    let mut buffer = vec![0.0; simulation.column_count()];
    let layers = simulation.write_layers(&mut buffer);
    layers
        .iter()
        .map(|pile| {
            let (surface, bed) = buffer[pile.offset..pile.offset + 2 * pile.count].split_at(pile.count);
            (pile.origin, surface.iter().zip(bed).map(|(s, b)| s - b).sum::<f32>() * 2.0)
        })
        .collect()
}

fn assert_finite(simulation: &SnowSimulation) {
    let mut sprites = vec![Sprite::default(); simulation.sprite_count()];
    let count = simulation.write_sprites(&mut sprites);
    for sprite in &sprites[..count] {
        assert!(sprite.position.x.is_finite() && sprite.position.y.is_finite() && sprite.size.is_finite() && sprite.alpha.is_finite());
    }
    let mut buffer = vec![0.0; simulation.column_count()];
    simulation.write_layers(&mut buffer);
    assert!(buffer.iter().all(|h| h.is_finite()));
}

#[test]
fn snow_falls_and_settles_on_the_ground() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    run(&mut simulation, 1.0, AWAY);
    assert!(simulation.sprite_count() > 500, "flakes: {}", simulation.sprite_count());
    run(&mut simulation, 60.0, AWAY);
    assert!(simulation.has_snow());
    let piles = piles(&simulation);
    assert_eq!(piles.len(), 1);
    assert!(piles[0].1 > 1000.0, "ground area {}", piles[0].1);
    assert_finite(&simulation);
}

/// The mean number of sprites in the air over `seconds`.
fn airborne(simulation: &mut SnowSimulation, seconds: f32) -> f32 {
    let steps = (seconds / DT) as usize;
    (0..steps)
        .map(|_| {
            simulation.step(DT, AWAY);
            simulation.sprite_count()
        })
        .sum::<usize>() as f32
        / steps as f32
}

#[test]
fn heavy_snow_keeps_far_more_flakes_in_the_air_than_snow() {
    let mut snow = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    let mut heavy = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    run(&mut snow, 10.0, AWAY);
    run(&mut heavy, 10.0, AWAY);
    let (snow, heavy) = (airborne(&mut snow, 5.0), airborne(&mut heavy, 5.0));
    assert!(snow > 100.0, "snow: {snow}");
    assert!(heavy > 2.5 * snow, "snow {snow}, heavy {heavy}");
}

/// The sprites inside the screen, as the person sees them.
fn on_screen(simulation: &SnowSimulation) -> usize {
    let mut sprites = vec![Sprite::default(); simulation.sprite_count()];
    let count = simulation.write_sprites(&mut sprites);
    sprites[..count].iter().filter(|sprite| sprite.position.y - sprite.size <= simulation.height()).count()
}

#[test]
fn turning_off_lets_the_snow_in_the_air_finish_falling() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    run(&mut simulation, 10.0, AWAY);
    let falling = on_screen(&simulation);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 0.5, AWAY);
    assert!(on_screen(&simulation) as f32 > 0.8 * falling as f32, "the snow on screen vanished: {falling} → {}", on_screen(&simulation));
    // Nothing new comes in over the top edge.
    for _ in 0..(60.0 / DT) as usize {
        let before = simulation.sprite_count();
        simulation.step(DT, AWAY);
        assert!(simulation.sprite_count() <= before, "new snow fell while off");
    }
    assert_eq!(simulation.sprite_count(), 0, "the sky never emptied");
    assert!(simulation.has_snow(), "what fell settled");
}

#[test]
fn off_stops_the_snowfall() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Off);
    run(&mut simulation, 30.0, AWAY);
    assert_eq!(simulation.sprite_count(), 0);
    assert!(!simulation.has_snow());
}

#[test]
fn cursor_sweeps_the_drift_aside() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    simulation.set_plough_taskbar(false);
    // Exercise carving; snow conserved in Snowballs is covered separately.
    simulation.set_snowballs(false);
    simulation.thaws = false;
    run(&mut simulation, 120.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 15.0, AWAY);
    let before = piles(&simulation)[0].1;

    let mut sprayed = 0;
    for i in 0..120 {
        let x = 200.0 + i as f32 * 8.0;
        simulation.step(DT, V2::new(x, 10.0));
        sprayed = sprayed.max(simulation.sprite_count());
    }
    assert!(sprayed > 20, "no snow thrown up: {sprayed}");
    run(&mut simulation, 10.0, AWAY);
    let after = piles(&simulation)[0].1;
    // Swept snow lands again; it moves rather than vanishes.
    assert!((after - before).abs() / before < 0.1, "before {before}, after {after}");
    assert_finite(&simulation);
}

#[test]
fn a_gust_blows_all_the_snow_away() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    let none = HashSet::new();
    let window = [surface(7, 300.0, 600.0, 500.0)];
    simulation.update_windows(&window, 0.2, &none, &none, false);
    run(&mut simulation, 90.0, AWAY);
    assert_eq!(piles(&simulation).len(), 2);

    simulation.blow_away(1.0, false);
    run(&mut simulation, 1.5, AWAY);
    assert!(!simulation.has_snow(), "drifts left: {:?}", piles(&simulation));
    run(&mut simulation, 3.0, AWAY);
    // Fresh flakes can already reach the Ledge, only 200 points below the screen top, but not the Ground.
    assert!(piles(&simulation).iter().all(|(origin, _)| origin.y > 0.0), "snow returned to the Ground");
    let mut sprites = vec![Sprite::default(); 20_000];
    let count = simulation.write_sprites(&mut sprites);
    assert!(sprites[..count].iter().all(|sprite| sprite.position.y > 400.0), "snow left in the lower half");
    run(&mut simulation, 60.0, AWAY);
    assert!(simulation.has_snow(), "the snowfall didn't come back");
    assert_finite(&simulation);
}

#[test]
fn a_gust_clears_old_snow_without_any_settling_back() {
    let none = HashSet::new();
    let window = [surface(7, 300.0, 600.0, 500.0)];
    for direction in [-1.0, 1.0] {
        let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
        simulation.update_windows(&window, 0.2, &none, &none, false);
        run(&mut simulation, 90.0, AWAY);
        assert_eq!(piles(&simulation).len(), 2);

        // Disable new snowfall so neither Pile can be rebuilt by fresh flakes after the gust.
        simulation.set_snowfall(Snowfall::Off);
        simulation.blow_away(direction, false);
        run(&mut simulation, 1.5, AWAY);
        assert!(!simulation.has_snow(), "Piles left after gust {direction}: {:?}", piles(&simulation));
        run(&mut simulation, 3.0, AWAY);
        assert!(simulation.is_empty(), "old snow left after gust {direction}: {:?}", piles(&simulation));
        assert_finite(&simulation);
    }
}

#[test]
fn a_screen_rests_only_once_its_snow_is_gone_and_none_is_coming() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    simulation.thaws = true;
    let none = HashSet::new();
    simulation.update_windows(&[surface(7, 300.0, 600.0, 500.0)], 0.2, &none, &none, false);
    run(&mut simulation, 30.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 1.0, AWAY);
    assert!(!simulation.is_empty(), "snow still falls and lies");

    simulation.blow_away(1.0, false);
    run(&mut simulation, 30.0, AWAY);
    assert!(simulation.is_empty(), "sprites {}, snow {}", simulation.sprite_count(), simulation.has_snow());
    assert!(!simulation.step(DT, AWAY), "a screen at rest has nothing to redraw");

    simulation.set_snowfall(Snowfall::Snow);
    assert!(!simulation.is_empty(), "the snowfall is coming");
}

#[test]
fn a_calm_gust_melts_the_drifts_where_they_lie() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    run(&mut simulation, 90.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    let before = piles(&simulation)[0].1;
    let falling = simulation.sprite_count();
    simulation.blow_away(-1.0, true);
    simulation.step(DT, AWAY);
    assert!(simulation.sprite_count() <= falling, "a calm gust shouldn't throw anything up");
    run(&mut simulation, 2.0, AWAY);
    // Only the last of the snowfall, still landing, is left.
    let after = piles(&simulation).first().map_or(0.0, |p| p.1);
    assert!(after < before * 0.05, "before {before}, after {after}");
}

#[test]
fn a_sliding_desktop_skims_the_drifts_without_blowing_them_away() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    run(&mut simulation, 90.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 15.0, AWAY);
    let before = piles(&simulation)[0].1;
    let flying = simulation.sprite_count();

    simulation.space_slid(-9000.0);
    run(&mut simulation, 0.2, AWAY);
    assert!(simulation.sprite_count() > flying + 20, "no powder off the crests");
    assert_eq!(simulation.pace(), Pace::Lively);
    run(&mut simulation, 4.0, AWAY);
    let after = piles(&simulation)[0].1;
    // The last of the snowfall is still landing meanwhile.
    assert!((after - before).abs() < before * 0.05, "before {before}, after {after}");
    assert_eq!(simulation.pace(), Pace::Calm);
    assert_finite(&simulation);
}

#[test]
fn snow_settles_on_a_window_and_drops_when_it_closes() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    let none = HashSet::new();
    let window = [surface(7, 300.0, 600.0, 500.0)];
    simulation.update_windows(&window, 0.2, &none, &none, false);
    run(&mut simulation, 90.0, AWAY);
    let layers = piles(&simulation);
    assert_eq!(layers.len(), 2, "no snow on the window");
    let ledge = layers.iter().find(|(origin, _)| origin.y > 500.0).expect("ledge");
    assert!(ledge.1 > 200.0, "ledge area {}", ledge.1);

    simulation.set_snowfall(Snowfall::Off);
    simulation.update_windows(&[], 0.2, &none, &none, false);
    simulation.step(DT, AWAY);
    assert!(simulation.sprite_count() > 20, "the window's snow didn't fall");
    run(&mut simulation, 5.0, AWAY);
    assert_eq!(piles(&simulation).len(), 1);
    assert_finite(&simulation);
}

#[test]
fn snow_back_from_another_desktop_grows_into_shape() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    let none = HashSet::new();
    let window = [surface(7, 300.0, 600.0, 500.0)];
    simulation.update_windows(&window, 0.2, &none, &none, false);
    run(&mut simulation, 60.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 2.0, AWAY);
    let ledge = |simulation: &SnowSimulation| piles(simulation).iter().find(|(origin, _)| origin.y > 500.0).map(|ledge| ledge.1);
    let full = ledge(&simulation).expect("snow on the window");

    simulation.update_windows(&[], 0.2, &none, &HashSet::from([7]), false);
    run(&mut simulation, 1.0, AWAY);
    assert_eq!(ledge(&simulation), None, "the snow stayed on screen while its window was away");
    simulation.update_windows(&window, 0.2, &none, &none, false);
    simulation.step(DT, AWAY);
    let first = ledge(&simulation).expect("the snow came back");
    assert!(first < full * 0.1, "came back whole at once: {first} of {full}");
    run(&mut simulation, 0.5, AWAY);
    let grown = ledge(&simulation).expect("the snow stayed");
    assert!((grown - full).abs() < full * 0.05, "grew to {grown} of {full}");
}

#[test]
fn dragged_window_sheds_its_snow() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    simulation.thaws = false;
    let none = HashSet::new();
    simulation.update_windows(&[surface(3, 300.0, 600.0, 500.0)], 0.2, &none, &none, false);
    run(&mut simulation, 90.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 10.0, AWAY);
    let resting = piles(&simulation).iter().find(|(origin, _)| origin.y > 500.0).map(|p| p.1).unwrap();

    // Shaken hard from side to side, as when a window is dragged back and forth.
    shake(&mut simulation, 3);
    run(&mut simulation, 3.0, AWAY);
    let shaken = piles(&simulation).iter().find(|(origin, _)| origin.y > 500.0).map_or(0.0, |p| p.1);
    assert!(shaken < resting * 0.7, "resting {resting}, after shaking {shaken}");
    assert_finite(&simulation);
}

#[test]
fn a_window_slid_past_the_screen_edge_leaves_its_snow_heaped_against_it() {
    for (out, wall) in [(-300.0, 0.0), (1080.0, 1280.0)] {
        let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
        simulation.thaws = false;
        simulation.walls = (true, true);
        let none = HashSet::new();
        simulation.update_windows(&[surface(5, 400.0, 600.0, 500.0)], 0.2, &none, &none, false);
        run(&mut simulation, 90.0, AWAY);
        simulation.set_snowfall(Snowfall::Off);
        run(&mut simulation, 10.0, AWAY);
        let resting = ledge(&simulation);

        for step in 1..=180 {
            let x = 400.0 + (out - 400.0) * step as f32 / 180.0;
            simulation.update_windows(&[surface(5, x, 600.0, 500.0)], DT, &none, &none, false);
            simulation.step(DT, AWAY);
        }
        run(&mut simulation, 5.0, AWAY);
        let heaped = ledge(&simulation);
        // Three fifths of the window went past the wall, and their snow packs down once, to 0.6 of its bulk.
        assert!((heaped / resting - 0.76).abs() < 0.08, "resting {resting}, against the wall {heaped}");

        let mut buffer = vec![0.0; simulation.column_count()];
        let layers = simulation.write_layers(&mut buffer);
        let pile = layers.iter().find(|pile| pile.origin.y > 500.0).expect("ledge");
        let (surface, bed) = buffer[pile.offset..pile.offset + 2 * pile.count].split_at(pile.count);
        let depth = |i: usize| surface[i] - bed[i];
        let x = |i: usize| pile.origin.x + (i as f32 + 0.5) * 2.0;
        let past = (0..pile.count).filter(|&i| (x(i) - wall).abs() > 4.0 && (x(i) < 0.0 || x(i) > 1280.0)).map(depth).fold(0.0, f32::max);
        assert!(past < 0.01, "snow left past the edge at {wall}: {past}");
        let deepest = (0..pile.count).max_by(|&a, &b| depth(a).total_cmp(&depth(b))).unwrap();
        assert!((x(deepest) - wall).abs() < 30.0, "deepest snow at {}, the wall at {wall}", x(deepest));
        assert_finite(&simulation);
    }
}

fn shake(simulation: &mut SnowSimulation, id: isize) {
    let none = HashSet::new();
    let mut t = 0.0;
    for _ in 0..120 {
        t += DT;
        let x = 300.0 + 150.0 * (2.0 * std::f32::consts::PI * 4.0 * t).sin();
        simulation.update_windows(&[surface(id, x, 600.0, 500.0)], DT, &none, &none, false);
        simulation.step(DT, AWAY);
    }
}

fn ledge(simulation: &SnowSimulation) -> f32 {
    piles(simulation).iter().find(|(origin, _)| origin.y > 500.0).map_or(0.0, |p| p.1)
}

/// The same scenarios as the Swift harness, to compare the port against the original.
#[test]
#[ignore]
fn compare_with_swift() {
    let none = HashSet::new();
    let runs = 6;
    let (mut ground, mut ledge_area, mut dropped, mut shake_ratio, mut sweep_peak, mut sweep_ratio) = (0.0, 0.0, 0, 0.0, 0, 0.0);
    for _ in 0..runs {
        let mut a = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
        a.thaws = true;
        run(&mut a, 60.0, AWAY);
        ground += piles(&a)[0].1;

        let mut b = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
        b.update_windows(&[surface(3, 300.0, 600.0, 500.0)], 0.2, &none, &none, false);
        run(&mut b, 90.0, AWAY);
        b.set_snowfall(Snowfall::Off);
        run(&mut b, 10.0, AWAY);
        let resting = ledge(&b);
        ledge_area += resting;
        shake(&mut b, 3);
        run(&mut b, 3.0, AWAY);
        shake_ratio += ledge(&b) / resting;
        b.update_windows(&[surface(3, 300.0, 600.0, 500.0)], DT, &none, &none, false);
        run(&mut b, 3.0, AWAY);
        b.update_windows(&[], 0.2, &none, &none, false);
        b.step(DT, AWAY);
        dropped += b.sprite_count();

        let mut c = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
        run(&mut c, 120.0, AWAY);
        c.set_snowfall(Snowfall::Off);
        run(&mut c, 15.0, AWAY);
        let before = piles(&c)[0].1;
        let mut peak = 0;
        for i in 0..120 {
            c.step(DT, V2::new(200.0 + i as f32 * 8.0, 10.0));
            peak = peak.max(c.sprite_count());
        }
        run(&mut c, 10.0, AWAY);
        sweep_peak += peak;
        sweep_ratio += piles(&c)[0].1 / before;
    }
    let n = runs as f32;
    println!(
        "ground60 {:.0}  ledge90 {:.0}  shakeLeft {:.2}  dropped {}  sweepPeak {}  sweepKept {:.3}",
        ground / n,
        ledge_area / n,
        shake_ratio / n,
        dropped / runs,
        sweep_peak / runs,
        sweep_ratio / n
    );
}

#[test]
fn survives_a_long_chaotic_session() {
    let mut simulation = SnowSimulation::new(1920.0, 1080.0, Snowfall::Snow);
    let none = HashSet::new();
    let mut seed = 12345u64;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed >> 40) as f32 / (1u32 << 24) as f32
    };
    let mut windows: Vec<(isize, f32, f32, f32)> = Vec::new();
    let mut cursor = V2::new(500.0, 100.0);
    for frame in 0..40_000 {
        if frame % 30 == 0 {
            let roll = next();
            if roll < 0.2 && windows.len() < 6 {
                windows.push((frame as isize + 1, next() * 1600.0 - 100.0, 200.0 + next() * 850.0, 60.0 + next() * 900.0));
            } else if roll < 0.3 && !windows.is_empty() {
                windows.remove((next() * windows.len() as f32) as usize % windows.len());
            }
        }
        for window in &mut windows {
            if next() < 0.05 {
                window.1 += (next() - 0.5) * 400.0;
                window.2 += (next() - 0.5) * 200.0;
            }
            if next() < 0.01 {
                window.3 = 20.0 + next() * 1200.0;
            }
        }
        let surfaces: Vec<WindowSurface> = windows.iter().map(|&(id, x, top, width)| surface(id, x, top, width)).collect();
        simulation.update_windows(&surfaces, DT, &none, &none, false);
        if next() < 0.3 {
            cursor += V2::new((next() - 0.5) * 300.0, (next() - 0.5) * 200.0);
        }
        if frame % 5000 == 0 {
            simulation.thaws = !simulation.thaws;
            simulation.accumulates = frame % 15000 != 0;
        }
        // The level changes now and then, in the middle of everything else.
        if frame % 3000 == 1500 {
            simulation.set_snowfall(Snowfall::ALL[(next() * 3.0) as usize % 3]);
        }
        if frame == 20_000 {
            simulation.resize(1280.0, 720.0);
        }
        let dt = if next() < 0.01 { 0.05 } else { DT };
        simulation.step(dt, cursor);
    }
    assert_finite(&simulation);
}

#[test]
#[ignore]
fn step_cost() {
    let none = HashSet::new();
    let mut simulation = SnowSimulation::new(1920.0, 1080.0, Snowfall::Snow);
    simulation.thaws = true;
    let window = [WindowSurface {
        id: 1,
        frame: Rect { x: 200.0, y: 300.0, width: 900.0, height: 500.0 },
        blockers: Vec::new(),
        corner_radius: 8.0,
        corner_exponent: 2.5,
    }];
    simulation.update_windows(&window, 0.2, &none, &none, false);
    for _ in 0..60 * 30 {
        simulation.step(1.0 / 30.0, AWAY);
    }
    let steps = 30 * 120;
    let start = std::time::Instant::now();
    for i in 0..steps {
        simulation.step(1.0 / 30.0, V2::new((i % 1900) as f32, 20.0));
    }
    let us = start.elapsed().as_secs_f64() * 1e6 / steps as f64;
    println!("rust: {us:.1} µs per step, {} sprites", simulation.sprite_count());
}

#[test]
fn snow_pouring_off_a_window_heaps_up_no_higher_than_it_pours_from() {
    let mut simulation = SnowSimulation::new(800.0, 600.0, Snowfall::HeavySnow);
    simulation.thaws = false;
    let none = HashSet::new();
    // The upper window sits in front, over the top of the lower one; snow pours off its right end onto the lower one.
    let windows = || {
        let upper = surface(1, 100.0, 340.0, 300.0);
        let mut lower = surface(2, 200.0, 300.0, 500.0);
        lower.blockers = vec![upper.frame];
        [upper, lower]
    };
    for _ in 0..(900.0 / DT) as usize {
        simulation.update_windows(&windows(), DT, &none, &none, false);
        simulation.step(DT, AWAY);
    }
    let mut buffer = vec![0.0; simulation.column_count()];
    let piles = simulation.write_layers(&mut buffer);
    let lower = piles.iter().find(|pile| (pile.origin.y - 300.0).abs() < 5.0).unwrap();
    let upper = piles.iter().find(|pile| (pile.origin.y - 340.0).abs() < 5.0).unwrap();
    let pours = upper.origin.y + buffer[upper.offset + upper.count - 1];
    let width = 500.0 / lower.count as f32;
    let peak = (0..lower.count)
        .filter(|&i| (400.0..480.0).contains(&(lower.origin.x + (i as f32 + 0.5) * width)))
        .map(|i| lower.origin.y + buffer[lower.offset + i])
        .fold(f32::MIN, f32::max);
    assert!(peak <= pours + 3.0, "the heap under the window's end rose to {peak}, above where the snow pours from at {pours}");
}

/// The mean sideways speed of everything in the air over `seconds`: positive to the right.
fn drift(simulation: &mut SnowSimulation, seconds: f32) -> f32 {
    let (mut sum, mut count) = (0.0, 0usize);
    let mut sprites = Vec::new();
    for _ in 0..(seconds / DT) as usize {
        simulation.step(DT, AWAY);
        sprites.resize(simulation.sprite_count(), Sprite::default());
        let written = simulation.write_sprites(&mut sprites);
        sum += sprites[..written].iter().map(|sprite| sprite.velocity.x).sum::<f32>();
        count += written;
    }
    sum / count.max(1) as f32
}

#[test]
fn wind_carries_the_flakes_its_way_and_no_wind_leaves_them_be() {
    let mut drifts = Vec::new();
    for wind in [Wind::None, Wind::LightRight, Wind::StrongRight, Wind::StrongLeft] {
        let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
        simulation.set_wind(wind);
        run(&mut simulation, 5.0, AWAY);
        drifts.push(drift(&mut simulation, 90.0));
    }
    let [none, light, strong_right, strong_left] = drifts[..] else { unreachable!() };
    assert!(none.abs() < 10.0, "no wind still drifts {none}");
    assert!(light > 15.0, "light → drifts {light}");
    assert!(strong_right > 2.0 * light, "strong → drifts {strong_right}, light {light}");
    assert!(strong_left < -2.0 * light, "strong ← drifts {strong_left}");
}

/// Snow area on each half of a window's ledge: left, then right.
fn halves(simulation: &SnowSimulation, layer: usize) -> (f32, f32) {
    let mut buffer = vec![0.0; simulation.column_count()];
    let layers = simulation.write_layers(&mut buffer);
    let pile = &layers[layer];
    let (surface, bed) = buffer[pile.offset..pile.offset + 2 * pile.count].split_at(pile.count);
    let depths: Vec<f32> = surface.iter().zip(bed).map(|(s, b)| s - b).collect();
    let (left, right) = depths.split_at(depths.len() / 2);
    (left.iter().sum(), right.iter().sum())
}

#[test]
fn a_strong_wind_drifts_a_pile_toward_its_downwind_side() {
    let none = HashSet::new();
    let window = [surface(7, 300.0, 500.0, 600.0)];
    let mut balance = Vec::new();
    for wind in [Wind::None, Wind::StrongRight, Wind::StrongLeft] {
        let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
        simulation.thaws = false;
        simulation.update_windows(&window, 0.2, &none, &none, false);
        run(&mut simulation, 60.0, AWAY);
        simulation.set_snowfall(Snowfall::Off);
        simulation.set_wind(wind);
        run(&mut simulation, 120.0, AWAY);
        let (left, right) = halves(&simulation, 1);
        balance.push((right - left) / (right + left));
    }
    let [calm, right, left] = balance[..] else { unreachable!() };
    assert!(calm.abs() < 0.15, "without wind the pile leans {calm}");
    // A Pile never lies quite even, so the winds are judged against each other as well.
    assert!(right > 0.05 && left < -0.05, "the pile leans {right} in a wind to the right and {left} in one to the left");
    assert!(right - left > 0.15, "the winds leave the pile leaning only {right} and {left}");
}

#[test]
fn snow_still_settles_in_a_strong_wind() {
    let mut calm = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    let mut windy = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    windy.set_wind(Wind::StrongLeft);
    run(&mut calm, 90.0, AWAY);
    run(&mut windy, 90.0, AWAY);
    let (calm, windy_area) = (piles(&calm)[0].1, piles(&windy)[0].1);
    assert!(windy_area > 0.7 * calm, "the ground holds {windy_area} in the wind, {calm} without");

    // With nothing falling, the wind moves the snow about but doesn't blow it all away.
    windy.set_snowfall(Snowfall::Off);
    run(&mut windy, 40.0, AWAY);
    let before = piles(&windy)[0].1;
    run(&mut windy, 30.0, AWAY);
    let after = piles(&windy)[0].1;
    assert!(after > 0.8 * before, "the wind blew the ground away: {before} → {after}");
    assert_finite(&windy);
}

#[test]
fn piles_come_to_rest_in_a_steady_strong_wind() {
    let none = HashSet::new();
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    simulation.thaws = false;
    simulation.update_windows(&[surface(7, 300.0, 500.0, 300.0)], 0.2, &none, &none, false);
    run(&mut simulation, 30.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    simulation.set_wind(Wind::StrongRight);
    run(&mut simulation, 900.0, AWAY);
    let restless = (0..300).filter(|_| simulation.step(DT, AWAY)).count();
    assert_eq!(restless, 0, "the Piles still change in {restless} of 300 steps");
    assert!(simulation.has_snow(), "the wind blew everything away");
}

/// Snow area in the Pile on the window's Ledge, found by its height; none once it has melted away.
fn on_window(simulation: &SnowSimulation) -> f32 {
    piles(simulation).iter().find(|(origin, _)| origin.y > 100.0).map_or(0.0, |pile| pile.1)
}

#[test]
fn keeping_windows_clear_leaves_only_a_trace_on_a_window_in_heavy_snow() {
    let none = HashSet::new();
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    simulation.set_keeps_windows_clear(true);
    simulation.update_windows(&[surface(7, 300.0, 500.0, 600.0)], 0.2, &none, &none, false);
    let mut thickest: f32 = 0.0;
    for _ in 0..120 {
        run(&mut simulation, 1.0, AWAY);
        thickest = thickest.max(on_window(&simulation) / 600.0);
    }
    // Flakes still land, each melting within a second or so; a Pile here would reach 14 deep.
    assert!(thickest < 1.5, "snow on the window averaged up to {thickest} deep");
}

#[test]
fn keeping_windows_clear_keeps_a_thin_pile_on_the_ground_that_stops_growing() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    simulation.set_keeps_windows_clear(true);
    run(&mut simulation, 60.0, AWAY);
    let early = piles(&simulation)[0].1 / 1280.0;
    run(&mut simulation, 120.0, AWAY);
    let late = piles(&simulation)[0].1 / 1280.0;
    assert!(late > 1.0, "no Pile kept on the ground: {late} deep on average");
    assert!(late < 6.0, "the ground's Pile is {late} deep on average");
    assert!(late < early * 1.15, "the ground's Pile keeps growing: {early} → {late} deep");
}

#[test]
fn keeping_windows_clear_melts_snow_already_on_windows_within_seconds() {
    let none = HashSet::new();
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    simulation.update_windows(&[surface(7, 300.0, 500.0, 600.0)], 0.2, &none, &none, false);
    run(&mut simulation, 60.0, AWAY);
    let before = on_window(&simulation) / 600.0;
    assert!(before > 5.0, "the window gathered only {before} deep");
    simulation.set_keeps_windows_clear(true);
    run(&mut simulation, 5.0, AWAY);
    // Down to the trace the falling snow leaves.
    let after = on_window(&simulation) / 600.0;
    assert!(after < 1.5, "the window still holds {after} deep, from {before}");
    assert_finite(&simulation);
}

#[test]
fn keeping_windows_clear_also_melts_the_snow_of_windows_out_of_sight() {
    let none = HashSet::new();
    let window = [surface(7, 300.0, 600.0, 500.0)];
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    simulation.update_windows(&window, 0.2, &none, &none, false);
    run(&mut simulation, 60.0, AWAY);
    simulation.set_snowfall(Snowfall::Off);
    run(&mut simulation, 2.0, AWAY);
    let full = on_window(&simulation);
    assert!(full > 1000.0, "the window gathered only {full}");

    // The window goes to another desktop, taking its Pile out of sight; it comes back once windows are kept clear.
    simulation.update_windows(&[], 0.2, &none, &HashSet::from([7]), false);
    run(&mut simulation, 1.0, AWAY);
    simulation.set_keeps_windows_clear(true);
    simulation.update_windows(&window, 0.2, &none, &none, false);
    run(&mut simulation, 1.0, AWAY);
    // Only the trace of the last flakes still landing.
    assert!(on_window(&simulation) < full * 0.1, "the Pile came back with {} of {full}", on_window(&simulation));
}

#[test]
fn opacity_fades_the_falling_flakes_and_leaves_the_piles_be() {
    let mut simulation = SnowSimulation::new(1280.0, 800.0, Snowfall::Snow);
    run(&mut simulation, 20.0, AWAY);
    let snapshot = |simulation: &SnowSimulation| {
        let mut sprites = vec![Sprite::default(); simulation.sprite_count()];
        let count = simulation.write_sprites(&mut sprites);
        sprites.truncate(count);
        let mut buffer = vec![0.0; simulation.column_count()];
        simulation.write_layers(&mut buffer);
        (sprites, buffer)
    };
    let (full, piles) = snapshot(&simulation);
    simulation.set_opacity(0.4);
    let (faded, faded_piles) = snapshot(&simulation);
    assert_eq!(full.len(), faded.len());
    // Flakes fade; the odd grain thrown up from a Pile keeps its look.
    let mut fading = 0;
    for (full, faded) in full.iter().zip(&faded) {
        let scaled = (faded.alpha - full.alpha * 0.4).abs() < 1e-6;
        assert!(scaled || faded.alpha == full.alpha, "alpha {} at 40% of {}", faded.alpha, full.alpha);
        fading += scaled as usize;
    }
    assert!(fading * 10 >= full.len() * 9, "only {fading} of {} sprites faded", full.len());
    assert_eq!(piles, faded_piles, "the Piles changed");
}

#[test]
fn a_pile_carried_to_another_screen_keeps_its_snow() {
    let none = HashSet::new();
    let mut from = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    from.update_windows(&[surface(7, 300.0, 500.0, 600.0)], 0.2, &none, &none, false);
    run(&mut from, 60.0, AWAY);
    from.set_snowfall(Snowfall::Off);
    run(&mut from, 40.0, AWAY);
    let carried = on_window(&from);
    assert!(carried > 1000.0, "the window gathered only {carried}");

    let falling = from.sprite_count();
    let pile = from.take_pile(7).expect("a Pile to carry");
    // The window has left this screen; with its Pile gone, nothing is left to fall.
    from.update_windows(&[], 0.2, &none, &none, false);
    run(&mut from, 1.0, AWAY);
    assert!(from.sprite_count() <= falling, "snow fell where the window left: {falling} → {}", from.sprite_count());
    assert_eq!(on_window(&from), 0.0);

    // On the other screen the window is a little narrower, as at another scale.
    let mut to = SnowSimulation::new(1920.0, 1080.0, Snowfall::Off);
    to.bring_pile(7, pile);
    to.update_windows(&[surface(7, 100.0, 700.0, 500.0)], 0.2, &none, &none, false);
    to.step(DT, AWAY);
    let arrived = on_window(&to);
    assert!((arrived - carried).abs() < carried * 0.03, "carried {carried}, arrived {arrived}");
    run(&mut to, 2.0, AWAY);
    assert_finite(&to);
}

#[test]
fn a_lent_pile_is_drawn_but_not_simulated() {
    let none = HashSet::new();
    let mut owner = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    owner.update_windows(&[surface(7, 1000.0, 500.0, 600.0)], 0.2, &none, &none, false);
    run(&mut owner, 60.0, AWAY);
    let view = owner.view_pile(7).expect("a Pile to lend");

    let mut neighbour = SnowSimulation::new(1280.0, 800.0, Snowfall::Off);
    let columns = neighbour.column_count();
    // The neighbouring screen starts where the owner's ends.
    neighbour.set_borrowed(vec![view.mapped((1.0, 1.0), (-1280.0, 0.0))]);
    assert!(neighbour.step(DT, AWAY), "the lent Pile isn't drawn");
    assert_eq!(neighbour.column_count(), columns + 2 * view.levels.len());
    assert!(!neighbour.is_empty());
    let lent = piles(&neighbour).iter().find(|(origin, _)| origin.y > 100.0).map(|pile| pile.1).expect("the lent Pile drawn");
    let kept = piles(&owner).iter().find(|(origin, _)| origin.y > 100.0).map(|pile| pile.1).unwrap();
    assert!((lent - kept).abs() < kept * 0.01, "lent {lent}, kept {kept}");
    neighbour.set_borrowed(Vec::new());
    assert!(neighbour.step(DT, AWAY), "the Pile taken back isn't redrawn");
    assert!(neighbour.is_empty());
}

#[test]
fn a_window_across_two_screens_gathers_one_pile_along_its_whole_length() {
    let none = HashSet::new();
    // Two screens side by side; the window runs from 1000 on the left one to 320 on the right one.
    let mut keeper = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    let mut other = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    keeper.thaws = false;
    other.thaws = false;
    keeper.update_windows(&[surface(7, 1000.0, 500.0, 600.0)], 0.2, &none, &none, false);
    other.update_windows(&[surface(7, -280.0, 500.0, 600.0)], 0.2, &none, &none, false);
    for _ in 0..(60.0 / DT) as usize {
        keeper.step(DT, AWAY);
        other.step(DT, AWAY);
        if let Some(snow) = other.drain_pile(7) {
            let snow: Vec<_> = snow.into_iter().map(|(x, area)| (x + 1280.0, area)).collect();
            keeper.add_to_pile(7, &snow);
        }
    }
    let view = keeper.view_pile(7).expect("the one Pile");
    let depth = |range: std::ops::Range<f32>| -> f32 {
        let columns: Vec<f32> = (0..view.levels.len())
            .filter(|&i| range.contains(&(view.origin.x + (i as f32 + 0.5) * 2.0)))
            .map(|i| view.levels[i] - view.beds[i])
            .collect();
        columns.iter().sum::<f32>() / columns.len() as f32
    };
    let (left, right) = (depth(1050.0..1250.0), depth(1310.0..1550.0));
    assert!(left > 3.0 && right > 3.0, "the Pile is {left} deep on the left screen and {right} on the right");
    assert!((left - right).abs() < 0.5 * left.max(right), "the Pile is {left} deep on the left screen and {right} on the right");
    assert!(on_window(&other) < 50.0, "the other screen kept {} of its own", on_window(&other));
}

#[test]
fn a_pile_brought_to_a_window_with_snow_of_its_own_joins_it() {
    let none = HashSet::new();
    let mut from = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    let mut to = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    from.thaws = false;
    to.thaws = false;
    from.update_windows(&[surface(7, 300.0, 500.0, 600.0)], 0.2, &none, &none, false);
    to.update_windows(&[surface(7, 300.0, 500.0, 600.0)], 0.2, &none, &none, false);
    run(&mut from, 30.0, AWAY);
    run(&mut to, 30.0, AWAY);
    from.set_snowfall(Snowfall::Off);
    to.set_snowfall(Snowfall::Off);
    run(&mut from, 45.0, AWAY);
    run(&mut to, 45.0, AWAY);
    let (a, b) = (on_window(&from), on_window(&to));
    to.bring_pile(7, from.take_pile(7).unwrap());
    to.step(DT, AWAY);
    let joined = on_window(&to);
    assert!((joined - (a + b)).abs() < (a + b) * 0.03, "{a} and {b} joined into {joined}");
}

#[test]
fn a_pile_carried_for_a_window_that_never_arrives_is_let_go() {
    let none = HashSet::new();
    let mut from = SnowSimulation::new(1280.0, 800.0, Snowfall::HeavySnow);
    from.update_windows(&[surface(7, 300.0, 500.0, 600.0)], 0.2, &none, &none, false);
    run(&mut from, 20.0, AWAY);
    let mut to = SnowSimulation::new(1280.0, 800.0, Snowfall::Off);
    to.bring_pile(7, from.take_pile(7).unwrap());
    assert!(!to.is_empty(), "the carried Pile waits for its window");
    run(&mut to, 3.0, AWAY);
    assert!(to.is_empty(), "the carried Pile is still kept with no window for it");
}

/// A known Ground, with no new snowfall or melting during a pass.
fn plough_scene(seed: u64, depth: f32) -> SnowSimulation {
    let mut simulation = bare_play();
    simulation.fill_ground(depth);
    simulation.seed_random(seed);
    simulation
}

#[test]
fn snowplough_kinds_and_directions_are_chosen_separately() {
    for (seed, direction, kind) in
        [(1, 1.0, PloughKind::Blade), (1 << 33, -1.0, PloughKind::Blade), (2 << 33, 1.0, PloughKind::Blower), (3 << 33, -1.0, PloughKind::Blower)]
    {
        let mut simulation = plough_scene(seed, 26.0);
        simulation.send_snowplough();
        let plough = simulation.snowplough().unwrap();
        assert_eq!(plough.direction, direction);
        assert_eq!(plough.kind, kind);
    }
}

#[test]
fn snowplough_spills_excess_snow_and_finishes_its_particles_after_the_pass() {
    for seed in [1, 1 << 33, 2 << 33, 3 << 33] {
        let mut simulation = plough_scene(seed, 60.0);
        let before = simulation.snow_area() as f64;
        simulation.send_snowplough();
        assert!(!simulation.is_empty());
        assert_eq!(simulation.pace(), Pace::Lively);
        let start = simulation.snowplough().unwrap().x;
        simulation.send_snowplough();
        assert_eq!(simulation.snowplough().unwrap().x, start);
        let mut saw_mound = false;
        let mut saw_spill = false;
        let mut sprites = [Sprite::default(); 160];
        for frame in 0..1000 {
            assert!(simulation.step(DT, AWAY));
            let area = simulation.snow_area() as f64;
            assert!((before - area - simulation.ploughed_area()).abs() < 1.0, "unaccounted snow: {before} - {area} - {}", simulation.ploughed_area());
            let Some(plough) = simulation.snowplough() else { break };
            assert_eq!(simulation.pace(), Pace::Lively);
            let cap = plough.kind.mound_cap();
            saw_mound |= plough.snow_height > cap - 0.1;
            assert!(plough.snow_height <= cap);
            assert!(simulation.plough_spill_count() <= 160);
            if simulation.plough_spill_count() > 0 {
                assert!(simulation.ploughed_area() > 0.0, "spill must count as removed snow immediately");
                let count = simulation.write_sprites(&mut sprites);
                assert_eq!(count, simulation.plough_spill_count());
                // Blade lumps are packed balls; blower snow is soft powder.
                assert!(sprites[..count].iter().all(|s| (s.ball == 1.0 || (s.ball == 0.0 && s.soft > 0.0)) && s.alpha >= 0.0 && s.alpha <= 1.0));
                assert!(sprites[..count].iter().all(|s| simulation.snowball_at(s.position).is_none()));
                saw_spill = true;
            }
            assert!(frame < 999, "the Snowplough never left");
        }
        assert!(saw_mound && saw_spill, "snow must build up and visibly leave the Ground");
        assert!(simulation.snowplough().is_none());
        assert!(simulation.snow_area() / simulation.width() < 2.0);
        if seed == 2 << 33 || seed == 3 << 33 {
            assert!(simulation.plough_spill_count() > 0, "the final stream should outlive the blower");
        }
        for _ in 0..180 {
            if simulation.plough_spill_count() == 0 {
                break;
            }
            assert_eq!(simulation.pace(), Pace::Lively);
            simulation.step(DT, AWAY);
        }
        assert_eq!(simulation.plough_spill_count(), 0);
        assert_eq!(simulation.pace(), Pace::Calm);
        assert!(!simulation.step(DT, AWAY));
        // Sending again bypasses the cooldown, even with automatic visits off.
        simulation.set_plough_taskbar(false);
        simulation.send_snowplough();
        assert!(simulation.snowplough().is_some());
    }
}

#[test]
fn snowplough_accounts_for_ground_and_whole_snow_play_exactly() {
    let man = build_snowman(&mut bare_play(), 0.0);
    for depth in [26.0, 60.0] {
        for kind in [PloughKind::Blade, PloughKind::Blower] {
            for direction in [1.0, -1.0] {
                let mut ground = SnowLayer::new(V2::ZERO, 641, 2.0, 40.0, false, (0.0, 0.0));
                ground.right = 1280.0;
                for i in 0..ground.count() {
                    ground.set(i, depth);
                }
                let mut play = SnowPlay::new();
                let mut ball = flight_ball(V2::new(300.0, 15.0), 15.0);
                ball.state = SnowballState::Resting { on: Surface::Ground };
                play.balls.push(ball);
                let mut man = man;
                man.center.x = 650.0;
                play.snowmen.push(man);
                let area = |ground: &SnowLayer, play: &SnowPlay| {
                    ground.heights.iter().map(|&h| h as f64 * ground.column_width as f64).sum::<f64>()
                        + play.balls.iter().map(|b| b.area() as f64).sum::<f64>()
                        + play.snowmen.iter().map(|m| m.area() as f64).sum::<f64>()
                };
                let before = area(&ground, &play);
                let mut plough = crate::plough::Snowplough {
                    kind,
                    x: if direction > 0.0 { -144.0 } else { 1424.0 },
                    direction,
                    distance: 0.0,
                    elapsed: 0.0,
                    snow_height: 0.0,
                    mound: Vec::new(),
                };
                let mut removed = 0.0;
                for _ in 0..800 {
                    removed += plough.advance(&mut ground, DT);
                    removed += play.plough(&plough, &ground, 1280.0);
                    assert_eq!(before, area(&ground, &play) + removed, "snow changed: {depth}, {kind:?}, {direction}");
                    assert!(play.snowmen.iter().all(|m| m.radii == man.radii));
                }
                assert!(play.balls.is_empty() && play.snowmen.is_empty());
                assert!(area(&ground, &play) / 1280.0 < 2.0);
            }
        }
    }
}

#[test]
fn clear_snow_removes_the_snowplough_spill_too() {
    for seed in [1, 2 << 33] {
        let mut simulation = plough_scene(seed, 60.0);
        simulation.send_snowplough();
        run(&mut simulation, 1.0, AWAY);
        assert!(simulation.plough_spill_count() > 0);
        simulation.blow_away(1.0, true);
        assert!(simulation.snowplough().is_none());
        assert_eq!(simulation.plough_spill_count(), 0);
        run(&mut simulation, 4.0, AWAY);
        assert!(simulation.is_empty());
    }
}

#[test]
fn snowplough_pushes_ground_snow_play_whole_and_leaves_ledges_alone() {
    for seed in [1, 1 << 33, 2 << 33, 3 << 33] {
        let mut simulation = snowball_pile(20.0);
        let ledge_man = build_snowman(&mut simulation, 300.0);
        let mut source = bare_play();
        let mut man = build_snowman(&mut source, 0.0);
        man.center.x = 650.0;
        simulation.bring_snowman(man);
        let ball = ground_ball(&mut simulation, 300.0, 15.0);
        simulation.fill_ground(26.0);
        simulation.seed_random(seed);
        let before = simulation.snow_area() as f64;
        simulation.send_snowplough();
        let mut rolled = false;
        let mut wobbled = false;
        for _ in 0..1000 {
            simulation.step(DT, AWAY);
            assert!((before - simulation.snow_area() as f64 - simulation.ploughed_area()).abs() < 1.0);
            if let Some(pushed) = simulation.snowballs().iter().find(|b| b.id == ball.id) {
                rolled |= (pushed.angle - ball.angle).abs() > 0.1;
                if (pushed.angle - ball.angle).abs() > 0.1 {
                    assert_pushed_on_mound(&simulation, pushed.center, pushed.radius);
                }
            }
            if let Some(pushed) = simulation.snowmen().iter().find(|m| m.id == man.id) {
                assert_eq!(pushed.radii, man.radii);
                wobbled |= pushed.tilt.abs() > 0.01;
                if pushed.tilt != 0.0 {
                    assert_pushed_on_mound(&simulation, pushed.center, pushed.radii[0]);
                }
            }
            if simulation.snowplough().is_none() {
                break;
            }
        }
        assert!(rolled && wobbled);
        assert!(simulation.snowballs().is_empty());
        assert_eq!(simulation.snowmen().len(), 1);
        assert_eq!(simulation.snowmen()[0].id, ledge_man.id);
        assert_eq!(simulation.snowmen()[0].center, ledge_man.center);
    }
}

#[test]
fn snow_play_pushed_by_the_snowplough_queues_up_instead_of_overlapping() {
    for seed in [1, 1 << 33, 2 << 33, 3 << 33] {
        let mut simulation = plough_scene(seed, 26.0);
        for x in [600.0, 700.0] {
            let mut man = build_snowman(&mut bare_play(), 0.0);
            man.center.x = x;
            simulation.bring_snowman(man);
        }
        ground_ball(&mut simulation, 650.0, 12.0);
        simulation.send_snowplough();
        let mut queued = false;
        for _ in 0..1000 {
            simulation.step(DT, AWAY);
            let mut bottoms: Vec<(f32, f32)> = simulation.snowmen().iter().map(|m| (m.center.x, m.radii[0])).collect();
            bottoms.extend(simulation.snowballs().iter().map(|b| (b.center.x, b.radius)));
            bottoms.sort_by(|a, b| a.0.total_cmp(&b.0));
            for pair in bottoms.windows(2) {
                assert!(pair[1].0 - pair[0].0 >= pair[0].1 + pair[1].1 - 0.01, "pushed snow play overlaps: {pair:?}");
            }
            queued |= simulation.snowmen().iter().filter(|m| m.tilt != 0.0).count() == 2;
            if simulation.snowplough().is_none() {
                break;
            }
        }
        assert!(queued, "both Snowmen should end up pushed together");
        assert!(simulation.snowmen().is_empty() && simulation.snowballs().is_empty());
    }
}

fn assert_pushed_on_mound(simulation: &SnowSimulation, center: V2, radius: f32) {
    let plough = simulation.snowplough().unwrap();
    // Against the blade, or queued ahead of whatever the blade already pushes.
    assert!(
        (center.x - plough.blade()) * plough.direction - radius - 4.0 > -0.001,
        "snow play at {center:?}, radius {radius}, blade {}, direction {}",
        plough.blade(),
        plough.direction
    );
    let mut buffer = vec![0.0; simulation.column_count()];
    let layers = simulation.write_layers(&mut buffer);
    let ground = &layers[0];
    let levels = &buffer[ground.offset..ground.offset + ground.count];
    let at = (center.x - ground.origin.x) / 2.0 - 0.5;
    let surface = ground.origin.y + crate::layer::sample(levels, at);
    assert!((center.y - radius - surface).abs() < 0.001, "pushed snow play must sit on the local mound surface");
    assert!(surface - ground.origin.y <= 26.0_f32.max(plough.kind.mound_cap()) + 0.001);
}

#[test]
fn grabbing_snow_play_in_front_of_the_snowplough_frees_it() {
    for snowman in [false, true] {
        let mut simulation = bare_play();
        let (id, x) = if snowman {
            let man = build_snowman(&mut simulation, 0.0);
            (man.id, man.center.x)
        } else {
            let ball = ground_ball(&mut simulation, 500.0, 18.0);
            (ball.id, ball.center.x)
        };
        simulation.seed_random(1);
        simulation.send_snowplough();
        let mut pushed = false;
        for _ in 0..500 {
            simulation.step(DT, AWAY);
            let center = if snowman { simulation.snowmen()[0].circles()[1].0 } else { simulation.snowballs()[0].center };
            if (center.x - x).abs() > 20.0 {
                assert!(simulation.grab(id, center));
                pushed = true;
                break;
            }
        }
        assert!(pushed);
        let center = if snowman { simulation.snowmen()[0].center } else { simulation.snowballs()[0].center };
        run(&mut simulation, 15.0, AWAY);
        let (after, state) = if snowman {
            (simulation.snowmen()[0].center, simulation.snowmen()[0].state)
        } else {
            (simulation.snowballs()[0].center, simulation.snowballs()[0].state)
        };
        assert_eq!(after, center);
        assert_eq!(state, SnowballState::Held);
        assert!(simulation.snowplough().is_none());
    }
}

#[test]
fn snowplough_disturbs_fighters_and_clear_snow_ends_its_visit() {
    let mut simulation = fight_scene(&[300.0, 900.0]);
    wait_for_fight(&mut simulation, 30);
    simulation.send_snowplough();
    for _ in 0..500 {
        simulation.step(DT, AWAY);
        if simulation.snowmen().iter().any(|m| m.tilt != 0.0) {
            break;
        }
    }
    assert!(simulation.fight_turn().is_none());
    assert!(simulation.snowplough().is_some());
    simulation.set_snowballs(false);
    assert!(simulation.snowplough().is_some());
    simulation.blow_away(1.0, true);
    assert!(simulation.snowplough().is_none());
    run(&mut simulation, 4.0, AWAY);
    assert!(simulation.is_empty());
}

#[test]
fn automatic_snowplough_needs_sustained_depth_and_obeys_its_setting_and_cooldown() {
    let mut simulation = plough_scene(1, 24.0);
    run(&mut simulation, 70.0, AWAY);
    assert!(simulation.snowplough().is_none(), "the threshold is strictly above 24 pt");
    simulation.fill_ground(30.0);
    run(&mut simulation, 59.0, AWAY);
    assert!(simulation.snowplough().is_none());
    simulation.fill_ground(1.0);
    simulation.step(DT, AWAY);
    simulation.fill_ground(30.0);
    run(&mut simulation, 59.0, AWAY);
    assert!(simulation.snowplough().is_none(), "a shallow interval must restart the wait");
    run(&mut simulation, 1.1, AWAY);
    assert!(simulation.snowplough().is_some());
    run(&mut simulation, 15.0, AWAY);
    assert!(simulation.snowplough().is_none());
    simulation.fill_ground(30.0);
    run(&mut simulation, 1700.0, AWAY);
    assert!(simulation.snowplough().is_none(), "thirty minutes have not passed");
    run(&mut simulation, 86.0, AWAY);
    assert!(simulation.snowplough().is_some(), "sustained deep snow can call the next visit after thirty minutes");
    for main_screen in [false, true] {
        let mut disabled = plough_scene(1, 36.0);
        disabled.set_main_screen(main_screen);
        disabled.set_plough_taskbar(false);
        run(&mut disabled, 1900.0, AWAY);
        assert!(disabled.snowplough().is_none());
        disabled.send_snowplough();
        assert_eq!(disabled.snowplough().is_some(), main_screen);
        if !main_screen {
            disabled.set_plough_taskbar(true);
            run(&mut disabled, 70.0, AWAY);
            assert!(disabled.snowplough().is_none(), "secondary screens never get automatic visits");
        }
    }
}

#[test]
fn snowplough_leaves_held_falling_and_ledge_snow_play_untouched() {
    let mut source = bare_play();
    let man = build_snowman(&mut source, 0.0);
    for state in [SnowballState::Held, SnowballState::Falling, SnowballState::Resting { on: Surface::Ledge(7) }] {
        let mut play = SnowPlay::new();
        let mut ball = flight_ball(V2::new(520.0, 20.0), 20.0);
        ball.state = state;
        play.balls.push(ball);
        let mut man = man;
        man.state = state;
        man.center.x = 520.0;
        play.snowmen.push(man);
        let ground = SnowLayer::new(V2::ZERO, 641, 2.0, 40.0, false, (0.0, 0.0));
        let plough = crate::plough::Snowplough {
            kind: PloughKind::Blade,
            x: 400.0,
            direction: 1.0,
            distance: 544.0,
            elapsed: 4.0,
            snow_height: 0.0,
            mound: Vec::new(),
        };
        assert_eq!(play.plough(&plough, &ground, 1280.0), 0.0);
        assert_eq!(play.balls[0].state, state);
        assert_eq!(play.balls[0].center, ball.center);
        assert_eq!(play.snowmen[0].state, state);
        assert_eq!(play.snowmen[0].center, man.center);
        assert_eq!(play.snowmen[0].circles(), man.circles());
    }
}
