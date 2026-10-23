//! Drop tables: what falls out of an alien (or a crate) when it dies.
//!
//! A [`LootTable`] is rolled `rolls` times over weighted [`LootEntry`]s; `Nothing` is an
//! ordinary entry so "70% of the time you get nothing" is written as data, not code.
//! `always` entries drop every time on top of the rolls, and an entry may point at
//! another table by name so common sub-tables (say "small ammo") are shared. Tables live
//! in `assets/loot/<name>.ron` and are addressed by file stem.
//!
//! An entity drops loot by carrying [`LootDrop`] naming its table; [`spawn_loot_on_death`]
//! rolls it the frame its `Health` hits zero and asks `items` to place the results.
//! A loot bag (`ItemKind::Loot`) rolls its table when a player walks into it; [`bags`]
//! scatters them over a map.

pub mod bags;

use std::collections::HashMap;

use avian3d::prelude::Position;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::assets::asset_definition::ItemKind;
use crate::general::components::Health;
use crate::items::SpawnItem;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum LootEntry {
    Nothing {
        weight: f32,
    },
    Item {
        weight: f32,
        kind: ItemKind,
        count: u32,
    },
    /// Roll another table (by file stem) in this slot.
    Table {
        weight: f32,
        table: String,
    },
}

impl LootEntry {
    const fn weight(&self) -> f32 {
        match self {
            Self::Nothing { weight } | Self::Item { weight, .. } | Self::Table { weight, .. } => {
                *weight
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LootTable {
    /// How many weighted picks to make.
    #[serde(default = "default_rolls")]
    pub rolls: u32,
    /// Dropped every time, regardless of the rolls.
    #[serde(default)]
    pub always: Vec<LootEntry>,
    #[serde(default)]
    pub entries: Vec<LootEntry>,
}

const fn default_rolls() -> u32 {
    1
}

impl Default for LootTable {
    fn default() -> Self {
        Self {
            rolls: 1,
            always: Vec::new(),
            entries: Vec::new(),
        }
    }
}

/// The table a loot bag rolls unless it names another: anything at all.
pub const DEFAULT_BAG_TABLE: &str = "trash_bag";

/// Every table found under `assets/loot`, by file stem.
#[derive(Resource, Debug, Default)]
pub struct LootTables(pub HashMap<String, LootTable>);

impl LootTables {
    pub fn load_dir(dir: &str) -> Self {
        let mut tables = HashMap::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                    continue;
                }
                let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                    continue;
                };
                match std::fs::read_to_string(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|t| ron::from_str::<LootTable>(&t).map_err(|e| e.to_string()))
                {
                    Ok(table) => {
                        tables.insert(stem.to_string(), table);
                    }
                    Err(e) => warn!("loot table {} failed to load: {e}", path.display()),
                }
            }
        }
        Self(tables)
    }

    /// Roll a table by name into a flat list of drops. Missing tables drop nothing.
    pub fn roll(&self, name: &str, rng: &mut impl FnMut() -> f32) -> Vec<(ItemKind, u32)> {
        let mut out = Vec::new();
        self.roll_into(name, rng, &mut out, 0);
        out
    }

    fn roll_into(
        &self,
        name: &str,
        rng: &mut impl FnMut() -> f32,
        out: &mut Vec<(ItemKind, u32)>,
        depth: u8,
    ) {
        // Tables that reference each other would otherwise recurse forever.
        if depth > 4 {
            return;
        }
        let Some(table) = self.0.get(name) else {
            return;
        };
        for entry in &table.always {
            self.resolve(entry, rng, out, depth);
        }
        let total: f32 = table
            .entries
            .iter()
            .map(LootEntry::weight)
            .filter(|w| *w > 0.0)
            .sum();
        if total <= 0.0 {
            return;
        }
        for _ in 0..table.rolls {
            let mut pick = rng().clamp(0.0, 0.999_999) * total;
            for entry in &table.entries {
                let w = entry.weight();
                if w <= 0.0 {
                    continue;
                }
                if pick < w {
                    self.resolve(entry, rng, out, depth);
                    break;
                }
                pick -= w;
            }
        }
    }

    fn resolve(
        &self,
        entry: &LootEntry,
        rng: &mut impl FnMut() -> f32,
        out: &mut Vec<(ItemKind, u32)>,
        depth: u8,
    ) {
        match entry {
            LootEntry::Nothing { .. } => {}
            LootEntry::Item { kind, count, .. } => out.push((kind.clone(), (*count).max(1))),
            LootEntry::Table { table, .. } => self.roll_into(table, rng, out, depth + 1),
        }
    }
}

/// Names the table rolled when this entity dies.
#[derive(Component, Debug, Clone)]
pub struct LootDrop(pub String);

/// Marks an entity whose table has been rolled, so a corpse lingering at zero health for
/// a frame does not drop twice.
#[derive(Component)]
pub struct LootRolled;

pub struct LootPlugin;

impl Plugin for LootPlugin {
    fn build(&self, app: &mut App) {
        // `spawn_loot_on_death` is scheduled by `GamePlugin`, where it can be ordered
        // against the death/despawn systems; adding it here too would make the
        // ordering ambiguous.
        app.insert_resource(LootTables::load_dir("assets/loot"))
            .add_systems(
                Update,
                bags::spawn_loot_bags.run_if(in_state(crate::game_state::GameState::InGame)),
            );
    }
}

