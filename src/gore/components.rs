//! Shared types for the gore/FX layer.
//!
//! The whole layer hangs off two messages — [`DamageDealt`] and [`EntityDied`] —
//! emitted wherever combat mutates `Health`. Blood, gibs, SFX and (later) barks
//! subscribe to those instead of each re-deriving who got hurt where.

use bevy::prelude::*;
use std::collections::VecDeque;

/// Broad category of damage, used to pick the gore/SFX response.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum DamageKind {
    /// Bullets, thrown balls — a sharp local spray.
    #[default]
    Ballistic,
    /// Melee / crushing.
    Blunt,
    /// Molotovs, burning ground — scorch, not spray.
    Fire,
    /// Grenades — big radial burst. Used once the grenade path lands (see plan §4).
    #[allow(dead_code)]
    Explosive,
}

/// Emitted wherever damage is applied to an entity. The single source of truth for
/// "something got hurt here", so gore/audio/score are subscribers, not special cases.
#[derive(Message, Clone, Copy, Debug)]
pub struct DamageDealt {
    pub target: Entity,
    /// World position of the hit.
    pub position: Vec3,
    /// Unit direction the gore should spray — i.e. pointing *away* from the source,
    /// roughly the surface normal at the impact.
    pub normal: Vec3,
    pub amount: i32,
    pub kind: DamageKind,
    /// True when this blow dropped the target to `health <= 0`.
    pub lethal: bool,
}

/// Emitted once when an entity dies, before it is despawned. Gibs and death SFX key
/// off this rather than polling `Health <= 0` themselves.
#[derive(Message, Clone, Copy, Debug)]
pub struct EntityDied {
    pub entity: Entity,
    pub position: Vec3,
    /// Direction of the killing blow, for throwing gibs the right way.
    pub normal: Vec3,
    /// How they died — lets fire deaths char instead of gib (see plan §4). Not yet
    /// branched on.
    #[allow(dead_code)]
    pub kind: DamageKind,
}

/// Remembers the last hit an entity took, so the death path (which only sees
/// `Health <= 0`) can attribute direction/kind to the resulting [`EntityDied`].
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct LastHit {
    pub normal: Vec3,
    pub kind: DamageKind,
}

/// A short-lived visual entity that grows and/or fades, then despawns. Generalises
/// the old bespoke `DeathEffect` tick so every FX (blood puff, dust, muzzle flash)
/// shares one system.
#[derive(Component)]
pub struct Ephemeral {
    pub timer: Timer,
    /// Fade `base_color` alpha linearly to 0 across the lifetime (needs a
    /// `MeshMaterial3d<StandardMaterial>`).
    pub fade: bool,
    /// Uniform scale reached at end of life, as a multiple of `base_scale`
    /// (1.0 = no growth).
    pub grow_to: f32,
    /// Scale at spawn; the grow animation is relative to this.
    pub base_scale: Vec3,
}

impl Ephemeral {
    pub fn new(secs: f32) -> Self {
        Self {
            timer: Timer::from_seconds(secs, TimerMode::Once),
            fade: true,
            grow_to: 1.0,
            base_scale: Vec3::ONE,
        }
    }

    pub fn with_grow(mut self, grow_to: f32) -> Self {
        self.grow_to = grow_to;
        self
    }

    pub fn base_scale(mut self, base_scale: Vec3) -> Self {
        self.base_scale = base_scale;
        self
    }

    #[allow(dead_code)]
    pub fn no_fade(mut self) -> Self {
        self.fade = false;
        self
    }
}

/// A cap on how many persistent gore entities can live at once, so a big wave can't
/// spawn thousands of decals/gibs and tank the frame. Recycles oldest-first.
#[derive(Resource)]
pub struct GoreBudget {
    pub max_decals: usize,
    pub max_gibs: usize,
    decals: VecDeque<Entity>,
    gibs: VecDeque<Entity>,
}

impl Default for GoreBudget {
    fn default() -> Self {
        Self {
            max_decals: 400,
            max_gibs: 200,
            decals: VecDeque::new(),
            gibs: VecDeque::new(),
        }
    }
}

impl GoreBudget {
    /// Register a freshly-spawned persistent decal. Returns the entity that fell out
    /// of the budget (to be despawned by the caller), if any.
    pub fn push_decal(&mut self, e: Entity) -> Option<Entity> {
        self.decals.push_back(e);
        if self.decals.len() > self.max_decals {
            self.decals.pop_front()
        } else {
            None
        }
    }

    /// As [`push_decal`](Self::push_decal), for gibs.
    #[allow(dead_code)]
    pub fn push_gib(&mut self, e: Entity) -> Option<Entity> {
        self.gibs.push_back(e);
        if self.gibs.len() > self.max_gibs {
            self.gibs.pop_front()
        } else {
            None
        }
    }
}
