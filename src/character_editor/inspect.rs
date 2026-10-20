//! Inspect mode: the tools for placing hardpoints by eye.
//!
//! Two independent switches on the debug panel, both off by default:
//!
//! - **Orbit camera** (`F11`): a Blender-style camera in place of the follow camera.
//!   Middle-drag orbits, Shift+middle-drag pans, Ctrl+middle-drag or the wheel zooms,
//!   `F` / numpad `.` frames the hardpoint being edited (or the character, with none
//!   selected), `Home` frames the character. Perspective, because judging where a hand is
//!   in depth is what the isometric ortho view cannot do.
//! - **Rest pose** (`F12`): the skeleton held in its bind pose — the pose the mesh was
//!   skinned in, a T-pose on most rigs — so a grip can be placed against a hand that is
//!   not swinging. Read from the skin, not the node transforms; see
//!   `gltf_info::rest_pose` for why.
//!
//! While either is on the character holds still and faces forward: the keyboard and mouse
//! are busy with the camera, and a body turning to follow the cursor would swing the very
//! frame being nudged.
//!
//! ## Why the rest pose is written last, not the animation paused
//!
//! The character editor is the live game. Pausing the clip still leaves the torso twist, the arm
//! and leg IK and the animation state machine all writing bones, and each would need its
//! own gate. Instead [`hold_rest_pose`] runs after all of them, just before propagation,
//! and overwrites every joint. Whatever they did this frame never reaches the screen.

use std::collections::HashMap;

use bevy::camera::primitives::Aabb;
use bevy::input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit};
use bevy::mesh::skinning::SkinnedMesh;
use bevy::prelude::*;
use bevy::window::PrimaryWindow;

use crate::assets::asset_definition::Hardpoint;
use crate::assets::gizmos::{bone_map, joints_under};
use crate::assets::gltf_info;
use crate::camera::components::GameCamera;
use crate::character_editor::debug::EditorOverlays;
use crate::character_editor::doc::{PropDoc, WeaponDoc};
use crate::character_editor::hardpoints::{HardpointEditor, HardpointSide};
use crate::character_editor::prop::{EditorProp, mesh_bounds};
use crate::character_editor::state::EditorSession;
use crate::control::components::CharacterControl;
use crate::model_settings::plugin::PlayerAssetDef;
use crate::player::components::{AutoAim, Player};
use crate::player::systems::equip::EquippedWeapon;
use crate::settings::resources::GameSettings;

/// Radians per unit of mouse motion when orbiting.
const ORBIT_SPEED: f32 = 0.008;
/// Stops short of straight up/down, where `look_at` loses its up vector and flips.
const MAX_PITCH: f32 = 1.55;
/// Distance multiplier per wheel notch.
const DOLLY_STEP: f32 = 1.15;
/// Wheel notches per unit of Ctrl+drag motion.
const DRAG_DOLLY_SPEED: f32 = 0.02;
const MIN_DISTANCE: f32 = 0.02;
const MAX_DISTANCE: f32 = 60.0;
/// Share of the pane's height the character fills when framed: head to toe, a margin.
const FRAME_CHARACTER_FILL: f32 = 0.8;
/// How much of the character's size "frame selected" shows around a hardpoint: about a
/// forearm, so a hand and a gun fill the pane.
const FRAME_HARDPOINT_SPAN: f32 = 0.3;
/// How long the camera keeps re-framing a freshly spawned character, about half a second.
const SETTLE_FRAMES: u32 = 30;
/// Stand-in skeleton radius before the joints have spawned.
const FALLBACK_RADIUS: f32 = 1.0;
/// Pixel-unit wheels (touchpads) report far larger numbers than line-unit ones.
const PIXELS_PER_NOTCH: f32 = 40.0;
const FOV_DEGREES: f32 = 40.0;
/// Close enough to put the lens against a fingertip without clipping it.
const NEAR_PLANE: f32 = 0.005;
const FAR_PLANE: f32 = 500.0;
/// Looking a little down on the character when the camera is switched on.
const START_PITCH: f32 = 0.35;

