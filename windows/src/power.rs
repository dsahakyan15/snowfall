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
    /// Half the snowfall, and frames within a budget: none above 60 a second, which are hard to tell apart in snow
    /// anyway, and under saver calm snow drawn less often still.
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
}
