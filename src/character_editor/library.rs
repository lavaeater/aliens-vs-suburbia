//! The Library tab: find any model under `assets/`, look at it, import it, open it.
//!
//! One list, built by [`library_rows`] from what is imported (the defs) and what is on
//! disk (the files), filtered by the search box and the type chips:
//!
//! - **Imported defs**, grouped by type. Clicking one opens it — worn, held or standing
//!   alone, by type (`doc::Docs::open`).
//! - **Files.** With the search box empty, the folder being browsed; with a query, every
//!   matching file anywhere under `assets/`. Each says what it is: already imported (and
//!   as what), new, or an animation library with no meshes.
//!
//! Clicking a **new** file previews it in Studio without writing anything; the bar under
//! the search box then offers *Import as <type>*. Clicking an **animation library** adds it
//! to the open character's `animation_sources`. Nothing about a file is written until you
//! import it, and importing never overwrites a def that already exists.
//!
//! The search box takes the keyboard while it has focus: Up / Down step through the list,
//! Enter opens the highlighted row.

use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use bevy::ui_widgets::Activate;

use crate::assets::asset_definition::AssetDefinition;
use crate::character_editor::animation::AnimationEditor;
use crate::character_editor::doc::{ConfirmAction, Docs, EditorFocus, PropDoc, Subject};
use crate::character_editor::models::{
    DefKind, EditorLibrary, ImportKind, add_animation_source, classify, def_stem,
};
use crate::character_editor::text_field::{FieldKey, TextField};
use crate::character_editor::ui::row;

/// The search box.
#[derive(Component)]
pub struct LibrarySearch;

/// The type filter chips.
#[derive(Component)]
pub struct LibraryChips;

/// The list itself.
#[derive(Component)]
pub struct LibraryList;

/// The "previewing X - Import as ..." bar, empty unless a new file is being previewed.
#[derive(Component)]
pub struct LibraryImportBar;

/// The library's one-line feedback.
#[derive(Component)]
pub struct LibraryStatus;

/// What a file on disk is, as far as the library is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileState {
    /// Has a def: opening the file opens that def.
    Imported { def_path: String, kind: DefKind },
    /// Has geometry and no def yet: previewable, importable.
    New,
    /// Clips but no meshes: belongs in a character's `animation_sources`.
    AnimsOnly,
}

/// One row of the list, in display order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LibraryRow {
    Heading(String),
    Def { def_path: String, kind: DefKind },
    Up,
    Folder(String),
    File { model_path: String, state: FileState },
}

impl LibraryRow {
    /// Rows Up / Down / Enter can land on.
    pub const fn selectable(&self) -> bool {
        !matches!(self, Self::Heading(_))
    }
}

/// Most files a search lists. A one-letter query matches nearly everything, and a list of
/// a few hundred rows is not something anyone reads.
pub const SEARCH_LIMIT: usize = 120;

fn matches(text: &str, query: &str) -> bool {
    query.is_empty() || text.to_lowercase().contains(&query.to_lowercase())
}

/// Build the list. Pure, so the filtering rules are tested without a screen.
///
/// `file_state` says what each file is; it is a closure because finding out reads the
/// file, and only the files actually listed should be read.
#[allow(clippy::too_many_arguments)]
pub fn library_rows(
    defs: &[(String, DefKind)],
    kind_filter: Option<DefKind>,
    query: &str,
    at_root: bool,
    folders: &[String],
    folder_files: &[String],
    all_files: &[String],
    mut file_state: impl FnMut(&str) -> FileState,
) -> Vec<LibraryRow> {
    let query = query.trim();
    let mut rows = Vec::new();

    let mut group = None;
    for (def_path, kind) in defs {
        if kind_filter.is_some_and(|k| k != *kind) || !matches(&def_stem(def_path), query) {
            continue;
        }
        if group != Some(*kind) {
            group = Some(*kind);
            rows.push(LibraryRow::Heading(kind.heading().to_string()));
        }
        rows.push(LibraryRow::Def {
            def_path: def_path.clone(),
            kind: *kind,
        });
    }

    if query.is_empty() {
        rows.push(LibraryRow::Heading("FILES".to_string()));
        if !at_root {
            rows.push(LibraryRow::Up);
        }
        rows.extend(folders.iter().cloned().map(LibraryRow::Folder));
        rows.extend(folder_files.iter().map(|model_path| LibraryRow::File {
            model_path: model_path.clone(),
            state: file_state(model_path),
        }));
    } else {
        rows.push(LibraryRow::Heading("FILES MATCHING".to_string()));
        rows.extend(
            all_files
                .iter()
                .filter(|path| matches(path, query))
                .take(SEARCH_LIMIT)
                .map(|model_path| LibraryRow::File {
                    model_path: model_path.clone(),
                    state: file_state(model_path),
                }),
        );
    }
    rows
}

