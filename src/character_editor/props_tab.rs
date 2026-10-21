//! The Props tab: the model's type and the stats that come with it.
//!
//! - **Type**: what the def is (character, weapon, enemy, ...). Changing it resets the
//!   stats to that type's defaults, so it asks first. The new type takes effect when the
//!   def is saved and opened again from the Library — a character doc turned into a
//!   weapon cannot keep being worn in the meantime (`doc::sync_doc_to_live` leaves it).
//! - **Stats**: every plain field of the type's props — numbers with coarse/fine nudges,
//!   switches, and pick-one choices. Nested data (resistances, explosions, projectiles,
//!   starting ammo, extra weapons) is listed but edited in the `.ron` for now.
//!
//! The asset browser only displayed these; here they are editable. The table of fields
//! is [`fields`], and every edit goes through [`set_number`] / [`set_toggle`] /
//! [`set_choice`], so the mapping between the screen and the def is one place, and tested.

use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::assets::asset_definition::{
    AmmoKind, EnemyAttack, ItemKind, ModelType, PlayerAbility, TowerKind, WeaponHands,
};
use crate::assets::gizmos::{bone_map, joints_under};
use crate::character_editor::aim_bones::spawn_aim_bones;
use crate::character_editor::doc::{
    EditorDoc, EditorFocus, PropDoc, SideDocs, WeaponDoc, subject_doc,
};
use crate::character_editor::models::DefKind;
use crate::character_editor::ui::row;
use crate::player::components::Player;
use bevy::mesh::skinning::SkinnedMesh;

/// The tab's contents.
#[derive(Component)]
pub struct PropsContainer;

/// The live character's bone names, for the torso-twist picker.
#[derive(bevy::ecs::system::SystemParam)]
pub struct CharacterRig<'w, 's> {
    players: Query<'w, 's, Entity, With<Player>>,
    children: Query<'w, 's, &'static Children>,
    skinned: Query<'w, 's, &'static SkinnedMesh>,
    names: Query<'w, 's, &'static Name>,
    new_skins: Query<'w, 's, (), Added<SkinnedMesh>>,
}

impl CharacterRig<'_, '_> {
    /// Sorted bone names of the character in the arena; empty before its skeleton spawns.
    pub fn bones(&self) -> Vec<String> {
        self.players.iter().next().map_or_default(|player| {
            let joints = joints_under(player, &self.children, &self.skinned);
            let mut names: Vec<String> = bone_map(&joints, &self.names).into_keys().collect();
            names.sort();
            names
        })
    }

    /// A skeleton just spawned, so the bone list has something new to show.
    pub fn respawned(&self) -> bool {
        !self.new_skins.is_empty()
    }
}

/// A type change waiting for confirmation.
#[derive(Resource, Debug, Default)]
pub struct PropsTab {
    pub pending_type: Option<DefKind>,
}

/// What one row of the stats shows, and how it is edited.
#[derive(Debug, Clone, PartialEq)]
pub enum FieldValue {
    Number {
        value: f32,
        coarse: f32,
        fine: f32,
        /// Stored as an integer: nudges round.
        int: bool,
    },
    Toggle(bool),
    Choice {
        value: &'static str,
        options: &'static [&'static str],
    },
    /// Shown, not editable here.
    Info(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub id: &'static str,
    pub label: &'static str,
    pub value: FieldValue,
}

const fn num(id: &'static str, label: &'static str, value: f32, coarse: f32, fine: f32) -> Field {
    Field {
        id,
        label,
        value: FieldValue::Number {
            value,
            coarse,
            fine,
            int: false,
        },
    }
}

#[allow(clippy::cast_precision_loss)]
const fn int(id: &'static str, label: &'static str, value: f32, coarse: f32) -> Field {
    Field {
        id,
        label,
        value: FieldValue::Number {
            value,
            coarse,
            fine: 1.0,
            int: true,
        },
    }
}

const fn toggle(id: &'static str, label: &'static str, value: bool) -> Field {
    Field {
        id,
        label,
        value: FieldValue::Toggle(value),
    }
}

