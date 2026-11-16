//! The campaign intro (`docs/intro.md`): before the first level, the camera follows a
//! copter down through the clouds and low across the town to the players' start, the team
//! goes down ropes, the camera settles into the game's own isometric view and play
//! begins while the copter climbs away. The campaign's `intro` music plays over it.
//!
//! It runs inside `GameState::InGame`, the level already loaded and alive under it
//! (townsfolk walking, lamps lit). While it runs it holds back:
//! - the players: the map's `SpawnPlayer`s are kept ([`hold_spawns`]) and `spawn_players`
//!   waits ([`holding_players`]); stand-ins go down the ropes instead and the real
//!   players are spawned where they land;
//! - the stories (`StoryStore::active`), so the level does not start: no `LevelStarted`,
//!   no intro crawl, no waves or spawners (they wait for `LevelState::InProgress`);
//! - the camera: `camera_follow` and the camera settings wait ([`holding_camera`]) while
//!   [`drive_camera`] flies a perspective camera along `plan::FlightPlan::shot_at`. The
//!   hand-off narrows the field of view while backing off (a dolly zoom) until a 3 degree
//!   view matches the orthographic one, then switches;
//! - the generative music, ducked under the intro's own track until that ends.
//!
//! Esc / Enter / Space / Start / Cross skip to the hand-off. `--no-intro` leaves it out.
//! The flight is pure maths in [`plan`], the clouds in [`clouds`], the copter, ropes and
//! stand-ins in [`copter`].

pub mod clouds;
pub mod copter;
pub mod plan;

use bevy::camera::Projection;
use bevy::input::gamepad::Gamepad;
use bevy::prelude::*;
use bevy::transform::TransformSystems;
use bevy::world_serialization::WorldAssetRoot;
use bevy_seedling::prelude::{AudioSample, SamplePlayer, Volume};
use serde::{Deserialize, Serialize};

use crate::assets::asset_definition::AssetDefinition;
use crate::assets::assets_plugin::GameAssets;
use crate::camera::components::{CameraFocus, GameCamera};
use crate::campaign::Campaign;
use crate::character_editor::state::in_normal_game;
use crate::general::events::map_events::{LoadMap, SpawnPlayer};
use crate::general::systems::lights_systems::LightingPreset;
use crate::general::systems::map_systems::TileDefinitions;
use crate::map::ground::GroundHeight;
use crate::model_settings::resources::ModelSettings;
use crate::player::systems::spawn_players::FixSceneTransform;
use crate::player_setup::state::PlayerRoster;
use crate::settings::resources::GameSettings;
use crate::game_state::GameState;
use copter::{CopterLook, Rappeller, Rope};
use plan::{CloudDeck, FlightPlan, IntroTiming, IsoView};

/// A campaign's intro (`intro:` in `campaign.ron`). Every field may be left out.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct CampaignIntro {
    /// The track played over it, inside `assets/` (`"music/intro.mp3"`; mp3, ogg or
    /// wav). Empty = none.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub music: String,
    /// Its volume in dB.
    pub music_db: f32,
    pub copter: CopterLook,
    pub timing: IntroTiming,
    pub clouds: CloudDeck,
}

/// `--no-intro`: straight into the first level.
#[derive(Resource)]
pub struct NoIntro;

/// Players' bodies stand this high above the ground (`ground_follow`'s `standing`, near
/// enough): the stand-ins land here and the players are spawned here.
const STANDING: f32 = 0.3;
/// Seconds to wait for the level and the cloud noise before flying without clouds, and
/// for the players' start before giving up on the intro.
const WAIT_FOR_CLOUDS: f32 = 4.0;
const WAIT_FOR_LEVEL: f32 = 8.0;
/// Seconds before the intro can be skipped, so a key still down from the menu does not.
const SKIP_GRACE: f32 = 1.0;
/// The copter is gone this long after the hand-off.
const COPTER_GONE: f32 = 14.0;
/// The generative music is held this far down while the intro's track plays.
const DUCK_DB: f32 = 60.0;
/// Of the screen's height, each letterbox bar.
const LETTERBOX: f32 = 11.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// For the level and its players' start.
    Waiting,
    /// The flight and the ropes; the players are held back.
    Flying,
    /// The players are down; the camera settles into the game's view.
    HandingOver,
    /// Play is on; the copter leaves and the track plays out.
    Done,
}