/// The orbit camera's state: a point it looks at and where it sits around it.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct OrbitCamera {
    pub pivot: Vec3,
    /// About world up; zero looks from +Z.
    pub yaw: f32,
    /// Above the horizon; positive looks down.
    pub pitch: f32,
    pub distance: f32,
    /// Where the character was last frame. The pivot moves with it, so a fall, a respawn
    /// or a model swap keeps the framing instead of leaving the camera staring at the
    /// spot the character used to be.
    pub tracking: Vec3,
    /// Frames left to keep framing the character. Set on switching on: the skeleton may
    /// not have spawned yet, and once it has, the model's scale fix lands a frame or two
    /// later — framing once would lock in a size it is about to lose.
    pub reframe: u32,
    /// A middle-drag that began over the viewport pane. Kept so a drag carries on when the
    /// cursor leaves the pane mid-gesture, and one begun over the UI never moves the camera.
    pub dragging: bool,
    /// What the camera is following: the character, or a prop opened in its place. When
    /// it changes the camera re-frames rather than sliding by the distance between them.
    pub subject: Option<Entity>,
}

impl OrbitCamera {
    /// Orbit `pivot` from where `eye` is now, so switching the camera on keeps the view
    /// angle you were already looking from.
    pub fn looking_from(pivot: Vec3, eye: Vec3) -> Self {
        let offset = eye - pivot;
        let distance = offset.length().clamp(MIN_DISTANCE, MAX_DISTANCE);
        let dir = offset.normalize_or(Vec3::new(0.0, 0.7, 0.7).normalize());
        Self {
            pivot,
            yaw: dir.x.atan2(dir.z),
            pitch: dir.y.clamp(-1.0, 1.0).asin().clamp(-MAX_PITCH, MAX_PITCH),
            distance,
            tracking: pivot,
            reframe: 0,
            dragging: false,
            subject: None,
        }
    }

    pub fn eye(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        self.pivot + Vec3::new(sin_yaw * cos_pitch, sin_pitch, cos_yaw * cos_pitch) * self.distance
    }

    pub fn transform(&self) -> Transform {
        Transform::from_translation(self.eye()).looking_at(self.pivot, Vec3::Y)
    }

    /// Drag right swings the camera left round the pivot, so the model turns the way the
    /// mouse went; drag down raises the camera. Both as in Blender.
    pub fn orbit(&mut self, delta: Vec2) {
        self.yaw -= delta.x * ORBIT_SPEED;
        self.pitch = (self.pitch + delta.y * ORBIT_SPEED).clamp(-MAX_PITCH, MAX_PITCH);
    }

    /// Slide the pivot in the view plane so the scene follows the cursor ("grab"), at
    /// `world_per_pixel` — the size of one pixel at the pivot's depth.
    pub fn pan(&mut self, delta: Vec2, world_per_pixel: f32) {
        let view = self.transform();
        let right = view.right().as_vec3();
        let up = view.up().as_vec3();
        self.pivot += (-right * delta.x + up * delta.y) * world_per_pixel;
    }