/// Step the highlight to the next selectable row in `direction`, wrapping.
pub fn step_highlight(rows: &[LibraryRow], from: Option<usize>, direction: isize) -> Option<usize> {
    let count = rows.len();
    if count == 0 || !rows.iter().any(LibraryRow::selectable) {
        return None;
    }
    let mut index = from.map_or(if direction > 0 { count - 1 } else { 0 }, |i| i.min(count - 1));
    for _ in 0..count {
        index = (index as isize + direction).rem_euclid(count as isize) as usize;
        if rows.get(index).is_some_and(LibraryRow::selectable) {
            return Some(index);
        }
    }
    None
}

/// Every model file under `assets/`, relative to it, sorted.
pub fn scan_all_files() -> Vec<String> {
    let root = std::path::Path::new("assets");
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|ext| ext == "glb" || ext == "gltf")
                && let Ok(relative) = path.strip_prefix(root)
            {
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    files.sort();
    files
}

/// What a file is: imported (by its def's path, which is named after the model), new, or
/// an animation library. The geometry check reads the file, so answers are cached.
pub fn file_state(
    model_path: &str,
    defs: &[(String, DefKind)],
    cache: &mut HashMap<String, ImportKind>,
) -> FileState {
    let def_path = AssetDefinition::def_path(model_path)
        .to_string_lossy()
        .replace('\\', "/");
    if let Some((_, kind)) = defs.iter().find(|(path, _)| *path == def_path) {
        return FileState::Imported {
            def_path,
            kind: *kind,
        };
    }
    let kind = *cache
        .entry(model_path.to_string())
        .or_insert_with(|| classify(model_path));
    match kind {
        ImportKind::Model => FileState::New,
        ImportKind::AnimationLibrary => FileState::AnimsOnly,
    }
}

/// Do what clicking (or Enter on) a row does.
pub fn activate(row: &LibraryRow, docs: &mut Docs, animation: &mut AnimationEditor) {
    match row {
        LibraryRow::Heading(_) => {}
        LibraryRow::Def { def_path, .. } => {
            // Refused rather than opened: a character would spawn as nothing, and a prop
            // would stand invisible, with no clue why.
            if docs.library.mesh_less.contains(def_path) {
                docs.library.status = format!(
                    "{} has no geometry - it is an animation library, not a model",
                    def_stem(def_path)
                );
                docs.library.list_dirty = true;
                return;
            }
            docs.request(ConfirmAction::OpenModel(def_path.clone()));
        }
        LibraryRow::Up => docs.library.leave_folder(),
        LibraryRow::Folder(name) => docs.library.enter_folder(name),
        LibraryRow::File { model_path, state } => match state {
            FileState::Imported { def_path, .. } => {
                docs.request(ConfirmAction::OpenModel(def_path.clone()));
            }
            FileState::New => docs.request(ConfirmAction::Preview(model_path.clone())),
            FileState::AnimsOnly => {
                let Some(def) = docs.character.def.as_mut() else {
                    docs.library.status = "open a character to add clips to".to_string();
                    return;
                };
                let (changed, status) = add_animation_source(def, model_path);
                docs.library.status = status;
                if changed {
                    animation.ui_dirty = true;
                }
            }
        },
    }
    docs.library.list_dirty = true;
}

