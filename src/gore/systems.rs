//! Cross-cutting gore systems: the generic ephemeral tick and last-hit bookkeeping.

use bevy::prelude::*;

use crate::gore::components::{DamageDealt, Ephemeral, LastHit};

/// Grow/fade every [`Ephemeral`] toward the end of its life, then despawn it.
pub fn tick_ephemeral(
    time: Res<Time>,
    mut commands: Commands,
    mut query: Query<(
        Entity,
        &mut Ephemeral,
        &mut Transform,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, mut eph, mut transform, material) in query.iter_mut() {
        eph.timer.tick(time.delta());
        let t = eph.timer.fraction();

        if eph.grow_to != 1.0 {
            let s = 1.0 + (eph.grow_to - 1.0) * t;
            transform.scale = eph.base_scale * s;
        }

        if eph.fade
            && let Some(mat_handle) = material
            && let Some(mat) = materials.get_mut(&mat_handle.0)
        {
            let c = mat.base_color.to_srgba();
            mat.base_color = Color::srgba(c.red, c.green, c.blue, 1.0 - t);
        }

        if eph.timer.is_finished() {
            commands.entity(entity).despawn();
        }
    }
}

/// Stamp each damaged entity with the direction/kind of its most recent hit, so the
/// death path can attribute the killing blow when it emits `EntityDied`.
pub fn record_last_hit(mut damage: MessageReader<DamageDealt>, mut commands: Commands) {
    for hit in damage.read() {
        commands.entity(hit.target).try_insert(LastHit {
            normal: hit.normal,
            kind: hit.kind,
        });
    }
}
