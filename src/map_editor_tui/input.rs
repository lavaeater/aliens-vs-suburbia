use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use enumflags2::BitFlags;
use crate::map::{key_to_feature, MapFeatures};
use super::app::{App, Mode, Prompt, PromptKind, TILE_VOID};

pub enum Action {
    Quit,
    Continue,
}

/// Returns the visible viewport size from outside so move_cursor can scroll correctly.
/// Caller passes (viewport_cols, viewport_rows).
pub fn handle_key(app: &mut App, key: KeyEvent, viewport: (usize, usize)) -> Action {
    // Prompt active — handle text input for all modes
    if app.prompt.is_some() {
        return handle_prompt(app, key);
    }

    match app.mode {
        Mode::Normal  => handle_normal(app, key, viewport),
        Mode::Alt     => handle_alt(app, key, viewport),
        Mode::Paint   => handle_paint(app, key, viewport),
        Mode::Command => handle_command(app, key),
        Mode::WaveEditor => handle_wave(app, key, viewport),
    }
}

fn handle_normal(app: &mut App, key: KeyEvent, vp: (usize, usize)) -> Action {
    match (key.modifiers, key.code) {
        (KeyModifiers::CONTROL, _) => { app.mode = Mode::Command; }
        (KeyModifiers::ALT, _)     => { app.mode = Mode::Alt; }
        (_, KeyCode::Up)    => app.move_cursor(0, -1, vp.0, vp.1),
        (_, KeyCode::Down)  => app.move_cursor(0,  1, vp.0, vp.1),
        (_, KeyCode::Left)  => app.move_cursor(-1, 0, vp.0, vp.1),
        (_, KeyCode::Right) => app.move_cursor( 1, 0, vp.0, vp.1),
        (_, code) => {
            let current = BitFlags::<MapFeatures>::from_bits_truncate(
                app.map.tiles.get(app.cursor.1).and_then(|r| r.get(app.cursor.0)).copied().unwrap_or(0)
            );
            let next = key_to_feature(&code, current).bits();
            // Only paint if key_to_feature actually changed something (i.e. it was a recognised key)
            if next != current.bits() || matches!(code, KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('0')) {
                app.paint(next);
            }
        }
    }
    Action::Continue
}

fn handle_alt(app: &mut App, key: KeyEvent, _vp: (usize, usize)) -> Action {
    // Any key that key_to_feature recognises as a tile key locks that tile in paint mode.
    let dummy = BitFlags::<MapFeatures>::default();
    let result = key_to_feature(&key.code, dummy);
    if result != dummy || matches!(key.code, KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('0')) {
        app.paint_tile = result.bits();
        app.mode = Mode::Paint;
    } else {
        app.mode = Mode::Normal;
    }
    Action::Continue
}

fn handle_paint(app: &mut App, key: KeyEvent, vp: (usize, usize)) -> Action {
    match (key.modifiers, key.code) {
        (KeyModifiers::ALT, _) => { app.mode = Mode::Normal; }
        (_, KeyCode::Up) => {
            app.move_cursor(0, -1, vp.0, vp.1);
            app.paint(app.paint_tile);
        }
        (_, KeyCode::Down) => {
            app.move_cursor(0, 1, vp.0, vp.1);
            app.paint(app.paint_tile);
        }
        (_, KeyCode::Left) => {
            app.move_cursor(-1, 0, vp.0, vp.1);
            app.paint(app.paint_tile);
        }
        (_, KeyCode::Right) => {
            app.move_cursor(1, 0, vp.0, vp.1);
            app.paint(app.paint_tile);
        }
        _ => {}
    }
    Action::Continue
}

fn handle_command(app: &mut App, key: KeyEvent) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') if key.modifiers == KeyModifiers::CONTROL => {
            app.mode = Mode::Normal;
        }
        KeyCode::Esc => { app.mode = Mode::Normal; }
        KeyCode::Char('s') => {
            if app.file_path.is_some() {
                if let Err(e) = app.save() {
                    app.status_msg = Some(format!("Save error: {e}"));
                }
                app.mode = Mode::Normal;
            } else {
                app.prompt = Some(Prompt { kind: PromptKind::SavePath, input: String::new(), label: "Save to" });
            }
        }
        KeyCode::Char('l') => {
            app.prompt = Some(Prompt { kind: PromptKind::LoadPath, input: String::new(), label: "Load file" });
        }
        KeyCode::Char('n') => {
            app.prompt = Some(Prompt { kind: PromptKind::NewWidth, input: String::new(), label: "New map width" });
        }
        KeyCode::Char('w') => {
            app.mode = Mode::WaveEditor;
        }
        KeyCode::Char('q') => {
            if app.dirty {
                app.prompt = Some(Prompt { kind: PromptKind::ConfirmQuit, input: String::new(), label: "Unsaved changes. Quit? (y/n)" });
            } else {
                return Action::Quit;
            }
        }
        _ => {}
    }
    Action::Continue
}

