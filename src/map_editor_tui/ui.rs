use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};
use enumflags2::BitFlags;
use crate::map::MapFeatures;
use super::app::{App, Mode, PromptKind};

fn tile_color(raw: u64) -> Color {
    if raw == 0 { return Color::Rgb(20, 20, 20); }
    let f = BitFlags::<MapFeatures>::from_bits_truncate(raw);
    if f.contains(MapFeatures::PlayerSpawn)  { return Color::Rgb(40, 160, 200); }
    if f.contains(MapFeatures::EnemySpawn)   { return Color::Rgb(180, 40, 40); }
    if f.contains(MapFeatures::EnemyExit)    { return Color::Rgb(40, 160, 40); }
    if f.contains(MapFeatures::ImpassableForPlayers) && f.contains(MapFeatures::ImpassableForEnemies) {
        return Color::Rgb(60, 55, 45);
    }
    if f.contains(MapFeatures::Water)  { return Color::Rgb(38, 90, 179); }
    if f.contains(MapFeatures::Mud)    { return Color::Rgb(115, 82, 46); }
    if f.contains(MapFeatures::Snow)   { return Color::Rgb(209, 224, 235); }
    if f.contains(MapFeatures::Rock)   { return Color::Rgb(97, 89, 77); }
    if f.contains(MapFeatures::Grass)  { return Color::Rgb(56, 140, 56); }
    Color::Rgb(80, 80, 80) // Floor
}

fn tile_label(raw: u64) -> &'static str {
    if raw == 0 { return "void"; }
    let f = BitFlags::<MapFeatures>::from_bits_truncate(raw);
    if f.contains(MapFeatures::PlayerSpawn)  { return "player-spawn"; }
    if f.contains(MapFeatures::EnemySpawn)   { return "enemy-spawn"; }
    if f.contains(MapFeatures::EnemyExit)    { return "enemy-exit"; }
    if f.contains(MapFeatures::ImpassableForPlayers) { return "impassable"; }
    if f.contains(MapFeatures::Water)  { return "water"; }
    if f.contains(MapFeatures::Mud)    { return "mud"; }
    if f.contains(MapFeatures::Snow)   { return "snow"; }
    if f.contains(MapFeatures::Rock)   { return "rock"; }
    if f.contains(MapFeatures::Grass)  { return "grass"; }
    "floor"
}

pub fn draw(frame: &mut Frame, app: &App) {
    if app.mode == Mode::WaveEditor {
        draw_wave_editor(frame, app);
        return;
    }

    let area = frame.area();
    let status_height = 4u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(1),
            Constraint::Length(status_height),
        ])
        .split(area);

    draw_map(frame, app, chunks[0]);
    draw_status(frame, app, chunks[1]);
}

fn draw_map(frame: &mut Frame, app: &App, area: Rect) {
    // Each cell is 2 chars wide, 1 char tall
    let cell_w: usize = 2;
    let viewport_cols = (area.width as usize) / cell_w;
    let viewport_rows = area.height as usize;

    let col_off = app.viewport.0;
    let row_off = app.viewport.1;

    let mut lines: Vec<Line> = Vec::new();
    for row in row_off..(row_off + viewport_rows).min(app.map_height()) {
        let mut spans: Vec<Span> = Vec::new();
        for col in col_off..(col_off + viewport_cols).min(app.map_width()) {
            let tile = app.map.tiles[row][col];
            let is_cursor = app.cursor == (col, row);
            let bg = tile_color(tile);
            let style = if is_cursor {
                Style::default()
                    .bg(Color::White)
                    .fg(Color::Black)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().bg(bg).fg(bg)
            };
            spans.push(Span::styled("  ", style));
        }
        lines.push(Line::from(spans));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" AVS Map Editor ");
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(Paragraph::new(lines), inner);
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let file_str = app.file_path.as_deref().unwrap_or("<unsaved>");
    let dirty = if app.dirty { " *" } else { "" };
    let mode_str = match app.mode {
        Mode::Normal  => "NORMAL",
        Mode::Alt     => "ALT",
        Mode::Paint   => "PAINT",
        Mode::Command => "COMMAND",
        Mode::WaveEditor => "WAVES",
    };
    let (cur_col, cur_row) = app.cursor;
    let w = app.map_width();
    let h = app.map_height();
    let cur_tile = if cur_row < h && cur_col < w {
        tile_label(app.map.tiles[cur_row][cur_col])
    } else {
        "?"
    };

    let hints = match &app.prompt {
        Some(p) => format!("{}: {}_", p.label, p.input),
        None => match app.mode {
            Mode::Normal  => "f:floor  s:spawn  g:goal  p:player  .:void  Del:erase  Alt:paint-mode  Ctrl:command".to_string(),
            Mode::Alt     => "f/s/g/p: lock tile + enter paint mode  |  Alt: back to normal".to_string(),
            Mode::Paint   => format!("painting: {}  |  arrows: move+paint  |  Alt: exit paint", tile_label(app.paint_tile)),
            Mode::Command => "s:save  l:load  n:new  w:waves  q:quit  |  Esc: back".to_string(),
            Mode::WaveEditor => "".to_string(),
        },
    };

    let lines = vec![
        Line::from(vec![
            Span::styled(format!(" Mode: {mode_str:<8}"), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!("  File: {file_str}{dirty}")),
        ]),
        Line::from(format!(" Cursor: ({cur_col}, {cur_row})  Size: {w}x{h}  Tile: {cur_tile}")),
        Line::from(format!(" {hints}")),
    ];

    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        area,
    );
}

fn draw_wave_editor(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let status_height = 4u16;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(status_height)])
        .split(area);

    // Wave table
    let header = Row::new(vec![
        Cell::from("  #").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Enemy Def").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Count").style(Style::default().add_modifier(Modifier::BOLD)),
        Cell::from("Rate/min").style(Style::default().add_modifier(Modifier::BOLD)),
    ]).height(1);

    let rows: Vec<Row> = app.map.waves.iter().enumerate().map(|(i, w)| {
        let marker = if i == app.wave_selected { "> " } else { "  " };
        let style = if i == app.wave_selected {
            Style::default().bg(Color::DarkGray).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        Row::new(vec![
            Cell::from(format!("{marker}{i}")),
            Cell::from(w.enemy_def.as_str()),
            Cell::from(format!("{}", w.count)),
            Cell::from(format!("{:.1}", w.spawn_rate_per_minute)),
        ]).style(style)
    }).collect();

    let table = Table::new(rows, [
        Constraint::Length(4),
        Constraint::Min(30),
        Constraint::Length(7),
        Constraint::Length(9),
    ])
    .header(header)
    .block(Block::default().borders(Borders::ALL).title(" Wave Editor "));
    frame.render_widget(table, chunks[0]);

    // Status bar
    let dirty = if app.dirty { " *" } else { "" };
    let file_str = app.file_path.as_deref().unwrap_or("<unsaved>");
    let hints = match &app.prompt {
        Some(p) => format!("{}: {}_", p.label, p.input),
        None => "a:add  d:delete  e:edit  |  Esc: back to map".to_string(),
    };
    let lines = vec![
        Line::from(vec![
            Span::styled(" Mode: WAVES   ", Style::default().add_modifier(Modifier::BOLD)),
            Span::raw(format!(" File: {file_str}{dirty}")),
        ]),
        Line::from(format!(" Waves: {}   Selected: {}", app.map.waves.len(), app.wave_selected)),
        Line::from(format!(" {hints}")),
    ];
    frame.render_widget(
        Paragraph::new(lines).block(Block::default().borders(Borders::TOP)),
        chunks[1],
    );
}