/// Mirror the search box into the library, and reset the highlight when the query changes.
pub fn sync_search_query(
    fields: Query<&TextField, (With<LibrarySearch>, Changed<TextField>)>,
    mut library: ResMut<EditorLibrary>,
) {
    let Ok(field) = fields.single() else {
        return;
    };
    if library.query != field.value {
        library.query.clone_from(&field.value);
        library.highlight = None;
        library.list_dirty = true;
    }
}

/// Up / Down / Enter from the search box.
pub fn navigate_library(
    mut keys: MessageReader<FieldKey>,
    search: Query<Entity, With<LibrarySearch>>,
    mut docs: Docs,
    mut animation: ResMut<AnimationEditor>,
) {
    let Ok(search) = search.single() else {
        keys.clear();
        return;
    };
    for key in keys.read() {
        match *key {
            FieldKey::Up(field) if field == search => {
                docs.library.highlight = step_highlight(&docs.library.rows, docs.library.highlight, -1);
                docs.library.list_dirty = true;
            }
            FieldKey::Down(field) if field == search => {
                docs.library.highlight = step_highlight(&docs.library.rows, docs.library.highlight, 1);
                docs.library.list_dirty = true;
            }
            FieldKey::Submit(field) if field == search => {
                // Enter with nothing highlighted opens the first thing that matched.
                let index = docs
                    .library
                    .highlight
                    .or_else(|| step_highlight(&docs.library.rows, None, 1));
                if let Some(row) = index.and_then(|i| docs.library.rows.get(i)).cloned() {
                    activate(&row, &mut docs, &mut animation);
                }
            }
            _ => {}
        }
    }
}

const ROW_DEF: Color = Color::srgb(0.85, 0.92, 0.85);
const ROW_FOLDER: Color = Color::srgb(0.75, 0.88, 1.0);
const ROW_NEW: Color = Color::srgb(0.9, 0.85, 0.7);
const ROW_ANIMS: Color = Color::srgb(0.7, 0.8, 0.95);
const ROW_MUTED: Color = Color::srgb(0.75, 0.6, 0.5);

/// The label and colour of a row. `*` marks what is open, `-` what else is in use (the
/// character worn, the weapon held), `>` the keyboard highlight.
fn row_label(row: &LibraryRow, open: &[Option<String>; 3], subject: Option<&str>, mesh_less: bool) -> (String, Color) {
    let file_name = |path: &str| path.rsplit('/').next().unwrap_or(path).to_string();
    match row {
        LibraryRow::Heading(text) => (text.clone(), HEADING),
        LibraryRow::Def { def_path, .. } => {
            let mark = if subject == Some(def_path.as_str()) {
                "*"
            } else if open.iter().flatten().any(|p| p == def_path) {
                "-"
            } else {
                " "
            };
            if mesh_less {
                (format!("{mark} {} (no mesh)", def_stem(def_path)), ROW_MUTED)
            } else {
                (format!("{mark} {}", def_stem(def_path)), ROW_DEF)
            }
        }
        LibraryRow::Up => ("  [..]".to_string(), ROW_FOLDER),
        LibraryRow::Folder(name) => (format!("  [dir] {name}"), ROW_FOLDER),
        LibraryRow::File { model_path, state } => match state {
            FileState::Imported { kind, .. } => (
                format!("  {}  ({})", file_name(model_path), kind.label()),
                ROW_DEF,
            ),
            FileState::New => (format!("+ {}  (new)", file_name(model_path)), ROW_NEW),
            FileState::AnimsOnly => (
                format!("~ {}  (anims -> add to character)", file_name(model_path)),
                ROW_ANIMS,
            ),
        },
    }
}

const HEADING: Color = Color::srgb(0.5, 0.7, 0.6);

