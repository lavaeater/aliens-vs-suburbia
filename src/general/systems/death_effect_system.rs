use bevy::prelude::*;
use avian3d::prelude::Position;
use crate::general::components::Health;

#[derive(Component)]
pub struct DeathEffect {
    timer: Timer,
}

pub fn spawn_death_effects(
    dying: Query<(&Health, &Position)>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (health, pos) in &dying {
        if health.health <= 0 {
            let mat = materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.5, 0.0, 1.0),
                emissive: LinearRgba::new(3.0, 1.5, 0.0, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            });
            commands.spawn((
                DeathEffect { timer: Timer::from_seconds(0.35, TimerMode::Once) },
                Mesh3d(meshes.add(Sphere::new(0.2))),
                MeshMaterial3d(mat),
                Transform::from_translation(pos.0),
            ));
        }
    }
}

pub fn tick_death_effects(
    mut query: Query<(Entity, &mut DeathEffect, &mut Transform, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    time: Res<Time>,
    mut commands: Commands,
) {
    for (entity, mut effect, mut transform, mat_handle) in &mut query {
        effect.timer.tick(time.delta());
        let t = effect.timer.fraction();
        transform.scale = Vec3::splat(1.0 + t * 4.0);
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            let c = mat.base_color.to_srgba();
            mat.base_color = bevy::prelude::Color::srgba(c.red, c.green, c.blue, 1.0 - t);
        }
        if effect.timer.fraction() >= 1.0 {
            commands.entity(entity).despawn();
        }
    }
}