/// Roll and drop for anything with [`LootDrop`] the frame it dies. Must run before the
/// systems that despawn dead things.
#[allow(clippy::type_complexity)]
pub fn spawn_loot_on_death(
    mut commands: Commands,
    tables: Res<LootTables>,
    dying: Query<
        (
            Entity,
            &Health,
            &LootDrop,
            Option<&Position>,
            Option<&Transform>,
        ),
        Without<LootRolled>,
    >,
    mut spawn_mw: MessageWriter<SpawnItem>,
    mut seed: Local<u32>,
) {
    for (entity, health, drop, pos, transform) in dying.iter() {
        if !health.is_dead() {
            continue;
        }
        commands.entity(entity).try_insert(LootRolled);
        let origin = pos
            .map(|p| p.0)
            .or_else(|| transform.map(|t| t.translation))
            .unwrap_or(Vec3::ZERO);
        let drops = tables.roll(&drop.0, &mut || unit_random(&mut seed));
        spawn_mw.write_batch(scatter(origin, drops, 0));
    }
}

/// Lay rolled drops out around `origin` on a widening spiral so a pile does not stack.
/// The first `skip` spots are left free (the middle, for a bag that is still there).
pub fn scatter(origin: Vec3, drops: Vec<(ItemKind, u32)>, skip: usize) -> Vec<SpawnItem> {
    let mut out = Vec::new();
    for (i, (kind, count)) in drops.into_iter().enumerate() {
        for j in 0..count {
            let n = i * 3 + j as usize + skip;
            let angle = n as f32 * 2.4;
            let radius = if n == 0 { 0.0 } else { 0.35 + 0.1 * n as f32 };
            out.push(SpawnItem {
                kind: kind.clone(),
                position: origin + Vec3::new(angle.cos() * radius, 0.0, angle.sin() * radius),
            });
        }
    }
    out
}

/// The next number in `0..1` from a xorshift stream kept in `seed`.
pub fn unit_random(seed: &mut u32) -> f32 {
    *seed = xorshift(seed.wrapping_add(0x9E37_79B9));
    (*seed >> 8) as f32 / (1u32 << 24) as f32
}

fn xorshift(mut x: u32) -> u32 {
    x = x.max(1);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assets::asset_definition::AmmoKind;

    fn tables() -> LootTables {
        let mut t = LootTables::default();
        t.0.insert(
            "alien".into(),
            LootTable {
                rolls: 1,
                always: vec![LootEntry::Item {
                    weight: 1.0,
                    kind: ItemKind::Coins { value: 5 },
                    count: 1,
                }],
                entries: vec![
                    LootEntry::Nothing { weight: 70.0 },
                    LootEntry::Item {
                        weight: 20.0,
                        kind: ItemKind::HealthPickup { amount: 25.0 },
                        count: 1,
                    },
                    LootEntry::Table {
                        weight: 10.0,
                        table: "ammo".into(),
                    },
                ],
            },
        );
        t.0.insert(
            "ammo".into(),
            LootTable {
                rolls: 1,
                always: vec![],
                entries: vec![LootEntry::Item {
                    weight: 1.0,
                    kind: ItemKind::AmmoPickup {
                        kind: AmmoKind::Pistol,
                        rounds: 12,
                    },
                    count: 2,
                }],
            },
        );
        t.0.insert(
            "loop".into(),
            LootTable {
                rolls: 1,
                always: vec![LootEntry::Table {
                    weight: 1.0,
                    table: "loop".into(),
                }],
                entries: vec![],
            },
        );
        t
    }

    #[test]
    fn always_entries_drop_and_nothing_is_honoured() {
        let t = tables();
        let mut rng = || 0.1; // lands in the 70% Nothing band
        let drops = t.roll("alien", &mut rng);
        assert_eq!(drops, vec![(ItemKind::Coins { value: 5 }, 1)]);
    }

    #[test]
    fn the_weighted_band_picks_the_right_entry() {
        let t = tables();
        let mut rng = || 0.8; // 0.8 * 100 = 80: past Nothing (70), inside Health (70..90)
        let drops = t.roll("alien", &mut rng);
        assert_eq!(drops.len(), 2);
        assert_eq!(drops[1], (ItemKind::HealthPickup { amount: 25.0 }, 1));
    }

    #[test]
    fn nested_tables_resolve_with_their_count() {
        let t = tables();
        let mut rng = || 0.95; // 95: the ammo sub-table band (90..100)
        let drops = t.roll("alien", &mut rng);
        assert_eq!(
            drops[1],
            (
                ItemKind::AmmoPickup {
                    kind: AmmoKind::Pistol,
                    rounds: 12
                },
                2
            )
        );
    }

    #[test]
    fn unknown_and_self_referencing_tables_are_harmless() {
        let t = tables();
        let mut rng = || 0.5;
        assert!(t.roll("nope", &mut rng).is_empty());
        assert!(t.roll("loop", &mut rng).is_empty());
    }

    #[test]
    fn a_scattered_pile_does_not_stack_and_can_leave_the_middle_free() {
        let drops = vec![(ItemKind::Coins { value: 1 }, 2)];
        let all = scatter(Vec3::ZERO, drops.clone(), 0);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].position, Vec3::ZERO);
        assert!(all[1].position.length() > 0.3);
        let around = scatter(Vec3::ZERO, drops, 1);
        assert!(around.iter().all(|s| s.position.length() > 0.3));
    }

    #[test]
    fn the_shipped_tables_parse() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/loot");
        let tables = LootTables::load_dir(dir);
        for name in ["alien", DEFAULT_BAG_TABLE] {
            assert!(
                tables.0.contains_key(name),
                "assets/loot/{name}.ron must exist and parse"
            );
        }
    }
}
