//! `SpatialQuery` with Avian's methods, over the Jolt world.

use bevy::ecs::entity::EntityHashSet;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use oxijolt::{BodyId, QueryFilter, RayCast, ShapeCast};

use super::components::Collider;
use super::layers::LayerMask;
use super::world::JoltWorld;

/// Which colliders a query sees: memberships meeting `mask`, minus `excluded_entities`.
#[derive(Clone, Debug, PartialEq)]
pub struct SpatialQueryFilter {
    pub mask: LayerMask,
    pub excluded_entities: EntityHashSet,
}

impl SpatialQueryFilter {
    pub const DEFAULT: Self = Self { mask: LayerMask::ALL, excluded_entities: EntityHashSet::new() };

    pub fn from_mask(mask: impl Into<LayerMask>) -> Self {
        Self { mask: mask.into(), ..Self::DEFAULT }
    }

    pub fn from_excluded_entities(entities: impl IntoIterator<Item = Entity>) -> Self {
        Self { excluded_entities: entities.into_iter().collect(), ..Self::DEFAULT }
    }

    pub fn with_mask(mut self, mask: impl Into<LayerMask>) -> Self {
        self.mask = mask.into();
        self
    }

    pub fn with_excluded_entities(mut self, entities: impl IntoIterator<Item = Entity>) -> Self {
        self.excluded_entities = entities.into_iter().collect();
        self
    }
}

impl Default for SpatialQueryFilter {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeCastConfig {
    pub max_distance: f32,
    pub target_distance: f32,
    pub compute_contact_on_penetration: bool,
    pub ignore_origin_penetration: bool,
}

impl Default for ShapeCastConfig {
    fn default() -> Self {
        Self {
            max_distance: f32::MAX,
            target_distance: 0.0,
            compute_contact_on_penetration: true,
            ignore_origin_penetration: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayHitData {
    pub entity: Entity,
    pub distance: f32,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeHitData {
    pub entity: Entity,
    pub distance: f32,
    /// On the cast shape.
    pub point1: Vec3,
    /// On the hit collider.
    pub point2: Vec3,
    pub normal1: Vec3,
    pub normal2: Vec3,
}

/// Jolt measures casts as a vector; Avian lets a ray be endless.
const MAX_CAST: f32 = 4_000.0;
/// Excluded colliders in the way are skipped by casting again without the one just hit. Jolt's
/// filter leaves out one body at a time, so two excluded colliders on one line can make a cast
/// give up (`None`) after this many tries. The game excludes nothing today.
const MAX_SKIPS: usize = 8;

#[derive(SystemParam)]
pub struct SpatialQuery<'w> {
    jolt: Res<'w, JoltWorld>,
}

impl SpatialQuery<'_> {
    /// The object layers the filter sees, `None` when it sees nothing (Jolt reads an empty list
    /// as "everything").
    fn layers(&self, filter: &SpatialQueryFilter) -> Option<Vec<oxijolt::ObjectLayer>> {
        let layers = self.jolt.layers.seen_by(filter.mask);
        (!layers.is_empty()).then_some(layers)
    }

    fn excluded(&self, filter: &SpatialQueryFilter, body: BodyId) -> bool {
        !filter.excluded_entities.is_empty()
            && self.jolt.entity(body).is_some_and(|e| filter.excluded_entities.contains(&e))
    }

    /// Runs `cast` with the filter's layers, skipping excluded colliders; `cast` gets the Jolt
    /// filter to use and returns the hit body with its hit.
    fn skipping<T>(
        &self,
        filter: &SpatialQueryFilter,
        cast: impl Fn(&QueryFilter<'_>) -> Option<(BodyId, T)>,
    ) -> Option<(Entity, T)> {
        let layers = self.layers(filter)?;
        let mut skip = None;
        for _ in 0..=MAX_SKIPS {
            let mut q = QueryFilter::new().object_layers(&layers);
            if let Some(body) = skip {
                q = q.exclude_body(body);
            }
            let (body, hit) = cast(&q)?;
            if self.excluded(filter, body) {
                skip = Some(body);
                continue;
            }
            return Some((self.jolt.entity(body)?, hit));
        }
        None
    }

    /// The closest hit along the ray, within `max_distance`. Convex colliders are always solid
    /// in Jolt, so `solid` is accepted and ignored: a ray starting inside one hits at 0.
    pub fn cast_ray(
        &self,
        origin: Vec3,
        direction: Dir3,
        max_distance: f32,
        _solid: bool,
        filter: &SpatialQueryFilter,
    ) -> Option<RayHitData> {
        let ray = RayCast::new(origin.into(), (*direction * max_distance.min(MAX_CAST)).into());
        let (entity, hit) = self.skipping(filter, |q| {
            let hit = self.jolt.world.cast_ray(&ray, q).ok()??;
            Some((hit.body, hit))
        })?;
        Some(RayHitData { entity, distance: hit.distance, normal: hit.normal.into() })
    }

    /// The first collider `shape` hits moving from `origin` along `direction`.
    pub fn cast_shape(
        &self,
        shape: &Collider,
        origin: Vec3,
        rotation: Quat,
        direction: Dir3,
        config: &ShapeCastConfig,
        filter: &SpatialQueryFilter,
    ) -> Option<ShapeHitData> {
        let travel = *direction * config.max_distance.min(MAX_CAST);
        let cast = ShapeCast::new(shape.shape(), origin.into(), rotation.normalize().into(), travel.into())
            .return_deepest_point(config.compute_contact_on_penetration);
        let (entity, hit) = self.skipping(filter, |q| {
            let hit = self.jolt.world.cast_shape(&cast, q).ok()??;
            if config.ignore_origin_penetration && hit.fraction == 0.0 {
                return None;
            }
            Some((hit.body, hit))
        })?;
        let point: Vec3 = hit.point.into();
        let normal: Vec3 = hit.normal.into();
        Some(ShapeHitData {
            entity,
            distance: hit.distance,
            point1: point,
            point2: point,
            normal1: -normal,
            normal2: normal,
        })
    }
}