const fn choice(
    id: &'static str,
    label: &'static str,
    value: &'static str,
    options: &'static [&'static str],
) -> Field {
    Field {
        id,
        label,
        value: FieldValue::Choice { value, options },
    }
}

fn info(id: &'static str, label: &'static str, value: String) -> Field {
    Field {
        id,
        label,
        value: FieldValue::Info(value),
    }
}

const ABILITIES: &[&str] = &[
    "Bombardment",
    "Healing",
    "Whirlwind",
    "GoldDigger",
    "Molotov",
];
const HANDS: &[&str] = &["OneHanded", "TwoHanded"];
const AMMO: &[&str] = &[
    "Infinite", "Pistol", "Rifle", "Shells", "Grenade", "Molotov",
];
const ATTACKS: &[&str] = &["Melee", "Ranged"];
const TOWER_KINDS: &[&str] = &["Shooter", "Slow", "Area"];
const ITEM_KINDS: &[&str] = &[
    "Decorative",
    "HealthPickup",
    "AmmoPickup",
    "WeaponPickup",
    "Coins",
    "Key",
];

const fn ability_name(ability: &PlayerAbility) -> &'static str {
    match ability {
        PlayerAbility::Bombardment => "Bombardment",
        PlayerAbility::Healing => "Healing",
        PlayerAbility::Whirlwind => "Whirlwind",
        PlayerAbility::GoldDigger => "GoldDigger",
        PlayerAbility::Molotov => "Molotov",
    }
}

const fn ammo_name(ammo: AmmoKind) -> &'static str {
    match ammo {
        AmmoKind::Infinite => "Infinite",
        AmmoKind::Pistol => "Pistol",
        AmmoKind::Rifle => "Rifle",
        AmmoKind::Shells => "Shells",
        AmmoKind::Grenade => "Grenade",
        AmmoKind::Molotov => "Molotov",
    }
}

fn ammo_from(name: &str) -> AmmoKind {
    match name {
        "Pistol" => AmmoKind::Pistol,
        "Rifle" => AmmoKind::Rifle,
        "Shells" => AmmoKind::Shells,
        "Grenade" => AmmoKind::Grenade,
        "Molotov" => AmmoKind::Molotov,
        _ => AmmoKind::Infinite,
    }
}

/// A one-line summary of nested data, for the fields edited in the `.ron`.
fn summary<T: std::fmt::Debug>(value: &T) -> String {
    let text = format!("{value:?}");
    if text.len() > 60 {
        format!("{}...", text.get(..57).unwrap_or_default())
    } else {
        text
    }
}

