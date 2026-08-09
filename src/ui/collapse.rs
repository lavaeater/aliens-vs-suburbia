//! Foldable sections for the tool screens.
//!
//! Both the playground and the asset browser are one tall column of lists in a narrow
//! pane, and there is never enough room for all of them. Which list needs the space
//! depends on what you are doing, so rather than pick heights that suit nobody, every
//! section title is a button that folds its contents away.
//!
//! A section is identified by a `&'static str` that doubles as its title. Sections are
//! declared by putting [`SectionHeader`] on the title row and [`SectionBody`] on each node
//! that folds with it — several bodies per section is normal, since a section is usually a
//! hint line, a list and a status line rather than one container.

use bevy::platform::collections::HashSet;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;
use lava_ui_builder::{InteractionPalette, TextTheme, UIBuilder};

/// The clickable title row. Its text carries the fold marker.
#[derive(Component, Clone, Copy)]
pub struct SectionHeader(pub &'static str);

/// A node that folds away with its section.
#[derive(Component, Clone, Copy)]
pub struct SectionBody(pub &'static str);

/// Which sections are currently folded.
///
/// One resource for every screen: the ids are distinct per screen, and a screen clears the
/// set when it opens, so nothing leaks across.
#[derive(Resource, Default)]
pub struct CollapsedSections(pub HashSet<&'static str>);

impl CollapsedSections {
    pub fn toggle(&mut self, section: &'static str) {
        if !self.0.remove(section) {
            self.0.insert(section);
        }
    }

    pub fn is_collapsed(&self, section: &str) -> bool {
        self.0.contains(section)
    }

    /// Marker shown in the header. ASCII only — Bevy's embedded font has nothing else.
    pub fn marker(&self, section: &str) -> &'static str {
        if self.is_collapsed(section) { "[+]" } else { "[-]" }
    }
}

/// Spawn a clickable section title as a child of the current builder node.
pub fn section_header(builder: &mut UIBuilder, section: &'static str, theme: &TextTheme) {
    let idle = Color::srgba(0.06, 0.11, 0.15, 0.9);
    builder.with_child(|c| {
        c.insert_bundle(lava_ui_builder::label(format!("[-] {section}"), theme))
            .insert(SectionHeader(section))
            .insert(BackgroundColor(idle))
            .insert(InteractionPalette {
                none: idle,
                hovered: Color::srgba(0.14, 0.28, 0.35, 0.95),
                pressed: Color::srgba(0.10, 0.22, 0.30, 1.0),
            })
            .insert(bevy::picking::hover::Hovered::default())
            .insert(bevy::ui_widgets::Button)
            .observe(move |_: On<Activate>, mut collapsed: ResMut<CollapsedSections>| {
                collapsed.toggle(section);
            })
            .modify_node(|mut n| {
                n.align_self = AlignSelf::Stretch;
                n.padding = UiRect::axes(Val::Px(4.0), Val::Px(3.0));
                n.margin = UiRect::top(Val::Px(4.0));
            });
    });
}

/// Fold and unfold sections, and keep the header markers honest.
///
/// Runs on change, plus once at startup — which is what `synced` is for, since a screen
/// spawns its headers after this system has already seen the resource unchanged.
pub fn sync_section_collapse(
    collapsed: Res<CollapsedSections>,
    mut bodies: Query<(&SectionBody, &mut Node)>,
    mut headers: Query<(&SectionHeader, &mut Text)>,
    mut synced: Local<bool>,
) {
    if !collapsed.is_changed() && *synced {
        return;
    }
    *synced = true;

    for (body, mut node) in bodies.iter_mut() {
        let wanted = if collapsed.is_collapsed(body.0) { Display::None } else { Display::Flex };
        if node.display != wanted {
            node.display = wanted;
        }
    }
    for (header, mut text) in headers.iter_mut() {
        **text = format!("{} {}", collapsed.marker(header.0), header.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODEL: &str = "MODEL";
    const IMPORT: &str = "IMPORT";

    #[test]
    fn a_section_folds_and_unfolds_on_repeated_clicks() {
        let mut collapsed = CollapsedSections::default();
        assert!(!collapsed.is_collapsed(MODEL), "everything starts open");
        collapsed.toggle(MODEL);
        assert!(collapsed.is_collapsed(MODEL));
        collapsed.toggle(MODEL);
        assert!(!collapsed.is_collapsed(MODEL));
    }

    #[test]
    fn folding_one_section_leaves_the_others_alone() {
        let mut collapsed = CollapsedSections::default();
        collapsed.toggle(IMPORT);
        assert!(collapsed.is_collapsed(IMPORT));
        assert!(!collapsed.is_collapsed(MODEL));
    }

    /// The marker is the only affordance saying a folded section is still there.
    #[test]
    fn the_header_marker_says_which_way_a_click_goes() {
        let mut collapsed = CollapsedSections::default();
        assert_eq!(collapsed.marker(MODEL), "[-]");
        collapsed.toggle(MODEL);
        assert_eq!(collapsed.marker(MODEL), "[+]");
    }
}
