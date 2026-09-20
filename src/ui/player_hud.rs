//! The bottom-of-screen player bar: one quarter per roster slot showing the character's
//! name, health, weapon, ammo and ability charge. Slots without a player are hidden.
//!
//! Ammo reads `--` until the ammo system lands (roadmap phase 3); the label is already
//! here so the layout does not shift when it does.

use bevy::prelude::*;
use lava_ui_builder::{progress_bar, LavaTheme, ProgressBar, TextStyle, UIBuilder};

use crate::general::components::Health;
use crate::player::components::{Player, PlayerDead, PlayerSlot};
use crate::player::systems::abilities::{AbilityCooldown, SpecialAbility};
use crate::player::systems::equip::EquippedWeapon;
use crate::player::systems::shoot::Weapon;
use crate::player_setup::state::PlayerRoster;
use crate::ui::spawn_ui::StateMarker;

/// Maximum roster size; the bar always has this many slots so widths are stable.
pub const HUD_SLOTS: usize = 4;

/// Root node of one player's quarter of the bar.
#[derive(Component)]
pub struct PlayerHudSlot(pub usize);

#[derive(Component)]
pub struct HudSlotName;
#[derive(Component)]
pub struct HudSlotHealthBar;
#[derive(Component)]
pub struct HudSlotHealthText;
#[derive(Component)]
pub struct HudSlotWeapon;
#[derive(Component)]
pub struct HudSlotAmmo;
#[derive(Component)]
pub struct HudSlotAbility;

const SLOT_BG: Color = Color::srgba(0.05, 0.12, 0.07, 0.75);
const NAME_COLOR: Color = Color::srgb(0.85, 1.0, 0.88);
const DIM_COLOR: Color = Color::srgb(0.55, 0.55, 0.55);
const WEAPON_COLOR: Color = Color::srgb(0.9, 0.85, 0.6);
const AMMO_COLOR: Color = Color::srgb(0.8, 0.8, 0.8);
const ABILITY_COLOR: Color = Color::srgb(0.5, 0.9, 1.0);

pub fn spawn_player_bar(mut commands: Commands, theme: Res<LavaTheme>) {
    let mut ui = UIBuilder::new(commands.reborrow(), Some(theme.clone()));
    ui.insert(StateMarker)
        .insert(Name::new("PlayerBar"))
        .absolute_position()
        .bottom(px(0.0))
        .left(px(0.0))
        .width_percent(100.0)
        .display_flex()
        .column_gap_px(4.0)
        .padding_all_px(4.0);

    for slot in 0..HUD_SLOTS {
        ui.with_child(|c| {
            c.insert(PlayerHudSlot(slot))
                .insert(Name::new(format!("PlayerHudSlot{slot}")))
                .display_none()
                .with_flex_grow(1.0)
                .flex_column()
                .row_gap_px(2.0)
                .padding_all_px(6.0)
                .bg_color(SLOT_BG)
                .border_radius_all_px(4.0);
            // Width is shared equally by whichever slots are visible.
            c.modify_node(|mut n| n.flex_basis = Val::Px(0.0));

            c.with_child(|c| {
                c.with_text("", Some(TextStyle::size_color(16.0, NAME_COLOR))).insert(HudSlotName);
            });
            c.with_child(|c| {
                c.display_flex().align_items_center().column_gap_px(6.0);
                c.with_child(|c| {
                    c.insert_bundle(progress_bar(
                        1.0,
                        120.0,
                        10.0,
                        Color::srgb(0.2, 0.85, 0.3),
                        Color::srgba(0.0, 0.0, 0.0, 0.5),
                    ))
                    .insert(HudSlotHealthBar);
                });
                c.with_child(|c| {
                    c.with_text("", Some(TextStyle::size_color(12.0, AMMO_COLOR))).insert(HudSlotHealthText);
                });
            });
            c.with_child(|c| {
                c.with_text("", Some(TextStyle::size_color(13.0, WEAPON_COLOR))).insert(HudSlotWeapon);
            });
            c.with_child(|c| {
                c.with_text("", Some(TextStyle::size_color(13.0, AMMO_COLOR))).insert(HudSlotAmmo);
            });
            c.with_child(|c| {
                c.with_text("", Some(TextStyle::size_color(13.0, ABILITY_COLOR))).insert(HudSlotAbility);
            });
        });
    }
    ui.build();
}

/// Character name for a slot: the def's file stem, or a generic label when there is no
/// roster (e.g. the playground or a skipped setup screen).
pub fn slot_name(roster: Option<&PlayerRoster>, slot: usize) -> String {
    roster
        .and_then(|r| r.def_paths.get(slot))
        .and_then(|p| std::path::Path::new(p).file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| format!("Player {}", slot + 1))
}