/// The editable fields of a model type, in display order.
#[allow(clippy::too_many_lines, clippy::cast_precision_loss)]
pub fn fields(model_type: &ModelType) -> Vec<Field> {
    match model_type {
        ModelType::Player(p) => vec![
            choice("ability", "ability", ability_name(&p.ability), ABILITIES),
            num(
                "throw_rate",
                "throws / min",
                p.throw_rate_per_minute,
                10.0,
                1.0,
            ),
            info(
                "weapon",
                "weapon",
                p.weapon.clone().unwrap_or_else(|| "(none)".into()),
            ),
            info("extra_weapons", "extra weapons", summary(&p.extra_weapons)),
            info("starting_ammo", "starting ammo", summary(&p.starting_ammo)),
        ],
        ModelType::Weapon(w) => vec![
            choice(
                "hands",
                "hands",
                match w.hands {
                    WeaponHands::OneHanded => "OneHanded",
                    WeaponHands::TwoHanded => "TwoHanded",
                },
                HANDS,
            ),
            int("damage", "damage", w.damage as f32, 5.0),
            num(
                "fire_rate",
                "shots / min",
                w.fire_rate_per_minute,
                30.0,
                5.0,
            ),
            num("range", "range", w.range, 5.0, 1.0),
            num("spread", "spread (deg)", w.spread_deg, 0.5, 0.1),
            int("pellets", "pellets", w.pellets as f32, 1.0),
            toggle("auto", "automatic", w.auto),
            choice("ammo", "ammo", ammo_name(w.ammo), AMMO),
            int("magazine", "magazine", w.magazine as f32, 5.0),
            num("reload", "reload (s)", w.reload_secs, 0.25, 0.05),
            info("projectile", "projectile", summary(&w.projectile)),
        ],
        ModelType::Enemy(e) => {
            let mut fields = vec![
                num("health", "health", e.health, 10.0, 1.0),
                num("speed", "speed", e.speed, 0.5, 0.1),
                int("coins", "coin drop", e.coin_drop as f32, 5.0),
                num("touch_dps", "touch damage / s", e.touch_dps, 5.0, 1.0),
                choice(
                    "attack",
                    "attack",
                    match e.attack {
                        EnemyAttack::Melee => "Melee",
                        EnemyAttack::Ranged { .. } => "Ranged",
                    },
                    ATTACKS,
                ),
            ];
            if let EnemyAttack::Ranged {
                damage,
                range,
                fire_rate_per_minute,
            } = e.attack
            {
                fields.push(int("ranged_damage", "  shot damage", damage as f32, 5.0));
                fields.push(num("ranged_range", "  shot range", range, 1.0, 0.1));
                fields.push(num(
                    "ranged_rate",
                    "  shots / min",
                    fire_rate_per_minute,
                    10.0,
                    1.0,
                ));
            }
            fields.push(info("loot", "loot table", summary(&e.loot_table)));
            fields.push(info("resistances", "resistances", summary(&e.resistances)));
            fields.push(info(
                "explodes",
                "explodes on death",
                summary(&e.explodes_on_death),
            ));
            fields
        }
        ModelType::Tower(t) => {
            let mut fields = vec![
                num("health", "health", t.health, 10.0, 1.0),
                int("cost", "cost", t.cost as f32, 10.0),
                num("range", "range", t.range, 0.5, 0.1),
                num("damage", "damage", t.damage, 5.0, 1.0),
                num("fire_rate", "shots / min", t.fire_rate_per_minute, 5.0, 1.0),
                choice(
                    "kind",
                    "kind",
                    match t.kind {
                        TowerKind::Shooter => "Shooter",
                        TowerKind::Slow { .. } => "Slow",
                        TowerKind::Area { .. } => "Area",
                    },
                    TOWER_KINDS,
                ),
            ];
            match t.kind {
                TowerKind::Slow { factor } => {
                    fields.push(num("slow_factor", "  slow factor", factor, 0.1, 0.01));
                }
                TowerKind::Area { tick_hz } => {
                    fields.push(num("tick_hz", "  ticks / s", tick_hz, 0.5, 0.1));
                }
                TowerKind::Shooter => {}
            }
            fields.push(info("description", "description", t.description.clone()));
            fields.push(info("resistances", "resistances", summary(&t.resistances)));
            fields
        }
        ModelType::Terrain(t) => {
            let mut fields = vec![
                toggle("blocks_enemies", "blocks enemies", t.blocks_enemies),
                toggle("blocks_players", "blocks players", t.blocks_players),
                toggle("breakable", "breakable", t.health.is_some()),
            ];
            if let Some(health) = t.health {
                fields.push(num("health", "  health", health, 10.0, 1.0));
            }
            fields.push(info("loot", "loot table", summary(&t.loot_table)));
            fields.push(info(
                "explodes",
                "explodes on death",
                summary(&t.explodes_on_death),
            ));
            fields
        }
        ModelType::Item(i) => {
            let kind = match i.kind {
                ItemKind::Decorative => "Decorative",
                ItemKind::HealthPickup { .. } => "HealthPickup",
                ItemKind::AmmoPickup { .. } => "AmmoPickup",
                ItemKind::WeaponPickup { .. } => "WeaponPickup",
                ItemKind::Coins { .. } => "Coins",
                ItemKind::Key { .. } => "Key",
            };
            let mut fields = vec![choice("kind", "kind", kind, ITEM_KINDS)];
            match &i.kind {
                ItemKind::HealthPickup { amount } => {
                    fields.push(num("amount", "  heals", *amount, 10.0, 1.0));
                }
                ItemKind::AmmoPickup { kind, rounds } => {
                    fields.push(choice("ammo", "  ammo", ammo_name(*kind), AMMO));
                    fields.push(int("rounds", "  rounds", *rounds as f32, 5.0));
                }
                ItemKind::Coins { value } => {
                    fields.push(int("value", "  value", *value as f32, 5.0))
                }
                ItemKind::WeaponPickup { def } => {
                    fields.push(info("def", "  weapon def", def.clone()))
                }
                ItemKind::Key { id } => fields.push(info("id", "  key id", id.clone())),
                ItemKind::Decorative => {}
            }
            fields
        }
    }
}