/// A running intro. Present from entering the first level until leaving `InGame`.
#[derive(Resource)]
pub struct Intro {
    spec: CampaignIntro,
    stage: Stage,
    t: f32,
    waited: f32,
    plan: Option<FlightPlan>,
    held: Vec<SpawnPlayer>,
    /// The level's middle and how far it reaches from it.
    level: Option<(Vec3, f32)>,
    /// Where each stand-in lands, and when they did.
    landings: Vec<(Vec3, Option<f32>)>,
    copter: Option<Entity>,
    clouds: Option<Entity>,
    music: Option<Entity>,
    unducked: bool,
    background_scale: f32,
}

/// `--intro-at <secs>`: start the flight that far in (to look at one moment of it).
#[derive(Resource)]
pub struct IntroStartAt(pub f32);

/// The intro's track.
#[derive(Component)]
struct IntroMusic;

/// A letterbox bar.
#[derive(Component)]
struct Letterbox;

/// The players are held back (`spawn_players` waits, the stories are off).
pub fn holding_players(intro: Option<Res<Intro>>) -> bool {
    intro.is_some_and(|i| matches!(i.stage, Stage::Waiting | Stage::Flying))
}

/// The intro has the camera (`camera_follow` and the camera settings wait).
pub fn holding_camera(intro: Option<Res<Intro>>) -> bool {
    intro.is_some_and(|i| i.stage != Stage::Done)
}

pub struct IntroPlugin;

impl Plugin for IntroPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<clouds::CloudNoise>()
            .add_plugins(MaterialPlugin::<clouds::CloudMaterial>::default())
            .add_systems(Startup, clouds::start_making_noise)
            .add_systems(Update, clouds::finish_noise)
            .add_systems(
                OnEnter(GameState::InGame),
                begin_intro.run_if(in_normal_game),
            )
            .add_systems(OnExit(GameState::InGame), end_intro)
            .add_systems(
                Update,
                (
                    learn_level,
                    hold_spawns.run_if(holding_players),
                    take_off,
                    run_intro,
                    hold_stories.before(turbofacts::systems::check_stories),
                    intro_fog.after(crate::camera::fog::apply_fog),
                    letterbox,
                    stretch_background,
                    unduck_when_done,
                )
                    .chain()
                    .run_if(in_state(GameState::InGame))
                    .run_if(resource_exists::<Intro>),
            )
            .add_systems(
                Update,
                (
                    clouds::drift_clouds,
                    copter::find_model_rotors,
                    copter::find_rotor_hubs,
                    copter::spin_rotors,
                    copter::blink_beacons,
                    copter::animate_rappellers,
                )
                    .run_if(in_state(GameState::InGame)),
            )
            .add_systems(
                PostUpdate,
                drive_camera
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(GameState::InGame))
                    .run_if(holding_camera),
            );
    }
}

/// The first level of a fresh campaign, with an intro in its file: start one.
#[allow(clippy::too_many_arguments)]
fn begin_intro(
    mut commands: Commands,
    mut campaign: ResMut<Campaign>,
    start: Option<Res<crate::general::systems::map_systems::StartLevel>>,
    no_intro: Option<Res<NoIntro>>,
    mix: Option<ResMut<rusty_music::mix::Mix>>,
) {
    let Some(spec) = campaign.file.intro.clone() else {
        return;
    };
    if campaign.current != 0 || campaign.intro_seen || start.is_some() || no_intro.is_some() {
        return;
    }
    campaign.intro_seen = true;
    info!("intro: the copter is on its way");
    if let Some(mut mix) = mix {
        mix.duck_time = 0.6;
        mix.duck_db = DUCK_DB;
    }
    for top in [true, false] {
        let mut node = Node {
            position_type: PositionType::Absolute,
            left: Val::Px(0.0),
            width: Val::Percent(100.0),
            height: Val::Percent(LETTERBOX),
            ..default()
        };
        if top {
            node.top = Val::Px(0.0);
        } else {
            node.bottom = Val::Px(0.0);
        }
        commands.spawn((
            Name::new("Letterbox"),
            Letterbox,
            node,
            BackgroundColor(Color::BLACK),
            GlobalZIndex(60),
        ));
    }
    commands.insert_resource(Intro {
        spec,
        stage: Stage::Waiting,
        t: 0.0,
        waited: 0.0,
        plan: None,
        held: Vec::new(),
        level: None,
        landings: Vec::new(),
        copter: None,
        clouds: None,
        music: None,
        unducked: false,
        background_scale: 1.0,
    });
}

