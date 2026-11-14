//! What kind of person a townsperson is, and how the time of day changes what they do.
//! Pure, no ECS.
//!
//! - **Plain**: does what everyone does.
//! - **Brave**: frightens late, runs to the players rather than hide while one is near.
//! - **Timid**: frightens early, runs fast to the nearest door, goes out less and stays
//!   in longer.
//! - **Curious**: frightens late, wanders further and lingers longer.
//!
//! The schedule follows the level's [`LightingPreset`]: by day most people are out and
//! about, at night fewer, in the dark hardly anyone, and those at home stay in longer.

use crate::general::systems::lights_systems::LightingPreset;

/// A townsperson's temper.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Temper {
    #[default]
    Plain,
    Brave,
    Timid,
    Curious,
}

impl Temper {
    /// A temper picked by `roll`: plain 40%, timid 25%, brave 20%, curious 15%.
    pub const fn roll(roll: u64) -> Self {
        match roll % 100 {
            0..40 => Self::Plain,
            40..65 => Self::Timid,
            65..85 => Self::Brave,
            _ => Self::Curious,
        }
    }

    /// A monster this close (metres) sends them running.
    pub const fn scare_radius(self) -> f32 {
        match self {
            Self::Plain => 5.0,
            Self::Brave => 3.0,
            Self::Timid => 8.0,
            Self::Curious => 4.0,
        }
    }

    /// Running speed, metres per second.
    pub const fn run_speed(self) -> f32 {
        match self {
            Self::Plain | Self::Curious => 2.2,
            Self::Brave => 2.4,
            Self::Timid => 2.6,
        }
    }

    /// Walking speed, metres per second.
    pub const fn walk_speed(self) -> f32 {
        match self {
            Self::Plain | Self::Curious => 1.1,
            Self::Brave => 1.2,
            Self::Timid => 0.9,
        }
    }

    /// How far (tiles) an errand takes them.
    pub const fn errand_reach(self) -> usize {
        match self {
            Self::Plain | Self::Brave => 30,
            Self::Timid => 18,
            Self::Curious => 45,
        }
    }

    /// How far (metres) they will run for a door.
    pub const fn shelter_reach(self) -> f32 {
        match self {
            Self::Plain | Self::Curious => 14.0,
            Self::Brave => 10.0,
            Self::Timid => 20.0,
        }
    }

    /// Whether they make for a door at all while a player is at hand (`player_near`);
    /// the brave run to the player instead.
    pub const fn seeks_shelter(self, player_near: bool) -> bool {
        !matches!(self, Self::Brave) || !player_near
    }

    /// Lingering on an errand, as a factor on the base time.
    pub const fn linger_factor(self) -> f32 {
        match self {
            Self::Plain | Self::Brave => 1.0,
            Self::Timid => 0.6,
            Self::Curious => 1.6,
        }
    }

    /// Staying in (at home, or hiding), as a factor on the base time.
    pub const fn stay_in_factor(self) -> f32 {
        match self {
            Self::Plain | Self::Curious => 1.0,
            Self::Brave => 0.7,
            Self::Timid => 1.8,
        }
    }

    /// Short, for logs and the inspector.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Plain => "plain",
            Self::Brave => "brave",
            Self::Timid => "timid",
            Self::Curious => "curious",
        }
    }
}

/// Out of 100: how likely someone with `temper` goes out on an errand under `light`
/// (the rest stay home).
pub const fn errand_chance(light: LightingPreset, temper: Temper) -> u64 {
    let base: i64 = match light {
        LightingPreset::Day => 85,
        LightingPreset::Night => 45,
        LightingPreset::Dark => 25,
    };
    let nudge: i64 = match temper {
        Temper::Plain => 0,
        Temper::Brave => 5,
        Temper::Timid => -20,
        Temper::Curious => 10,
    };
    let chance = base + nudge;
    if chance < 5 {
        5
    } else if chance > 95 {
        95
    } else {
        chance as u64
    }
}

/// Staying in under `light`, as a factor on the base time.
pub const fn stay_in_by_light(light: LightingPreset) -> f32 {
    match light {
        LightingPreset::Day => 1.0,
        LightingPreset::Night => 1.6,
        LightingPreset::Dark => 2.2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_temper_comes_up() {
        let tempers: Vec<Temper> = (0..100).map(Temper::roll).collect();
        for t in [Temper::Plain, Temper::Brave, Temper::Timid, Temper::Curious] {
            assert!(tempers.contains(&t), "{t:?}");
        }
        assert_eq!(tempers.iter().filter(|t| **t == Temper::Plain).count(), 40);
    }

    #[test]
    fn the_timid_frighten_first_and_the_brave_last() {
        assert!(Temper::Timid.scare_radius() > Temper::Plain.scare_radius());
        assert!(Temper::Brave.scare_radius() < Temper::Plain.scare_radius());
        assert!(!Temper::Brave.seeks_shelter(true));
        assert!(Temper::Brave.seeks_shelter(false));
        assert!(Temper::Timid.seeks_shelter(true));
    }

    #[test]
    fn fewer_go_out_after_dark() {
        for t in [Temper::Plain, Temper::Brave, Temper::Timid, Temper::Curious] {
            let day = errand_chance(LightingPreset::Day, t);
            let night = errand_chance(LightingPreset::Night, t);
            let dark = errand_chance(LightingPreset::Dark, t);
            assert!(day > night && night > dark, "{t:?}: {day} {night} {dark}");
            assert!((5..=95).contains(&dark) && (5..=95).contains(&day));
        }
        assert!(stay_in_by_light(LightingPreset::Dark) > stay_in_by_light(LightingPreset::Day));
    }
}