/// Nudged numbers never go negative: no stat here means anything below zero.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn set_number(model_type: &mut ModelType, id: &str, value: f32) {
    let value = value.max(0.0);
    let whole = value.round();
    match model_type {
        ModelType::Player(p) => {
            if id == "throw_rate" {
                p.throw_rate_per_minute = value;
            }
        }
        ModelType::Weapon(w) => match id {
            "damage" => w.damage = whole as i32,
            "fire_rate" => w.fire_rate_per_minute = value,
            "range" => w.range = value,
            "spread" => w.spread_deg = value,
            "pellets" => w.pellets = (whole as u32).max(1),
            "magazine" => w.magazine = (whole as u32).max(1),
            "reload" => w.reload_secs = value,
            _ => {}
        },
        ModelType::Enemy(e) => match id {
            "health" => e.health = value,
            "speed" => e.speed = value,
            "coins" => e.coin_drop = whole as u32,
            "touch_dps" => e.touch_dps = value,
            _ => {
                if let EnemyAttack::Ranged {
                    damage,
                    range,
                    fire_rate_per_minute,
                } = &mut e.attack
                {
                    match id {
                        "ranged_damage" => *damage = whole as i32,
                        "ranged_range" => *range = value,
                        "ranged_rate" => *fire_rate_per_minute = value,
                        _ => {}
                    }
                }
            }
        },
        ModelType::Tower(t) => match id {
            "health" => t.health = value,
            "cost" => t.cost = whole as u32,
            "range" => t.range = value,
            "damage" => t.damage = value,
            "fire_rate" => t.fire_rate_per_minute = value,
            "slow_factor" => {
                if let TowerKind::Slow { factor } = &mut t.kind {
                    *factor = value.min(1.0);
                }
            }
            "tick_hz" => {
                if let TowerKind::Area { tick_hz } = &mut t.kind {
                    *tick_hz = value;
                }
            }
            _ => {}
        },
        ModelType::Terrain(t) => {
            if id == "health" && t.health.is_some() {
                t.health = Some(value);
            }
        }
        ModelType::Item(i) => match (&mut i.kind, id) {
            (ItemKind::HealthPickup { amount }, "amount") => *amount = value,
            (ItemKind::AmmoPickup { rounds, .. }, "rounds") => *rounds = whole as u32,
            (ItemKind::Coins { value: coins }, "value") => *coins = whole as u32,
            _ => {}
        },
    }
}

pub fn set_toggle(model_type: &mut ModelType, id: &str, on: bool) {
    match model_type {
        ModelType::Weapon(w) if id == "auto" => w.auto = on,
        ModelType::Terrain(t) => match id {
            "blocks_enemies" => t.blocks_enemies = on,
            "blocks_players" => t.blocks_players = on,
            "breakable" => t.health = on.then_some(t.health.unwrap_or(50.0)),
            _ => {}
        },
        _ => {}
    }
}