fn end_intro(
    mut commands: Commands,
    intro: Option<Res<Intro>>,
    mix: Option<ResMut<rusty_music::mix::Mix>>,
) {
    if intro.is_none() {
        return;
    }
    commands.remove_resource::<Intro>();
    if let Some(mut mix) = mix {
        mix.duck_db = 0.0;
    }
}

/// The level's middle and reach, from the map being loaded.
fn learn_level(
    mut loads: MessageReader<LoadMap>,
    tiles: Option<Res<TileDefinitions>>,
    mut intro: ResMut<Intro>,
) {
    let width = tiles.map_or(1.0, |t| t.tile_width);
    for LoadMap { map } in loads.read() {
        let rows = map.tiles.len() as f32;
        let cols = map.tiles.iter().map(Vec::len).max().unwrap_or(0) as f32;
        let centre = Vec3::new(cols * 0.5 * width, 0.0, rows * 0.5 * width);
        intro.level = Some((centre, cols.max(rows) * 0.5 * width));
    }
}

/// Keep the map's players back for the stand-ins to take their place.
fn hold_spawns(mut spawns: MessageReader<SpawnPlayer>, mut intro: ResMut<Intro>) {
    intro.held.extend(spawns.read().cloned());
}

/// Once the level and its players' start are known (and the clouds, or long enough
/// without them): plan the flight, spawn the copter, the clouds, the ropes and the
/// stand-ins, and start the music.
#[allow(clippy::too_many_arguments)]
fn take_off(
    mut commands: Commands,
    mut intro: ResMut<Intro>,
    time: Res<Time>,
    ground: Option<Res<GroundHeight>>,
    tiles: Option<Res<TileDefinitions>>,
    noise: Res<clouds::CloudNoise>,
    preset: Res<LightingPreset>,
    roster: Option<Res<PlayerRoster>>,
    game_assets: Res<GameAssets>,
    model_settings: Res<ModelSettings>,
    asset_server: Res<AssetServer>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut cloud_materials: ResMut<Assets<clouds::CloudMaterial>>,
    start_at: Option<Res<IntroStartAt>>,
) {
    if intro.stage != Stage::Waiting {
        return;
    }
    intro.waited += time.delta_secs();
    let (Some((centre, radius)), false) = (intro.level, intro.held.is_empty()) else {
        if intro.waited > WAIT_FOR_LEVEL {
            warn!("intro: no players' start on this level; skipping the intro");
            intro.stage = Stage::Done;
            for spawn in std::mem::take(&mut intro.held) {
                commands.write_message(spawn);
            }
        }
        return;
    };
    if noise.image.is_none() && intro.waited < WAIT_FOR_CLOUDS {
        return;
    }

    let floor = tiles.map_or(0.0, |t| t.floor_level);
    let height = |x: f32, z: f32| ground.as_ref().map_or(0.0, |g| g.height(x, z)) + floor;
    // One stand-in per player; they take the map's starts in turn, as `spawn_players`
    // hands them out.
    let count = roster.as_ref().map_or(1, |r| r.def_paths.len().max(1));
    let starts: Vec<Vec3> = intro
        .held
        .iter()
        .map(|s| s.position.with_y(height(s.position.x, s.position.z) + STANDING))
        .collect();
    let landings: Vec<Vec3> = (0..count)
        .filter_map(|i| starts.get(i % starts.len().max(1)).copied())
        .collect();
    let focus = landings.iter().copied().sum::<Vec3>() / landings.len().max(1) as f32;
    // The players are spawned standing where they land.
    for (spawn, at) in intro.held.iter_mut().zip(&starts) {
        spawn.position = *at;
    }

    let spec = intro.spec.clone();
    let plan = FlightPlan::new(focus, centre, radius, spec.timing, spec.clouds, height);

    // The plane hiding the void round the level only reaches a few level widths out:
    // from the air its edges would show. Stretch it under the whole flight.
    let reach = plan.copter_position(0.0).with_y(0.0).distance(centre.with_y(0.0));
    // `map_systems` makes it four level widths across, at least 200 m.
    intro.background_scale = (reach + 2500.0) * 2.0 / (radius * 8.0).max(200.0);

    intro.copter = Some(copter::spawn_copter(
        &mut commands,
        &mut meshes,
        &mut materials,
        &asset_server,
        &spec.copter,
        Transform::from_translation(plan.copter_position(0.0)).with_rotation(plan.copter_rotation(0.0)),
    ));

    if let Some(image) = noise.image.clone() {
        // Wide enough to fill the view to the horizon from over the clouds.
        let middle = plan.copter_position(plan.timing.dive * 0.5);
        let reach = Vec3::new(1400.0, 0.0, 1400.0);
        let min = (middle - reach).with_y(plan.clouds_base);
        let max = (middle + reach).with_y(plan.clouds_top);
        let towards_light = Vec3::new(-0.4, 0.55, -0.3);
        let params = clouds::cloud_params(min, max, towards_light, *preset);
        intro.clouds = Some(clouds::spawn_clouds(
            &mut commands,
            &mut meshes,
            &mut cloud_materials,
            image,
            params,
        ));
    } else {
        warn!("intro: the cloud noise is not ready; flying without clouds");
    }

    let rope_mesh = meshes.add(Cylinder::new(0.025, 1.0));
    let rope_material = materials.add(StandardMaterial {
        base_color: Color::srgb(0.55, 0.5, 0.38),
        perceptual_roughness: 0.9,
        ..default()
    });
    let s = &*model_settings;
    for index in 0..landings.len() {
        let def = roster
            .as_ref()
            .and_then(|r| r.def_paths.get(index))
            .and_then(|path| AssetDefinition::load_from_def_path(path));
        let (scene, gltf) = def.map_or_else(
            || (game_assets.player_scene.clone(), game_assets.player_gltf.clone()),
            |def| {
                (
                    asset_server.load(bevy::gltf::GltfAssetLabel::Scene(0).from_asset(def.model_path.clone())),
                    asset_server.load(def.model_path),
                )
            },
        );
        commands.spawn((
            Name::new(format!("Rappeller {index}")),
            Rappeller {
                index,
                count,
                gltf,
            },
            WorldAssetRoot(scene),
            FixSceneTransform::new(
                Vec3::new(s.translation_x, s.translation_y, s.translation_z),
                Quat::from_rotation_y(s.rotation_y_degrees.to_radians()),
                Vec3::splat(s.scale),
            ),
            Transform::from_translation(plan.hatch(index, 0.0))
                .looking_to(-plan.heading, Vec3::Y),
        ));
        commands.spawn((
            Name::new(format!("Rope {index}")),
            Rope { index, count },
            Mesh3d(rope_mesh.clone()),
            MeshMaterial3d(rope_material.clone()),
            Transform::default(),
            Visibility::Hidden,
        ));
    }
    intro.landings = landings.into_iter().map(|l| (l, None)).collect();

    if !spec.music.is_empty() {
        let track = asset_server.load::<AudioSample>(spec.music.clone());
        intro.music = Some(
            commands
                .spawn((
                    Name::new("Intro music"),
                    IntroMusic,
                    SamplePlayer::new(track).with_volume(Volume::Decibels(spec.music_db)),
                ))
                .id(),
        );
    }

    intro.plan = Some(plan);
    intro.stage = Stage::Flying;
    intro.t = start_at.map_or(0.0, |s| s.0.max(0.0));
}