fn handle_wave(app: &mut App, key: KeyEvent, _vp: (usize, usize)) -> Action {
    match key.code {
        KeyCode::Esc => { app.mode = Mode::Normal; }
        KeyCode::Up => {
            if app.wave_selected > 0 { app.wave_selected -= 1; }
        }
        KeyCode::Down => {
            if app.wave_selected + 1 < app.map.waves.len() { app.wave_selected += 1; }
        }
        KeyCode::Char('d') => { app.delete_wave(); }
        KeyCode::Char('a') => {
            app.new_wave_scratch = (String::new(), String::new(), String::new());
            app.prompt = Some(Prompt { kind: PromptKind::WaveEnemyDef, input: String::new(), label: "Enemy def path" });
        }
        KeyCode::Char('e') => {
            if !app.map.waves.is_empty() {
                let idx = app.wave_selected;
                let current = app.map.waves[idx].enemy_def.clone();
                app.prompt = Some(Prompt {
                    kind: PromptKind::WaveEditEnemyDef(idx),
                    input: current,
                    label: "Edit enemy def",
                });
            }
        }
        _ => {}
    }
    Action::Continue
}

fn handle_prompt(app: &mut App, key: KeyEvent) -> Action {
    let prompt = app.prompt.as_mut().unwrap();
    match key.code {
        KeyCode::Esc => {
            app.prompt = None;
        }
        KeyCode::Backspace => {
            prompt.input.pop();
        }
        KeyCode::Char(c) => {
            prompt.input.push(c);
        }
        KeyCode::Enter => {
            let input = prompt.input.clone();
            let kind = prompt.kind.clone();
            app.prompt = None;
            handle_prompt_commit(app, kind, input);
        }
        _ => {}
    }
    Action::Continue
}

fn handle_prompt_commit(app: &mut App, kind: PromptKind, input: String) {
    match kind {
        PromptKind::SavePath => {
            app.file_path = Some(input);
            if let Err(e) = app.save() {
                app.status_msg = Some(format!("Save error: {e}"));
            }
            app.mode = Mode::Normal;
        }
        PromptKind::LoadPath => {
            if let Err(e) = app.load(&input) {
                app.status_msg = Some(format!("Load error: {e}"));
            }
            app.mode = Mode::Normal;
        }
        PromptKind::NewWidth => {
            let w = input.parse::<usize>().unwrap_or(20);
            app.new_wave_scratch.0 = w.to_string();
            app.prompt = Some(Prompt { kind: PromptKind::NewHeight, input: String::new(), label: "New map height" });
        }
        PromptKind::NewHeight => {
            let w = app.new_wave_scratch.0.parse::<usize>().unwrap_or(20);
            let h = input.parse::<usize>().unwrap_or(20);
            app.new_map(w, h);
            app.mode = Mode::Normal;
        }
        PromptKind::ConfirmQuit => {
            if input.to_lowercase().starts_with('y') {
                // caller will see Quit next tick — set a flag via status_msg trick
                // instead, we just forcibly quit by clearing dirty
                app.dirty = false;
                app.status_msg = Some("__quit__".to_string());
            } else {
                app.mode = Mode::Normal;
            }
        }
        PromptKind::WaveEnemyDef => {
            app.new_wave_scratch.0 = input;
            app.prompt = Some(Prompt { kind: PromptKind::WaveCount, input: String::new(), label: "Count" });
        }
        PromptKind::WaveCount => {
            app.new_wave_scratch.1 = input;
            app.prompt = Some(Prompt { kind: PromptKind::WaveSpawnRate, input: String::new(), label: "Spawn rate/min" });
        }
        PromptKind::WaveSpawnRate => {
            app.new_wave_scratch.2 = input;
            app.commit_new_wave();
        }
        PromptKind::WaveEditEnemyDef(idx) => {
            app.commit_wave_edit(idx, 0, &input);
            app.prompt = Some(Prompt {
                kind: PromptKind::WaveEditCount(idx),
                input: app.map.waves.get(idx).map(|w| w.count.to_string()).unwrap_or_default(),
                label: "Edit count",
            });
        }
        PromptKind::WaveEditCount(idx) => {
            app.commit_wave_edit(idx, 1, &input);
            app.prompt = Some(Prompt {
                kind: PromptKind::WaveEditSpawnRate(idx),
                input: app.map.waves.get(idx).map(|w| format!("{:.1}", w.spawn_rate_per_minute)).unwrap_or_default(),
                label: "Edit spawn rate/min",
            });
        }
        PromptKind::WaveEditSpawnRate(idx) => {
            app.commit_wave_edit(idx, 2, &input);
        }
    }
}