    /// Positive steps move away, negative closer. Multiplicative, so a notch feels the
    /// same at a fingertip as across the arena.
    pub fn dolly(&mut self, steps: f32) {
        self.distance = (self.distance * DOLLY_STEP.powf(steps)).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    pub const fn frame(&mut self, pivot: Vec3, distance: f32) {
        self.pivot = pivot;
        self.distance = distance;
    }

    /// Carry the pivot along by however far the character moved since last frame.
    pub fn follow(&mut self, character: Vec3) {
        self.pivot += character - self.tracking;
        self.tracking = character;
    }
}

/// The orbit camera is driving the game camera, so `camera_follow` must not.
///
/// Checks the session too: `EditorOverlays` outlives an editor session, and a flag
/// left on must not freeze the camera of the next real match.
pub fn orbit_camera_active(
    session: Option<Res<EditorSession>>,
    debug: Option<Res<EditorOverlays>>,
) -> bool {
    session.is_some() && debug.is_some_and(|d| d.orbit_camera)
}

pub fn rest_pose_held(debug: Option<Res<EditorOverlays>>) -> bool {
    debug.is_some_and(|d| d.rest_pose)
}

pub fn inspecting(debug: Option<Res<EditorOverlays>>) -> bool {
    debug.is_some_and(|d| d.inspecting())
}

/// `F11` / `F12` toggle the two tools. Every other F-key up to F10 is taken.
pub fn inspect_keys(keys: Res<ButtonInput<KeyCode>>, mut debug: ResMut<EditorOverlays>) {
    if keys.just_pressed(KeyCode::F11) {
        debug.toggle_orbit_camera();
    }
    if keys.just_pressed(KeyCode::F12) {
        debug.toggle_rest_pose();
    }
}

/// Hand the game camera over to the orbit camera and back.
///
/// On: start orbiting the character from the game camera's heading, in perspective. Off:
/// mark `GameSettings` changed so `apply_camera_settings` puts the configured projection
/// back, and `camera_follow` (no longer gated) eases back to the players.
pub fn switch_orbit_camera(
    mut commands: Commands,
    debug: Res<EditorOverlays>,
    mut was_on: Local<bool>,
    mut settings: ResMut<GameSettings>,
    players: Query<&Transform, With<Player>>,
    props: Query<&Transform, With<EditorProp>>,
) {
    if debug.orbit_camera == *was_on {
        return;
    }

    if debug.orbit_camera {
        // Wait for something to orbit: switched on before a character has spawned (or mid
        // model swap), the camera would settle on the world origin instead.
        let Some(pivot) = props
            .iter()
            .next()
            .or_else(|| players.iter().next())
            .map(|t| t.translation)
        else {
            return;
        };
        *was_on = true;
        // Start from the game camera's heading, read from the settings rather than the
        // camera itself: the follow camera may not have placed itself yet, and an eye at
        // the world origin points the view at whatever wall is in between.
        let (sin_yaw, cos_yaw) = settings.yaw_degrees.to_radians().sin_cos();
        let mut orbit = OrbitCamera::looking_from(pivot, pivot + Vec3::new(sin_yaw, 0.0, cos_yaw));
        orbit.pitch = START_PITCH;
        orbit.reframe = SETTLE_FRAMES;
        commands.insert_resource(orbit);
    } else {
        *was_on = false;
        commands.remove_resource::<OrbitCamera>();
        settings.set_changed();
    }
}

/// Read the mouse into the [`OrbitCamera`] and place the game camera from it. Runs where
/// `camera_follow` would have, just before propagation.
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn drive_orbit_camera(
    orbit: Option<ResMut<OrbitCamera>>,
    mouse: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(&Camera, &mut Transform, &mut Projection), With<GameCamera>>,
    focus: FocusTargets,
) {
    let Some(mut orbit) = orbit else {
        return;
    };
    let subject = focus.subject_root();
    if subject != orbit.subject {
        // A prop opened, or closed again: frame the new subject from where it stands
        // rather than sliding the camera by however far apart the two are.
        orbit.subject = subject;
        if let Some(at) = focus.origin() {
            orbit.tracking = at;
        }
        orbit.reframe = SETTLE_FRAMES;
    }
    if let Some(at) = focus.origin() {
        orbit.follow(at);
    }
    let bounds = focus.character_bounds();
    if orbit.reframe > 0
        && let Some((center, radius)) = bounds
    {
        orbit.frame(center, character_distance(radius));
        orbit.reframe -= 1;
    }
    let Ok((camera, mut transform, mut projection)) = cameras.single_mut() else {
        return;
    };

    // Our own lens, not merely *a* perspective one: the game may already run in
    // perspective with a far-off near plane and a telephoto FOV (`game-settings.ron` has
    // 2 m and 10 degrees), which clips a close-up and throws off the pan and framing
    // maths. Re-asserted every frame because the F1 camera panel and the zoom keys
    // rewrite the projection whenever they touch `GameSettings`.
    if !is_inspect_lens(&projection) {
        *projection = Projection::Perspective(PerspectiveProjection {
            fov: FOV_DEGREES.to_radians(),
            near: NEAR_PLANE,
            far: FAR_PLANE,
            ..default()
        });
    }

    let pane = camera.logical_viewport_rect();
    let over_pane = windows
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .zip(pane)
        .is_some_and(|(cursor, rect)| rect.contains(cursor));

    if mouse.just_pressed(MouseButton::Middle) && over_pane {
        orbit.dragging = true;
    }
    if !mouse.pressed(MouseButton::Middle) {
        orbit.dragging = false;
    }

    let shift = keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]);
    let ctrl = keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]);
    let delta = motion.delta;
    if orbit.dragging && delta != Vec2::ZERO {
        if shift {
            let height = pane.map_or(720.0, |r| r.height()).max(1.0);
            let world_per_pixel =
                2.0 * orbit.distance * (FOV_DEGREES.to_radians() * 0.5).tan() / height;
            orbit.pan(delta, world_per_pixel);
        } else if ctrl {
            orbit.dolly(delta.y * DRAG_DOLLY_SPEED);
        } else {
            orbit.orbit(delta);
        }
    }

    if over_pane && scroll.delta.y != 0.0 {
        let notches = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y,
            MouseScrollUnit::Pixel => scroll.delta.y / PIXELS_PER_NOTCH,
        };
        // Wheel up comes closer.
        orbit.dolly(-notches);
    }

    // Distances scale with the skeleton: characters here are drawn at a quarter of life
    // size, and a fixed distance would frame them as a speck.
    let radius = bounds.map_or(FALLBACK_RADIUS, |(_, r)| r);
    let frame_selected = keys.any_just_pressed([KeyCode::KeyF, KeyCode::NumpadDecimal]);
    if frame_selected && let Some(at) = focus.active_hardpoint() {
        orbit.frame(at, character_distance(radius * FRAME_HARDPOINT_SPAN));
    } else if (frame_selected || keys.just_pressed(KeyCode::Home))
        && let Some((center, radius)) = bounds
    {
        orbit.frame(center, character_distance(radius));
    }

    *transform = orbit.transform();
}