/// Advance the intro: the copter, the stand-ins and the ropes; the players handed over
/// when the last one is down; the camera handed back at the end; the copter gone after.
#[allow(clippy::too_many_arguments)]
fn run_intro(
    mut commands: Commands,
    mut intro: ResMut<Intro>,
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut transforms: Query<&mut Transform, (Without<GameCamera>, Without<Rappeller>, Without<Rope>)>,
    rappellers: Query<(Entity, &Rappeller, &mut Transform), (Without<Rope>, Without<GameCamera>)>,
    mut ropes: Query<
        (Entity, &Rope, &mut Transform, &mut Visibility),
        (Without<Rappeller>, Without<GameCamera>),
    >,
    mut spawns: MessageWriter<SpawnPlayer>,
    mut focus: ResMut<CameraFocus>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<GameCamera>>,
    settings: Res<GameSettings>,
) {
    if matches!(intro.stage, Stage::Waiting) {
        return;
    }
    let Some(plan) = intro.plan.clone() else {
        return;
    };
    intro.t += time.delta_secs();

    // Skip to the hand-off.
    let skip = keys.any_just_pressed([KeyCode::Escape, KeyCode::Enter, KeyCode::Space])
        || gamepads.iter().any(|pad| {
            pad.any_just_pressed([GamepadButton::Start, GamepadButton::South])
        });
    if skip && intro.stage == Stage::Flying && intro.t > SKIP_GRACE {
        info!("intro: skipped");
        intro.t = intro.t.max(plan.timing.landed_at());
        if let Some(music) = intro.music.take() {
            commands.entity(music).try_despawn();
        }
    }
    let t = intro.t;

    if let Some(mut at) = intro.copter.and_then(|c| transforms.get_mut(c).ok()) {
        at.translation = plan.copter_position(t);
        at.rotation = plan.copter_rotation(t);
    }

    for (entity, rappeller, mut at) in rappellers {
        let Some(&(ground, _)) = intro.landings.get(rappeller.index) else {
            continue;
        };
        let (pos, down) = plan.rappeller_at(rappeller.index, rappeller.count, t, ground);
        at.translation = pos;
        if down {
            if let Some(landing) = intro.landings.get_mut(rappeller.index) {
                landing.1.get_or_insert(t);
            }
        }
        // Gone once the real players are in.
        if intro.stage != Stage::Flying {
            commands.entity(entity).try_despawn();
        }
    }
    for (entity, rope, mut at, mut visible) in &mut ropes {
        let Some(&(ground, down_at)) = intro.landings.get(rope.index) else {
            continue;
        };
        let (held, _) = plan.rappeller_at(rope.index, rope.count, t, ground);
        let (top, bottom) = copter::rope_ends(&plan, rope.index, t, held, down_at);
        *at = copter::rope_transform(top, bottom);
        *visible = if t >= plan.timing.hover_at() - 0.3 && top.distance(bottom) > 0.05 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if down_at.is_some() && top.distance(bottom) <= 0.05 {
            commands.entity(entity).try_despawn();
        }
    }

    if intro.stage == Stage::Flying && t >= plan.timing.landed_at() {
        info!("intro: the team is down");
        let held = std::mem::take(&mut intro.held);
        spawns.write_batch(held);
        intro.stage = Stage::HandingOver;
    }

    if intro.stage == Stage::HandingOver && t >= plan.timing.end() {
        info!("intro: over to the game camera");
        intro.stage = Stage::Done;
        if let Some(clouds) = intro.clouds.take() {
            commands.entity(clouds).try_despawn();
        }
        for (mut projection, mut at) in &mut cameras {
            *projection = crate::camera::systems::projection_for(&settings);
            let offset = crate::camera::systems::camera_offset(&settings);
            at.translation = plan.landing + offset;
            at.look_at(plan.landing, Vec3::Y);
        }
        // `camera_follow` snaps to the players rather than easing in from the old focus.
        *focus = CameraFocus::default();
    }

    if intro.stage == Stage::Done && t > plan.timing.end() + COPTER_GONE {
        if let Some(copter) = intro.copter.take() {
            commands.entity(copter).try_despawn();
        }
    }
}