/// Ability line: "[Q] Bombardment - READY" or "... - 40%".
pub fn ability_label(ability: &SpecialAbility, meter: &AbilityCooldown) -> String {
    if meter.ready() {
        format!("[Q] {} - READY", ability.label())
    } else {
        format!("[Q] {} - {}%", ability.label(), (meter.charge * 100.0) as u32)
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn update_player_bar(
    roster: Option<Res<PlayerRoster>>,
    players: Query<
        (&PlayerSlot, &Health, Option<&EquippedWeapon>, Option<&SpecialAbility>, Option<&AbilityCooldown>, Has<PlayerDead>),
        With<Player>,
    >,
    weapon_names: Query<&Name, With<Weapon>>,
    mut slots: Query<(Entity, &PlayerHudSlot, &mut Node)>,
    children_q: Query<&Children>,
    mut names: Query<(&mut Text, &mut TextColor), (With<HudSlotName>, Without<HudSlotHealthText>, Without<HudSlotWeapon>, Without<HudSlotAmmo>, Without<HudSlotAbility>)>,
    mut health_texts: Query<&mut Text, (With<HudSlotHealthText>, Without<HudSlotName>, Without<HudSlotWeapon>, Without<HudSlotAmmo>, Without<HudSlotAbility>)>,
    mut weapons: Query<&mut Text, (With<HudSlotWeapon>, Without<HudSlotName>, Without<HudSlotHealthText>, Without<HudSlotAmmo>, Without<HudSlotAbility>)>,
    mut ammos: Query<&mut Text, (With<HudSlotAmmo>, Without<HudSlotName>, Without<HudSlotHealthText>, Without<HudSlotWeapon>, Without<HudSlotAbility>)>,
    mut abilities: Query<&mut Text, (With<HudSlotAbility>, Without<HudSlotName>, Without<HudSlotHealthText>, Without<HudSlotWeapon>, Without<HudSlotAmmo>)>,
    mut bars: Query<&mut ProgressBar, With<HudSlotHealthBar>>,
) {
    for (slot_entity, hud_slot, mut node) in slots.iter_mut() {
        let player = players.iter().find(|(s, ..)| s.0 == hud_slot.0);
        let Some((_, health, equipped, ability, meter, dead)) = player else {
            node.display = Display::None;
            continue;
        };
        node.display = Display::Flex;

        let name = slot_name(roster.as_deref(), hud_slot.0);
        let weapon = equipped
            .and_then(|e| weapon_names.get(e.0).ok())
            .map(|n| n.as_str().to_string())
            .unwrap_or_else(|| "Unarmed".to_string());
        let ability_text = match (ability, meter) {
            (Some(a), Some(m)) => ability_label(a, m),
            _ => String::new(),
        };
        let fraction = (health.health as f32 / health.max_health.max(1) as f32).clamp(0.0, 1.0);

        // Walk this slot's subtree once and fill whatever labelled nodes it holds.
        for entity in descendants(slot_entity, &children_q) {
            if let Ok((mut t, mut color)) = names.get_mut(entity) {
                **t = if dead { format!("{name} - DOWN") } else { name.clone() };
                *color = TextColor(if dead { DIM_COLOR } else { NAME_COLOR });
            } else if let Ok(mut t) = health_texts.get_mut(entity) {
                **t = format!("{} / {}", health.health.max(0), health.max_health);
            } else if let Ok(mut t) = weapons.get_mut(entity) {
                **t = weapon.clone();
            } else if let Ok(mut t) = ammos.get_mut(entity) {
                **t = "Ammo: --".to_string();
            } else if let Ok(mut t) = abilities.get_mut(entity) {
                **t = ability_text.clone();
            } else if let Ok(mut bar) = bars.get_mut(entity) {
                bar.value = fraction;
            }
        }
    }
}

/// Every entity below `root` (not including it), depth first.
fn descendants(root: Entity, children_q: &Query<&Children>) -> Vec<Entity> {
    let mut out = Vec::new();
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Ok(children) = children_q.get(e) {
            for child in children.iter() {
                out.push(child);
                stack.push(child);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::slot_name;
    use crate::player_setup::state::PlayerRoster;

    #[test]
    fn slot_name_is_the_def_stem() {
        let roster = PlayerRoster { def_paths: vec!["assets/defs/amy.ron".into()], devices: vec![] };
        assert_eq!(slot_name(Some(&roster), 0), "amy");
    }

    #[test]
    fn missing_roster_or_slot_falls_back_to_a_number() {
        let roster = PlayerRoster { def_paths: vec![], devices: vec![] };
        assert_eq!(slot_name(Some(&roster), 1), "Player 2");
        assert_eq!(slot_name(None, 0), "Player 1");
    }
}
