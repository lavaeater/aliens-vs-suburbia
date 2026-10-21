# UI Refactoring Plan

## Goal

Make `lava_ui_builder` the only place anyone needs to look to build UI.
Every screen should read like a description of its structure, not a manual
assembly of Bevy components.

---

## What we have

`lava_ui_builder` already provides:
- Full layout API (`width`, `height`, `padding`, `gap`, `flex_row`, etc.)
- `add_row`, `add_column`, `add_panel`, `add_grid`, `add_centered`
- `add_button_observe`, `add_themed_button_observe`
- `with_child`, `foreach_child`
- `with_text(text, style)` + `TextStyle::size()` / `::color()` / `::size_color()`
- `progress_bar()`, `with_collapsible()`
- Theme system (`LavaTheme`, `ButtonTheme`, `TextTheme`)

---

## Repeated patterns not yet abstracted

### 1. Themed text labels (most frequent pain point)

Every screen passes `&t` (a cloned `TextTheme`) into `lava_ui_builder::label()`
and wraps it in `insert_bundle()`. This is the most common boilerplate:

```rust
// current — requires cloning theme, import, insert_bundle
left.with_child(|c| {
    c.insert_bundle(lava_ui_builder::header("Map Editor", &t));
});
left.with_child(|c| {
    c.insert_bundle(lava_ui_builder::label("[R] rotate", &dim));
});
```

The builder already holds the theme. It should expose direct methods:

```rust
// target
left.themed_header("Map Editor");
left.themed_label("[R] rotate");
left.label("seed: —", 11.0, Color::srgba(0.5, 0.7, 0.5, 0.7));
```

### 2. Side panel

Every tool screen (map editor ×2, asset browser ×2, poly_pizza ×2) opens with:

```rust
panel.modify_node(|mut n| {
    n.width = Val::Px(200.0);
    n.height = Val::Percent(100.0);
    n.flex_direction = FlexDirection::Column;
    n.padding = UiRect::all(Val::Px(6.0));
    n.row_gap = Val::Px(3.0);
}).bg_color(Color::srgba(0.04, 0.08, 0.05, 0.95));
```

Target:

```rust
ui.side_panel(200.0, Color::srgba(0.04, 0.08, 0.05, 0.95), |panel| { ... });
```

### 3. Scrollable list container

Every scrollable item list (palette, wave list, enemy picker, folder list, node list…):

```rust
container.display_flex().flex_column().gap_px(2.0)
    .with_flex_grow(1.0).width_percent(100.0)
    .overflow_scroll_y()
    .insert(MyMarker);
```

Target:

```rust
ui.scrollable_list(|list| { list.insert(MyMarker); });
ui.scrollable_list_bounded(120.0, |list| { list.insert(MyMarker); });
```

### 4. Selectable list item

The "green highlight on select" item appears ~20 times across map_editor,
asset_browser, and poly_pizza. Always the same: full-width row, padding,
border-radius, `InteractionPalette`, `Button`, text child, observer.

```rust
// current — 10 lines every time
parent.spawn((Node { width: Val::Percent(100.0), padding: UiRect::axes(Val::Px(6.0), Val::Px(4.0)), border_radius: BorderRadius::all(Val::Px(3.0)), ..Default::default() },
    BackgroundColor(bg), InteractionPalette { none: bg, hovered: ..., pressed: ... },
    bevy::picking::hover::Hovered::default(), bevy::ui_widgets::Button))
    .with_child((Text::new(name), TextFont::default().with_font_size(11.0), TextColor(tc)))
    .observe(move |_: On<Activate>, ...| { ... });
```

Target (inside `with_children` context, where we need raw `Commands`):

```rust
// Via UIBuilder (build-time lists):
ui.list_item(name, selected, move |_: On<Activate>, ...| { ... });

// Via ChildSpawner (runtime-rebuild lists):
// add a free function: spawn_list_item(parent, name, selected, handler)
spawn_list_item(parent, &name, selected, move |_: On<Activate>, ...| { ... });
```

### 5. Icon / delete button

Small square buttons (16–28 px) with a single glyph appear in wave rows,
asset browser sources, etc.:

```rust
// current
row.spawn((Node { width: Val::Px(16.0), height: Val::Px(16.0), ... },
    BackgroundColor(RED), Hovered::default(), Button,
    InteractionPalette { ... }))
    .with_child((Text::new("x"), ...))
    .observe(...);
```

Target:

```rust
ui.icon_button("x", 16.0, move |_: On<Activate>, ...| { ... });
ui.delete_button(move |_: On<Activate>, ...| { ... }); // fixed 16px red "x"
```