/// Already showing the inspect lens. Checked rather than written blindly, so the camera's
/// projection is not marked changed every frame.
fn is_inspect_lens(projection: &Projection) -> bool {
    matches!(projection, Projection::Perspective(p)
        if (p.fov - FOV_DEGREES.to_radians()).abs() < 1e-4
            && (p.near - NEAR_PLANE).abs() < 1e-6
            && (p.far - FAR_PLANE).abs() < 1e-3)
}

/// What "frame selected" can aim at: the hardpoint open in the editor, or the character.
#[derive(bevy::ecs::system::SystemParam)]
pub struct FocusTargets<'w, 's> {
    editor: Option<Res<'w, HardpointEditor>>,
    player_def: Res<'w, PlayerAssetDef>,
    weapon_def: Res<'w, WeaponDoc>,
    prop_def: Res<'w, PropDoc>,
    players: Query<'w, 's, (Entity, Option<&'static EquippedWeapon>), With<Player>>,
    props: Query<'w, 's, Entity, With<EditorProp>>,
    aabbs: Query<'w, 's, (&'static Aabb, &'static GlobalTransform)>,
    children_q: Query<'w, 's, &'static Children>,
    skinned_q: Query<'w, 's, &'static SkinnedMesh>,
    names: Query<'w, 's, &'static Name>,
    globals: Query<'w, 's, &'static GlobalTransform>,
}

/// How far back a sphere of `radius` fills [`FRAME_CHARACTER_FILL`] of the pane.
fn character_distance(radius: f32) -> f32 {
    (radius / ((FOV_DEGREES.to_radians() * 0.5).tan() * FRAME_CHARACTER_FILL))
        .clamp(MIN_DISTANCE, MAX_DISTANCE)
}

impl FocusTargets<'_, '_> {
    /// What is being looked at: a prop opened on its own if there is one, otherwise the
    /// character.
    fn subject_root(&self) -> Option<Entity> {
        self.props
            .iter()
            .next()
            .or_else(|| self.players.iter().next().map(|(player, _)| player))
    }

    /// The subject's root position: steady, unlike the animated skeleton, so it is what
    /// the pivot follows.
    fn origin(&self) -> Option<Vec3> {
        let root = self.subject_root()?;
        self.globals
            .get(root)
            .ok()
            .map(GlobalTransform::translation)
    }

