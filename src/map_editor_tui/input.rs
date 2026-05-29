use crossterm::event::{KeyCode, KeyEvent};
use super::app::{App, Mode, Prompt, PromptKind};
use super::commands::{CmdResult, alt_map, command_map, normal_map, paint_map, wave_map};

pub enum Action {
    Quit,
    Continue,
}

pub fn handle_key(app: &mut App, key: KeyEvent, viewport: (usize, usize)) -> Action {
    if app.prompt.is_some() {
        return handle_prompt(app, key);
    }

    let result = match app.mode {
        Mode::Normal     => normal_map().execute(app, key, viewport),
        Mode::Alt        => alt_map().execute(app, key, viewport),
        Mode::Paint      => paint_map().execute(app, key, viewport),
        Mode::Command    => command_map().execute(app, key, viewport),
        Mode::WaveEditor => wave_map().execute(app, key, viewport),
    };

    // Fallback: any unhandled modifier-only event in Normal/Alt switches mode
    if result.is_none() {
        match app.mode {
            Mode::Normal => {
                if key.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                    app.mode = Mode::Command;
                } else if key.modifiers.contains(crossterm::event::KeyModifiers::ALT) {
                    app.mode = Mode::Alt;
                }
            }
            Mode::Alt => { app.mode = Mode::Normal; }
            _ => {}
        }
    }

    match result {
        Some(CmdResult::Quit) => Action::Quit,
        _ => Action::Continue,
    }
}

fn handle_prompt(app: &mut App, key: KeyEvent) -> Action {
    let prompt = app.prompt.as_mut().unwrap();
    match key.code {
        KeyCode::Esc => { app.prompt = None; }
        KeyCode::Backspace => { prompt.input.pop(); }
        KeyCode::Char(c) => { prompt.input.push(c); }
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
            if let Err(e) = app.save() { app.status_msg = Some(format!("Save error: {e}")); }
            app.mode = Mode::Normal;
        }
        PromptKind::LoadPath => {
            if let Err(e) = app.load(&input) { app.status_msg = Some(format!("Load error: {e}")); }
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