/// Rebuild the chips, the list, the import bar and the status line.
#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub fn rebuild_library(
    mut library: ResMut<EditorLibrary>,
    doc: Res<crate::character_editor::doc::EditorDoc>,
    weapon_doc: Res<crate::character_editor::doc::WeaponDoc>,
    prop_doc: Res<PropDoc>,
    focus: Res<EditorFocus>,
    mut commands: Commands,
    lists: Query<Entity, With<LibraryList>>,
    chips: Query<Entity, With<LibraryChips>>,
    bars: Query<Entity, With<LibraryImportBar>>,
    mut statuses: Query<&mut Text, With<LibraryStatus>>,
    mut last_open: Local<[Option<String>; 3]>,
) {
    let open = [
        doc.def_path.clone(),
        weapon_doc.def_path.clone(),
        prop_doc.def_path.clone(),
    ];
    if *last_open != open || prop_doc.is_changed() {
        last_open.clone_from(&open);
        library.list_dirty = true;
    }
    if !library.list_dirty {
        return;
    }
    // Cleared only once there is somewhere to draw, so a frame without the containers
    // retries instead of leaving the list empty for the session.
    let (Ok(list), Ok(chip_row), Ok(bar)) = (lists.single(), chips.single(), bars.single())
    else {
        return;
    };
    library.list_dirty = false;

    let library = &mut *library;
    let mut cache = std::mem::take(&mut library.file_kinds);
    let rows = library_rows(
        &library.defs,
        library.kind_filter,
        &library.query,
        library.browse_folder.is_empty(),
        &library.folders,
        &library.files,
        &library.all_files,
        |path| file_state(path, &library.defs, &mut cache),
    );
    library.file_kinds = cache;
    if library.highlight.is_some_and(|i| i >= rows.len()) {
        library.highlight = None;
    }
    library.rows.clone_from(&rows);

    if let Ok(mut text) = statuses.single_mut() {
        let line = if library.query.is_empty() {
            format!("assets/{}   {}", library.browse_folder, library.status)
        } else {
            library.status.clone()
        };
        if text.0 != line {
            text.0 = line;
        }
    }

    // Chips: All, then one per kind.
    commands.entity(chip_row).despawn_related::<Children>();
    let filter = library.kind_filter;
    commands.entity(chip_row).with_children(|strip| {
        for kind in std::iter::once(None).chain(DefKind::ALL.into_iter().map(Some)) {
            let label = kind.map_or("All", DefKind::label).to_string();
            row(strip, label, filter == kind, Color::srgb(0.85, 0.9, 0.95)).observe(
                move |_: On<Activate>, mut library: ResMut<EditorLibrary>| {
                    library.kind_filter = kind;
                    library.highlight = None;
                    library.list_dirty = true;
                },
            );
        }
    });

    // The import bar, while a new file is being previewed.
    commands.entity(bar).despawn_related::<Children>();
    let previewing = (focus.subject == Subject::Prop && prop_doc.def_path.is_none())
        .then(|| prop_doc.def.as_ref().map(|def| def.model_path.clone()))
        .flatten();
    if let Some(model_path) = previewing {
        commands.entity(bar).with_children(|bar| {
            bar.spawn((
                Text::new(format!("previewing {} - import as:", def_stem(&model_path))),
                TextFont::default().with_font_size(11.0),
                TextColor(Color::srgb(1.0, 0.85, 0.5)),
            ));
            bar.spawn(Node {
                display: Display::Flex,
                flex_direction: FlexDirection::Row,
                flex_wrap: FlexWrap::Wrap,
                column_gap: Val::Px(4.0),
                row_gap: Val::Px(4.0),
                ..Default::default()
            })
            .with_children(|strip| {
                for kind in DefKind::ALL {
                    let path = model_path.clone();
                    row(strip, kind.label().to_string(), false, ROW_NEW).observe(
                        move |_: On<Activate>, mut docs: Docs| {
                            docs.import_as(&path, kind);
                        },
                    );
                }
            });
        });
    }

    // The list.
    commands.entity(list).despawn_related::<Children>();
    let subject = match focus.subject {
        Subject::Character => open[0].clone(),
        Subject::Weapon => open[1].clone(),
        Subject::Prop => open[2].clone(),
    };
    let highlight = library.highlight;
    let mesh_less = library.mesh_less.clone();
    commands.entity(list).with_children(|parent| {
        if rows.iter().all(|r| !r.selectable()) {
            parent.spawn((
                Text::new("nothing matches"),
                TextFont::default().with_font_size(11.0),
                TextColor(ROW_MUTED),
            ));
        }
        for (index, entry) in rows.into_iter().enumerate() {
            let is_mesh_less = matches!(&entry, LibraryRow::Def { def_path, .. } if mesh_less.contains(def_path));
            let (label, color) = row_label(&entry, &open, subject.as_deref(), is_mesh_less);
            if let LibraryRow::Heading(_) = entry {
                parent.spawn((
                    Text::new(label),
                    TextFont::default().with_font_size(10.0),
                    TextColor(color),
                    Node {
                        margin: UiRect::top(Val::Px(4.0)),
                        ..Default::default()
                    },
                ));
                continue;
            }
            let is_open = match &entry {
                LibraryRow::Def { def_path, .. } => subject.as_deref() == Some(def_path.as_str()),
                _ => false,
            };
            let label = if highlight == Some(index) {
                format!(">{}", label.get(1..).unwrap_or_default())
            } else {
                label
            };
            row(parent, label, is_open || highlight == Some(index), color).observe(
                move |_: On<Activate>, mut docs: Docs, mut animation: ResMut<AnimationEditor>| {
                    docs.library.highlight = Some(index);
                    activate(&entry, &mut docs, &mut animation);
                },
            );
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defs() -> Vec<(String, DefKind)> {
        vec![
            ("assets/defs/amy.ron".to_string(), DefKind::Player),
            ("assets/defs/Pistol.ron".to_string(), DefKind::Weapon),
            ("assets/defs/Alien.ron".to_string(), DefKind::Enemy),
        ]
    }

    fn build(kind: Option<DefKind>, query: &str) -> Vec<LibraryRow> {
        library_rows(
            &defs(),
            kind,
            query,
            false,
            &["characters".to_string()],
            &["packs/a/Box.glb".to_string()],
            &[
                "packs/a/Box.glb".to_string(),
                "packs/b/Pistol.glb".to_string(),
                "packs/c/anims.glb".to_string(),
            ],
            |_| FileState::New,
        )
    }

    fn def_names(rows: &[LibraryRow]) -> Vec<String> {
        rows.iter()
            .filter_map(|row| match row {
                LibraryRow::Def { def_path, .. } => Some(def_stem(def_path)),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn with_no_query_the_folder_is_browsed() {
        let rows = build(None, "");
        assert_eq!(def_names(&rows), ["amy", "Pistol", "Alien"]);
        assert!(rows.contains(&LibraryRow::Up));
        assert!(rows.contains(&LibraryRow::Folder("characters".to_string())));
    }

    #[test]
    fn a_type_chip_keeps_only_that_type() {
        assert_eq!(def_names(&build(Some(DefKind::Weapon), "")), ["Pistol"]);
    }

    /// A query searches every folder, not just the one being browsed, and is
    /// case-insensitive.
    #[test]
    fn a_query_searches_everywhere() {
        let rows = build(None, "pIsToL");
        assert_eq!(def_names(&rows), ["Pistol"]);
        assert!(rows.contains(&LibraryRow::File {
            model_path: "packs/b/Pistol.glb".to_string(),
            state: FileState::New,
        }));
        assert!(!rows.contains(&LibraryRow::Up), "no folder browsing while searching");
    }

    #[test]
    fn the_highlight_skips_headings_and_wraps() {
        let rows = vec![
            LibraryRow::Heading("A".into()),
            LibraryRow::Up,
            LibraryRow::Heading("B".into()),
            LibraryRow::Folder("x".into()),
        ];
        assert_eq!(step_highlight(&rows, None, 1), Some(1));
        assert_eq!(step_highlight(&rows, Some(1), 1), Some(3));
        assert_eq!(step_highlight(&rows, Some(3), 1), Some(1), "wraps past the end");
        assert_eq!(step_highlight(&rows, Some(1), -1), Some(3), "wraps past the start");
        assert_eq!(step_highlight(&[LibraryRow::Heading("A".into())], None, 1), None);
    }

    #[test]
    fn a_file_with_a_def_is_imported_as_that_kind() {
        let mut cache = HashMap::new();
        let state = file_state("packs/mesh2motion/amy.glb", &defs(), &mut cache);
        assert_eq!(
            state,
            FileState::Imported {
                def_path: "assets/defs/amy.ron".to_string(),
                kind: DefKind::Player,
            }
        );
    }
}