    /// Centre and radius of the subject. A prop is measured by its meshes, since a barrel
    /// or a tower has no skeleton; the character by its joints, which are steadier than
    /// a skinned mesh's bounds. `None` until it has spawned.
    fn character_bounds(&self) -> Option<(Vec3, f32)> {
        if let Some(prop) = self.props.iter().next() {
            return mesh_bounds(prop, &self.children_q, &self.aabbs);
        }
        let (player, _) = self.players.iter().next()?;
        let joints = joints_under(player, &self.children_q, &self.skinned_q);
        let mut points = joints
            .iter()
            .filter_map(|&j| self.globals.get(j).ok())
            // A joint spawned this frame has not been propagated yet and still sits at
            // the world origin, which would stretch the box across the arena.
            .filter(|gt| **gt != GlobalTransform::IDENTITY)
            .map(GlobalTransform::translation);
        let first = points.next()?;
        let (min, max) = points.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p)));
        Some((
            (min + max) * 0.5,
            ((max - min).length() * 0.5).max(MIN_DISTANCE),
        ))
    }

    /// World position of the hardpoint selected in the editor, on whichever side it is
    /// being edited — resolved the same way the overlay draws it.
    fn active_hardpoint(&self) -> Option<Vec3> {
        let editor = self.editor.as_ref()?;
        let role = editor.active_role.as_deref()?;
        let (player, equipped) = self.players.iter().next()?;

        let (hardpoint, anchor): (&Hardpoint, Entity) = match editor.side {
            HardpointSide::Character => {
                let hardpoint = self.player_def.0.as_ref()?.hardpoints.get(role)?;
                let anchor = match &hardpoint.anchor {
                    None => player,
                    Some(bone) => {
                        let joints = joints_under(player, &self.children_q, &self.skinned_q);
                        *bone_map(&joints, &self.names).get(bone)?
                    }
                };
                (hardpoint, anchor)
            }
            // Weapon frames are relative to the model origin: the spawned weapon itself.
            HardpointSide::Weapon => (
                self.weapon_def.def.as_ref()?.hardpoints.get(role)?,
                equipped?.0,
            ),
            HardpointSide::Prop => {
                let prop = self.props.iter().next()?;
                let hardpoint = self.prop_def.def.as_ref()?.hardpoints.get(role)?;
                let anchor = match &hardpoint.anchor {
                    None => prop,
                    Some(bone) => {
                        let joints = joints_under(prop, &self.children_q, &self.skinned_q);
                        *bone_map(&joints, &self.names).get(bone)?
                    }
                };
                (hardpoint, anchor)
            }
        };
        let at = self.globals.get(anchor).ok()?;
        Some(at.transform_point(Vec3::from(hardpoint.translation)))
    }
}

/// Keep the character still and facing forward while inspecting: no walking, and the aim
/// pinned to the body so the torso and the weapon stop following the cursor around.
///
/// Runs after the input systems have written this frame's intent and before movement acts
/// on it. The held keys are left alone, so a key still down when inspecting ends walks on.
pub fn hold_player_still(
    mut players: Query<(&Transform, &mut CharacterControl, &mut AutoAim), With<Player>>,
) {
    for (transform, mut control, mut aim) in &mut players {
        control.walk_direction = Vec3::ZERO;
        control.torque = Vec3::ZERO;
        aim.0 = transform.rotation * Vec3::NEG_Z;
    }
}

/// Rest transforms for the model being worn, read off its file. Keyed by model path so a
/// model swap re-reads it; a file that cannot be read caches empty, rather than retrying
/// (and warning) every frame.
#[derive(Default)]
pub struct RestPoseCache {
    model_path: Option<String>,
    bones: HashMap<String, Transform>,
}