/// The level does not start (no `LevelStarted`, so no crawl, waves or spawners) while the
/// players are held back; once they are in, the stories look again.
fn hold_stories(mut store: ResMut<turbofacts::story::StoryStore>, intro: Res<Intro>) {
    let hold = matches!(intro.stage, Stage::Waiting | Stage::Flying);
    if hold && store.active {
        store.active = false;
    } else if !hold && !store.active {
        store.active = true;
        store.needs_checking = true;
    }
}

/// The game's own view of the start, for the hand-off to settle into.
fn iso_view(settings: &GameSettings) -> IsoView {
    IsoView {
        dir: crate::camera::systems::camera_offset(settings).normalize_or(Vec3::Y),
        height: settings.ortho_viewport_height * settings.zoom,
    }
}

/// Fly the camera along the plan's shots, in perspective.
fn drive_camera(
    intro: Res<Intro>,
    settings: Res<GameSettings>,
    mut cameras: Query<(&mut Projection, &mut Transform), With<GameCamera>>,
) {
    let Some(plan) = &intro.plan else {
        return;
    };
    if intro.stage == Stage::Done {
        return;
    }
    let shot = plan.shot_at(intro.t, iso_view(&settings));
    for (mut projection, mut at) in &mut cameras {
        let fov = shot.fov_degrees.clamp(1.0, 120.0).to_radians();
        if let Projection::Perspective(p) = &mut *projection {
            p.fov = fov;
        } else {
            *projection = Projection::Perspective(PerspectiveProjection {
                fov,
                near: 0.2,
                far: 4000.0,
                ..default()
            });
        }
        *at = Transform::from_translation(shot.eye).looking_at(shot.target, Vec3::Y);
    }
}