### 6. Missing layout setters

`min_width`, `max_width`, `min_height`, `max_height` — currently require
`modify_node(|mut n| n.min_width = Val::Px(...))` — should be 1-liners.

### 7. Section divider label

The `"-- Waves --"` / `"-- Enemy --"` pattern appears in multiple panels.
A `section_label(text)` method would unify font size, color, and style.

---

## Plan

### Phase 1 — Add to `lava_ui_builder` (no game-code changes yet)

| Addition | Where in builder | Notes |
|---|---|---|
| `themed_header(text)` | `builder.rs` | Uses builder's own theme |
| `themed_label(text)` | `builder.rs` | Uses builder's own theme |
| `label(text, size, color)` | `builder.rs` | Inline override, no TextTheme needed |
| `section_label(text)` | `builder.rs` | Dim divider style |
| `min_width(Val)`, `max_width(Val)` | `builder.rs` | Missing layout setters |
| `min_height(Val)`, `max_height(Val)` | `builder.rs` | Missing layout setters |
| `side_panel(width, bg, f)` | `builder.rs` | Full-height column panel |
| `scrollable_list(f)` | `builder.rs` | flex_column + grow + scroll |
| `scrollable_list_bounded(max_h, f)` | `builder.rs` | same + max_height cap |
| `list_item(name, selected, handler)` | `builder.rs` | Selectable row with InteractionPalette |
| `icon_button(glyph, size, handler)` | `builder.rs` | Square glyph button |
| `delete_button(handler)` | `builder.rs` | 16 px red "x" icon_button |
| `spawn_list_item(parent, …)` | `lib.rs` | Free fn for raw `with_children` contexts |

Colors for `list_item` / `delete_button` will be parameterized or use
theme-derived defaults. The green palette (`0.15, 0.40, 0.20` selected,
`0.07, 0.12, 0.09` unselected) is specific to this game's tool screens, so
it becomes the default but can be overridden.

### Phase 2 — Refactor `map_editor/ui.rs`

This is the most recently written and most consistent screen — ideal starting
point. Every manual pattern in it matches something from Phase 1:

- Side panels → `side_panel()`
- `with_child(|c| { c.insert_bundle(label(...)) })` → `themed_label()` / `label()`
- `with_child(|c| { c.insert_bundle(header(...)) })` → `themed_header()`
- The `"-- Enemy --"` / `"-- Waves --"` labels → `section_label()`
- Scrollable containers → `scrollable_list()` / `scrollable_list_bounded()`
- `rebuild_palette` / `rebuild_enemy_picker` → `spawn_list_item()`
- Wave delete button → `delete_button()`

Expected line count: ~360 → ~200.

### Phase 3 — Refactor `spawn_ui.rs` (HUD + settings panels)

- HUD label blocks → `label(text, size, color).insert(Marker)` chain
- `modify_node` for min_width → `min_width()`
- Settings panel setup → `side_panel()` or builder layout chains

Expected line count: ~400 lines of UI construction → ~260.

### Phase 4 — Refactor `asset_browser/ui.rs` and `poly_pizza/ui.rs`

Larger, more complex. After Phases 1–3 prove the patterns work:
- `list_item` / `spawn_list_item` for all chip/tag/folder lists
- `side_panel` for left/right sidebars
- `icon_button` for +/- height buttons and source delete buttons

---

## Ergonomics improvements (beyond current scope but worth noting)

- **`themed_label` / `themed_header` as insertable bundles** — the bundle-function
  API (`label(text, &theme)`) is great for `children![]` macros. The imperative
  API should offer equivalents that don't need the theme passed in.

- **`UIBuilder::hud_label(text, style, marker)`** — for HUD labels that are
  absolutely positioned and have a specific marker component. Currently 4 lines
  of boilerplate per label.

- **Color palette constants** — hardcoded `Color::srgba(0.07, 0.12, 0.09, 0.85)`
  appears ~15 times. A `const PANEL_BG: Color` in game code (not the library)
  would make intent clear.

- **`UIBuilder::spacer()`** — a flex_grow(1) child to push content apart.
  Currently `c.with_flex_grow(1.0)` inside `with_child`, which works but could
  be one word.

---

## What NOT to do

- Do not unify `asset_browser` and `poly_pizza` UI styles — they intentionally
  differ. The abstraction should be structural, not visual.
- Do not make `list_item` take a `LavaTheme` parameter — the builder already
  has one. The green color is an in-game default, not a design-system concern.
- Do not add a macro layer. The imperative builder is already expressive enough;
  macros would hide Rust's borrow checker errors in confusing ways.