/// Write the rest pose onto every joint of every player, after the animation, twist and IK
/// have had their turn — see the module docs.
pub fn hold_rest_pose(
    player_def: Res<PlayerAssetDef>,
    mut cache: Local<RestPoseCache>,
    players: Query<Entity, With<Player>>,
    children_q: Query<&Children>,
    skinned_q: Query<&SkinnedMesh>,
    names: Query<&Name>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(def) = player_def.0.as_ref() else {
        return;
    };
    if cache.model_path.as_deref() != Some(def.model_path.as_str()) {
        cache.bones = gltf_info::rest_pose(&def.model_path).unwrap_or_else(|| {
            warn!(
                "character editor: cannot read a rest pose from {}",
                def.model_path
            );
            HashMap::new()
        });
        cache.model_path = Some(def.model_path.clone());
    }

    for player in &players {
        for joint in joints_under(player, &children_q, &skinned_q) {
            let Some(rest) = names
                .get(joint)
                .ok()
                .and_then(|n| cache.bones.get(n.as_str()))
            else {
                continue;
            };
            if let Ok(mut transform) = transforms.get_mut(joint) {
                *transform = *rest;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        a.distance(b) < 1e-4
    }

    #[test]
    fn looking_from_reproduces_the_eye_it_was_given() {
        let pivot = Vec3::new(1.0, 0.5, -2.0);
        let eye = Vec3::new(4.0, 3.0, 2.0);
        let orbit = OrbitCamera::looking_from(pivot, eye);
        assert!(close(orbit.eye(), eye), "{} vs {eye}", orbit.eye());
    }

    #[test]
    fn the_camera_looks_at_the_pivot() {
        let orbit = OrbitCamera::looking_from(Vec3::ZERO, Vec3::new(2.0, 1.0, 3.0));
        let t = orbit.transform();
        let towards = (orbit.pivot - t.translation).normalize();
        assert!(close(t.forward().as_vec3(), towards));
    }

    #[test]
    fn orbiting_keeps_the_distance_and_the_pivot() {
        let mut orbit = OrbitCamera::looking_from(Vec3::Y, Vec3::new(0.0, 1.0, 3.0));
        orbit.orbit(Vec2::new(120.0, -40.0));
        assert!(close(orbit.pivot, Vec3::Y));
        assert!((orbit.eye().distance(orbit.pivot) - 3.0).abs() < 1e-4);
    }

    /// Past straight up `look_at` flips the view; the pitch stops short of it.
    #[test]
    fn pitch_is_clamped_short_of_the_poles() {
        let mut orbit = OrbitCamera::looking_from(Vec3::ZERO, Vec3::Z);
        orbit.orbit(Vec2::new(0.0, 1.0e6));
        assert!(orbit.pitch <= MAX_PITCH);
        orbit.orbit(Vec2::new(0.0, -1.0e6));
        assert!(orbit.pitch >= -MAX_PITCH);
    }

    /// Grab semantics: drag right and the scene follows the cursor right, which means the
    /// pivot moves to the camera's left.
    #[test]
    fn panning_right_moves_the_pivot_to_the_cameras_left() {
        let mut orbit = OrbitCamera::looking_from(Vec3::ZERO, Vec3::new(0.0, 0.0, 5.0));
        let right = orbit.transform().right().as_vec3();
        orbit.pan(Vec2::new(10.0, 0.0), 0.01);
        assert!(orbit.pivot.dot(right) < 0.0);
        assert!((orbit.pivot.length() - 0.1).abs() < 1e-4);
    }

    /// The character drops onto the floor after spawning; the view has to come with it.
    #[test]
    fn the_pivot_rides_along_with_the_character() {
        let mut orbit = OrbitCamera::looking_from(Vec3::Y, Vec3::new(0.0, 1.0, 3.0));
        orbit.pan(Vec2::new(10.0, 0.0), 0.01);
        let panned = orbit.pivot;
        orbit.follow(Vec3::new(0.0, -0.5, 0.0));
        assert!(close(orbit.pivot, panned - Vec3::Y * 1.5));
        orbit.follow(Vec3::new(0.0, -0.5, 0.0));
        assert!(
            close(orbit.pivot, panned - Vec3::Y * 1.5),
            "standing still moves nothing"
        );
    }

    /// A quarter-size character and a life-size one fill the pane alike.
    #[test]
    fn framing_distance_scales_with_the_character() {
        let small = character_distance(0.2);
        let large = character_distance(0.8);
        assert!((large / small - 4.0).abs() < 1e-4);
        let visible_half_height = small * (FOV_DEGREES.to_radians() * 0.5).tan();
        assert!((0.2 / visible_half_height - FRAME_CHARACTER_FILL).abs() < 1e-4);
    }

    /// The game's own perspective (2 m near plane, 10 degree FOV) is not good enough: it
    /// clips every close-up. Only the exact inspect lens passes.
    #[test]
    fn only_the_inspect_lens_counts_as_the_inspect_lens() {
        let game = Projection::Perspective(PerspectiveProjection {
            fov: 10.0_f32.to_radians(),
            near: 2.0,
            ..default()
        });
        assert!(!is_inspect_lens(&game));
        assert!(!is_inspect_lens(&Projection::Orthographic(
            OrthographicProjection::default_3d()
        )));
        let ours = Projection::Perspective(PerspectiveProjection {
            fov: FOV_DEGREES.to_radians(),
            near: NEAR_PLANE,
            far: FAR_PLANE,
            ..default()
        });
        assert!(is_inspect_lens(&ours));
    }

    #[test]
    fn dolly_is_multiplicative_and_clamped() {
        let mut orbit = OrbitCamera::looking_from(Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0));
        orbit.dolly(1.0);
        assert!((orbit.distance - 2.0 * DOLLY_STEP).abs() < 1e-4);
        orbit.dolly(-1.0);
        assert!((orbit.distance - 2.0).abs() < 1e-4);
        orbit.dolly(-1000.0);
        assert!((orbit.distance - MIN_DISTANCE).abs() < 1e-6);
    }

    #[test]
    fn the_orbit_camera_is_only_active_inside_a_session() {
        let mut app = App::new();
        app.insert_resource(EditorOverlays {
            orbit_camera: true,
            ..Default::default()
        });
        assert!(
            !app.world_mut()
                .run_system_cached(orbit_camera_active)
                .unwrap()
        );
        app.init_resource::<EditorSession>();
        assert!(
            app.world_mut()
                .run_system_cached(orbit_camera_active)
                .unwrap()
        );
    }
}
