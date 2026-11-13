//! What the power source asks of the snow, free of Windows so it can be tested.

/// Where the power comes from, and whether the system is saving it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Power {
    /// Unplugged.
    pub battery: bool,
    /// Battery saver, or energy saver, which can be on while plugged in too.
    pub saver: bool,
}

/// How hard the snow works.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strength {
    Full,
    /// Half the snowfall, and frames within a budget: see [`frame_rates`].
    Half,
    /// No snow and no frames: Pause on battery, unplugged.
    Paused,
}

impl Strength {
    /// The share of the snowfall that falls.
    pub fn density(self) -> f32 {
        match self {
            Strength::Full => 1.0,
            Strength::Half => 0.5,
            Strength::Paused => 0.0,
        }
    }
}

/// On battery or under saver the snow runs at half strength; unplugged with Pause on battery, not at all.
pub fn strength(power: Power, pause_on_battery: bool) -> Strength {
    if power.battery && pause_on_battery {
        Strength::Paused
    } else if power.battery || power.saver {
        Strength::Half
    } else {
        Strength::Full
    }
}

/// The frame rates the snow may draw at on a display drawing `display` frames a second: for calm snow, and the fastest,
/// for snow being pushed around. At half strength nothing above 60, which is hard to tell apart in snow anyway; under
/// saver, calm snow at 24 where that divides the display's rate evenly, as elsewhere it would judder.
pub fn frame_rates(power: Power, display: u32) -> (u32, u32) {
    let fastest = if strength(power, false) == Strength::Half { display.min(60) } else { display };
    let calm = if power.saver && display.is_multiple_of(24) { 24 } else { 30 };
    (calm, fastest)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAINS: Power = Power { battery: false, saver: false };
    const MAINS_SAVER: Power = Power { battery: false, saver: true };
    const BATTERY: Power = Power { battery: true, saver: false };
    const BATTERY_SAVER: Power = Power { battery: true, saver: true };

    #[test]
    fn plugged_in_the_snow_runs_at_full_strength_whatever_the_setting() {
        assert_eq!(strength(MAINS, false), Strength::Full);
        assert_eq!(strength(MAINS, true), Strength::Full);
        assert_eq!(Strength::Full.density(), 1.0);
    }

    #[test]
    fn on_battery_or_under_saver_the_snow_runs_at_half_strength() {
        assert_eq!(strength(BATTERY, false), Strength::Half);
        assert_eq!(strength(BATTERY_SAVER, false), Strength::Half);
        assert_eq!(strength(MAINS_SAVER, false), Strength::Half);
        assert_eq!(Strength::Half.density(), 0.5);
    }

    #[test]
    fn pause_on_battery_stops_the_snow_only_when_unplugged() {
        assert_eq!(strength(BATTERY, true), Strength::Paused);
        assert_eq!(strength(BATTERY_SAVER, true), Strength::Paused);
        assert_eq!(strength(MAINS_SAVER, true), Strength::Half, "saver while plugged in is no reason to pause");
        assert_eq!(Strength::Paused.density(), 0.0);
    }

    #[test]
    fn frames_are_capped_at_half_strength_and_calm_snow_slows_under_saver() {
        assert_eq!(frame_rates(MAINS, 144), (30, 144));
        assert_eq!(frame_rates(BATTERY, 144), (30, 60));
        assert_eq!(frame_rates(BATTERY_SAVER, 144), (24, 60));
        assert_eq!(frame_rates(MAINS_SAVER, 120), (24, 60));
        assert_eq!(frame_rates(BATTERY_SAVER, 60), (30, 60), "24 doesn't divide 60");
    }
}