/// From high up the game's depth haze would swallow the town: push it out to the
/// distance, and bring it back to the game's own over the hand-off.
fn intro_fog(
    intro: Res<Intro>,
    level: Res<crate::camera::fog::FogLevel>,
    preset: Res<LightingPreset>,
    settings: Res<GameSettings>,
    mut scene: ResMut<crate::camera::fog::SceneFog>,
) {
    let Some(plan) = &intro.plan else {
        return;
    };
    if intro.stage == Stage::Done {
        return;
    }
    let shot = plan.shot_at(intro.t, iso_view(&settings));
    let distance = shot.eye.distance(shot.target);
    let handed = ((intro.t - plan.timing.landed_at()) / plan.timing.handoff).clamp(0.0, 1.0);
    scene.0 = crate::camera::fog::aerial_fog(*level, *preset, distance, handed);
}

/// Bars in, and out over the hand-off.
fn letterbox(
    mut commands: Commands,
    intro: Res<Intro>,
    mut bars: Query<(Entity, &mut Node), With<Letterbox>>,
) {
    let out = intro.plan.as_ref().map_or(0.0, |plan| {
        ((intro.t - plan.timing.landed_at()) / plan.timing.handoff).clamp(0.0, 1.0)
    });
    for (entity, mut node) in &mut bars {
        if intro.stage == Stage::Done {
            commands.entity(entity).try_despawn();
        } else {
            node.height = Val::Percent(LETTERBOX * (1.0 - out));
        }
    }
}

/// Keep the plane hiding the void stretched under the whole flight (the map loader may
/// spawn it after the take-off).
fn stretch_background(
    intro: Res<Intro>,
    mut backgrounds: Query<
        &mut Transform,
        With<crate::general::systems::map_systems::BackgroundFloor>,
    >,
) {
    if intro.background_scale <= 1.0 {
        return;
    }
    for mut background in &mut backgrounds {
        if background.scale.x < intro.background_scale {
            background.scale = Vec3::splat(intro.background_scale);
        }
    }
}

/// The generative music comes back once play is on and the intro's track has ended.
fn unduck_when_done(
    mut intro: ResMut<Intro>,
    playing: Query<(), With<IntroMusic>>,
    mix: Option<ResMut<rusty_music::mix::Mix>>,
) {
    if intro.unducked || intro.stage != Stage::Done || !playing.is_empty() {
        return;
    }
    intro.unducked = true;
    if let Some(mut mix) = mix {
        mix.duck_time = 3.0;
        mix.duck_db = 0.0;
    }
}