/// Pick an option. Switching to a variant with data of its own starts it at sensible
/// values rather than zeros.
pub fn set_choice(model_type: &mut ModelType, id: &str, option: &str) {
    match model_type {
        ModelType::Player(p) if id == "ability" => {
            p.ability = match option {
                "Healing" => PlayerAbility::Healing,
                "Whirlwind" => PlayerAbility::Whirlwind,
                "GoldDigger" => PlayerAbility::GoldDigger,
                "Molotov" => PlayerAbility::Molotov,
                _ => PlayerAbility::Bombardment,
            };
        }
        ModelType::Weapon(w) => match id {
            "hands" => {
                w.hands = if option == "TwoHanded" {
                    WeaponHands::TwoHanded
                } else {
                    WeaponHands::OneHanded
                };
            }
            "ammo" => w.ammo = ammo_from(option),
            _ => {}
        },
        ModelType::Enemy(e) if id == "attack" => {
            let ranged = matches!(e.attack, EnemyAttack::Ranged { .. });
            match option {
                "Ranged" if !ranged => {
                    e.attack = EnemyAttack::Ranged {
                        damage: 10,
                        range: 8.0,
                        fire_rate_per_minute: 30.0,
                    };
                }
                "Melee" => e.attack = EnemyAttack::Melee,
                _ => {}
            }
        }
        ModelType::Tower(t) if id == "kind" => {
            t.kind = match (option, &t.kind) {
                ("Slow", TowerKind::Slow { factor }) => TowerKind::Slow { factor: *factor },
                ("Slow", _) => TowerKind::Slow { factor: 0.5 },
                ("Area", TowerKind::Area { tick_hz }) => TowerKind::Area { tick_hz: *tick_hz },
                ("Area", _) => TowerKind::Area { tick_hz: 2.0 },
                _ => TowerKind::Shooter,
            };
        }
        ModelType::Item(i) => match id {
            "kind" => {
                let current =
                    fields(&ModelType::Item(i.clone()))
                        .first()
                        .and_then(|f| match f.value {
                            FieldValue::Choice { value, .. } => Some(value),
                            _ => None,
                        });
                if current == Some(option) {
                    return;
                }
                i.kind = match option {
                    "HealthPickup" => ItemKind::HealthPickup { amount: 25.0 },
                    "AmmoPickup" => ItemKind::AmmoPickup {
                        kind: AmmoKind::Pistol,
                        rounds: 30,
                    },
                    "WeaponPickup" => ItemKind::WeaponPickup { def: String::new() },
                    "Coins" => ItemKind::Coins { value: 10 },
                    "Key" => ItemKind::Key { id: String::new() },
                    _ => ItemKind::Decorative,
                };
            }
            "ammo" => {
                if let ItemKind::AmmoPickup { kind, .. } = &mut i.kind {
                    *kind = ammo_from(option);
                }
            }
            _ => {}
        },
        _ => {}
    }
}

const LABEL: Color = Color::srgb(0.8, 0.85, 0.9);
const VALUE: Color = Color::srgb(0.6, 0.85, 1.0);
const BUTTON: Color = Color::srgb(0.85, 0.92, 0.95);
const NOTE: Color = Color::srgb(0.7, 0.65, 0.5);

fn line<'a>(parent: &'a mut ChildSpawnerCommands) -> EntityCommands<'a> {
    parent.spawn(Node {
        display: Display::Flex,
        flex_direction: FlexDirection::Row,
        flex_wrap: FlexWrap::Wrap,
        align_items: AlignItems::Center,
        column_gap: Val::Px(4.0),
        row_gap: Val::Px(2.0),
        ..Default::default()
    })
}

fn label(parent: &mut ChildSpawnerCommands, text: &str, width: f32, color: Color) {
    parent.spawn((
        Text::new(text),
        TextFont::default().with_font_size(11.0),
        TextColor(color),
        Node {
            min_width: Val::Px(width),
            ..Default::default()
        },
    ));
}

