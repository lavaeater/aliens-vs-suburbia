use bevy::prelude::*;
use avian3d::prelude::LinearVelocity;
use crate::animation::animation_plugin::{AnimationKey, CurrentAnimationKey};
use crate::camera::components::GameCamera;
use crate::player::components::Player;
use crate::sprite_billboard::components::*;
use crate::sprite_billboard::material::SpriteBillboardMaterial;

/// Initialise the shared quad mesh once.
pub fn setup_billboard_mesh(
    mut meshes: ResMut<Assets<Mesh>>,
    mut commands: Commands,
) {
    let handle = meshes.add(Rectangle::new(1.0, 1.5));
    commands.insert_resource(BillboardMeshHandle(handle));
}

/// Each frame: face camera, pick direction row, advance animation, update UV.
pub fn billboard_system(
    time: Res<Time>,
    camera_q: Query<&GlobalTransform, With<GameCamera>>,
    player_q: Query<(&LinearVelocity, &GlobalTransform, &CurrentAnimationKey), With<Player>>,
    mut billboard_q: Query<(
        &mut Transform,
        &mut SpriteBillboard,
        &MeshMaterial3d<SpriteBillboardMaterial>,
        &GlobalTransform,
    )>,
    mut materials: ResMut<Assets<SpriteBillboardMaterial>>,
) {
    let Ok(cam_gtf) = camera_q.single() else { return };
    let Ok((vel, player_gtf, anim_key)) = player_q.single() else { return };

    let vel_xz = Vec2::new(vel.x, vel.z);
    let is_moving = vel_xz.length_squared() > 0.04;

    // Direction: relative to camera view.
    // to_cam = unit vector from player toward camera in XZ.
    let cam_pos = cam_gtf.translation();
    let player_pos = player_gtf.translation();
    let to_cam_xz = {
        let d = Vec2::new(cam_pos.x - player_pos.x, cam_pos.z - player_pos.z);
        if d.length_squared() > 0.0 { d.normalize() } else { Vec2::Y }
    };
    // Camera-right in XZ = rotate to_cam 90° CCW.
    let cam_right_xz = Vec2::new(-to_cam_xz.y, to_cam_xz.x);

    let dir = if !is_moving {
        DIR_DOWN
    } else {
        let toward = vel_xz.dot(to_cam_xz);   // >0 = moving toward cam = front
        let rightward = vel_xz.dot(cam_right_xz);
        if toward.abs() >= rightward.abs() {
            if toward > 0.0 { DIR_DOWN } else { DIR_UP }
        } else {
            if rightward > 0.0 { DIR_RIGHT } else { DIR_LEFT }
        }
    };

    let target_anim = match anim_key.key {
        AnimationKey::Walking | AnimationKey::Crawling => SpriteAnim::Walk,
        _ => SpriteAnim::Idle,
    };

    for (mut tf, mut bb, mat_handle, gtf) in &mut billboard_q {
        // Reset frame when animation changes.
        if bb.anim != target_anim {
            bb.anim = target_anim;
            bb.frame = 0;
            bb.frame_timer = 0.0;
        }
        bb.dir = dir;

        // Advance frame.
        let (max_frames, fps) = match bb.anim {
            SpriteAnim::Idle => (IDLE_FRAMES, IDLE_FPS),
            SpriteAnim::Walk => (WALK_FRAMES, WALK_FPS),
        };
        bb.frame_timer += time.delta_secs();
        if bb.frame_timer >= 1.0 / fps {
            bb.frame_timer -= 1.0 / fps;
            bb.frame = (bb.frame + 1) % max_frames;
        }

        // Update material UV.
        if let Some(mat) = materials.get_mut(&mat_handle.0) {
            mat.uv_rect = bb.uv_rect();
        }

        // Rotate billboard to face camera (cylindrical Y-up billboard).
        let bill_pos = gtf.translation();
        let to_cam_3d = Vec3::new(cam_pos.x - bill_pos.x, 0.0, cam_pos.z - bill_pos.z);
        if to_cam_3d.length_squared() > 0.0 {
            tf.rotation = Quat::from_rotation_arc(Vec3::Z, to_cam_3d.normalize());
        }
    }
}