fn heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::default().with_font_size(12.0),
        TextColor(Color::srgb(0.55, 0.8, 0.65)),
        Node {
            margin: UiRect::top(Val::Px(6.0)),
            ..Default::default()
        },
    ));
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn rebuild_props_tab(
    doc: Res<EditorDoc>,
    weapon_doc: Res<WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    tab: Res<PropsTab>,
    mut commands: Commands,
    container_q: Query<Entity, With<PropsContainer>>,
    mut built: Local<Option<Entity>>,
    rig: CharacterRig,
) {
    let Ok(container) = container_q.single() else {
        return;
    };
    let changed = doc.is_changed()
        || weapon_doc.is_changed()
        || prop_doc.is_changed()
        || focus.is_changed()
        || tab.is_changed()
        || rig.respawned();
    if !changed && *built == Some(container) {
        return;
    }
    *built = Some(container);
    commands.entity(container).despawn_related::<Children>();

    let open = subject_doc(focus.subject, &doc, &weapon_doc, &prop_doc);
    let Some(def) = open.def.as_ref() else {
        commands.entity(container).with_children(|parent| {
            label(parent, "nothing open", 0.0, NOTE);
        });
        return;
    };
    let current = DefKind::of(&def.model_type);
    let saved_kind = open.saved.as_ref().map(|d| DefKind::of(&d.model_type));
    let field_list = fields(&def.model_type);
    let pending = tab.pending_type;
    let is_character = matches!(def.model_type, ModelType::Player(_));
    let bones = if is_character {
        rig.bones()
    } else {
        Vec::new()
    };
    let def_for_twist = def.clone();

    commands.entity(container).with_children(|parent| {
        heading(parent, "TYPE");
        line(parent).with_children(|strip| {
            for kind in DefKind::ALL {
                row(strip, kind.label().to_string(), kind == current, BUTTON).observe(
                    move |_: On<Activate>, mut tab: ResMut<PropsTab>| {
                        tab.pending_type = (kind != current).then_some(kind);
                    },
                );
            }
        });
        if let Some(kind) = pending {
            label(
                parent,
                &format!(
                    "change to {}? its stats reset to the defaults",
                    kind.label()
                ),
                0.0,
                Color::srgb(1.0, 0.8, 0.45),
            );
            line(parent).with_children(|strip| {
                row(strip, "Change".to_string(), true, BUTTON).observe(
                    move |_: On<Activate>,
                          mut tab: ResMut<PropsTab>,
                          focus: Res<EditorFocus>,
                          mut docs: SideDocs| {
                        if let Some(def) = docs.subject_mut(focus.subject) {
                            def.model_type = kind.model_type();
                        }
                        tab.pending_type = None;
                    },
                );
                row(strip, "Cancel".to_string(), false, BUTTON).observe(
                    |_: On<Activate>, mut tab: ResMut<PropsTab>| {
                        tab.pending_type = None;
                    },
                );
            });
        }
        if saved_kind.is_some_and(|saved| saved != current) {
            label(
                parent,
                "the new type takes effect once saved and opened again from the Library",
                0.0,
                NOTE,
            );
        }

        heading(parent, "STATS");
        for field in field_list {
            spawn_field(parent, field);
        }

        if is_character {
            spawn_aim_bones(parent, &def_for_twist, &bones);
        }
    });
}

fn spawn_field(parent: &mut ChildSpawnerCommands, field: Field) {
    let id = field.id;
    match field.value {
        FieldValue::Number {
            value,
            coarse,
            fine,
            int,
        } => {
            line(parent).with_children(|strip| {
                label(strip, field.label, 120.0, LABEL);
                let shown = if int {
                    format!("{value:.0}")
                } else {
                    format!("{value:.2}")
                };
                label(strip, &shown, 56.0, VALUE);
                for (text, delta) in [("<<", -coarse), ("<", -fine), (">", fine), (">>", coarse)] {
                    row(strip, text.to_string(), false, BUTTON).observe(
                        move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                            if let Some(def) = docs.subject_mut(focus.subject) {
                                set_number(&mut def.model_type, id, value + delta);
                            }
                        },
                    );
                }
            });
        }
        FieldValue::Toggle(on) => {
            row(
                parent,
                format!("[{}] {}", if on { "x" } else { " " }, field.label),
                on,
                BUTTON,
            )
            .observe(
                move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                    if let Some(def) = docs.subject_mut(focus.subject) {
                        set_toggle(&mut def.model_type, id, !on);
                    }
                },
            );
        }
        FieldValue::Choice { value, options } => {
            line(parent).with_children(|strip| {
                label(strip, field.label, 120.0, LABEL);
                for option in options {
                    row(strip, (*option).to_string(), *option == value, BUTTON).observe(
                        move |_: On<Activate>, focus: Res<EditorFocus>, mut docs: SideDocs| {
                            if let Some(def) = docs.subject_mut(focus.subject) {
                                set_choice(&mut def.model_type, id, option);
                            }
                        },
                    );
                }
            });
        }
        FieldValue::Info(value) => {
            line(parent).with_children(|strip| {
                label(strip, field.label, 120.0, LABEL);
                label(strip, &format!("{value}  (edit in the .ron)"), 0.0, NOTE);
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::{
        EnemyProps, ItemProps, PlayerProps, TerrainProps, TowerProps, WeaponProps,
    };

    fn number(fields: &[Field], id: &str) -> Option<f32> {
        fields
            .iter()
            .find(|f| f.id == id)
            .and_then(|f| match f.value {
                FieldValue::Number { value, .. } => Some(value),
                _ => None,
            })
    }

    /// Every editable field a type shows must be one its setters handle, or the buttons
    /// on screen would silently do nothing.
    #[test]
    fn every_number_shown_can_be_set() {
        let types = [
            ModelType::Player(PlayerProps::default()),
            ModelType::Weapon(WeaponProps::default()),
            ModelType::Enemy(EnemyProps {
                attack: EnemyAttack::Ranged {
                    damage: 1,
                    range: 1.0,
                    fire_rate_per_minute: 1.0,
                },
                ..Default::default()
            }),
            ModelType::Tower(TowerProps {
                kind: TowerKind::Slow { factor: 0.5 },
                ..Default::default()
            }),
            ModelType::Terrain(TerrainProps {
                health: Some(10.0),
                ..Default::default()
            }),
            ModelType::Item(ItemProps {
                kind: ItemKind::HealthPickup { amount: 5.0 },
            }),
        ];
        for model_type in types {
            for field in fields(&model_type) {
                if let FieldValue::Number { value, coarse, .. } = field.value {
                    let mut edited = model_type.clone();
                    set_number(&mut edited, field.id, value + coarse);
                    assert_ne!(
                        number(&fields(&edited), field.id),
                        Some(value),
                        "{} on {model_type:?} did not change",
                        field.id
                    );
                }
            }
        }
    }

    #[test]
    fn integers_round_and_nothing_goes_negative() {
        let mut weapon = ModelType::Weapon(WeaponProps::default());
        set_number(&mut weapon, "magazine", 7.6);
        assert_eq!(number(&fields(&weapon), "magazine"), Some(8.0));
        set_number(&mut weapon, "range", -5.0);
        assert_eq!(number(&fields(&weapon), "range"), Some(0.0));
        set_number(&mut weapon, "pellets", 0.0);
        assert_eq!(
            number(&fields(&weapon), "pellets"),
            Some(1.0),
            "a gun fires something"
        );
    }

    #[test]
    fn switching_an_enemy_to_ranged_gives_it_a_usable_attack() {
        let mut enemy = ModelType::Enemy(EnemyProps::default());
        set_choice(&mut enemy, "attack", "Ranged");
        assert!(number(&fields(&enemy), "ranged_range").is_some_and(|r| r > 0.0));
        set_choice(&mut enemy, "attack", "Melee");
        assert!(number(&fields(&enemy), "ranged_range").is_none());
    }

    #[test]
    fn breakable_terrain_gets_health_and_loses_it_again() {
        let mut terrain = ModelType::Terrain(TerrainProps::default());
        set_toggle(&mut terrain, "breakable", true);
        assert!(number(&fields(&terrain), "health").is_some());
        set_toggle(&mut terrain, "breakable", false);
        assert!(number(&fields(&terrain), "health").is_none());
    }

    /// Re-picking the item kind it already is must not reset its amount.
    #[test]
    fn reselecting_the_current_item_kind_keeps_its_values() {
        let mut item = ModelType::Item(ItemProps {
            kind: ItemKind::Coins { value: 99 },
        });
        set_choice(&mut item, "kind", "Coins");
        assert_eq!(number(&fields(&item), "value"), Some(99.0));
    }

    #[test]
    fn labels_and_options_are_plain_ascii() {
        for options in [ABILITIES, HANDS, AMMO, ATTACKS, TOWER_KINDS, ITEM_KINDS] {
            assert!(options.iter().all(|o| o.is_ascii()));
        }
    }
}
