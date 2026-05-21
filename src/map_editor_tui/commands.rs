use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use enumflags2::BitFlags;
use crate::map::{key_to_feature, MapFeatures};
use super::app::{App, Mode, Prompt, PromptKind};

pub enum CmdResult {
    Continue,
    Quit,
}

pub struct Cmd {
    pub key: KeyCode,
    pub mods: KeyModifiers,
    pub desc: &'static str,
    action: Box<dyn Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync>,
}

pub struct CommandMap {
    pub name: &'static str,
    cmds: Vec<Cmd>,
}

impl CommandMap {
    pub fn new(name: &'static str) -> Self {
        Self { name, cmds: vec![] }
    }

    pub fn add(
        mut self,
        key: KeyCode,
        mods: KeyModifiers,
        desc: &'static str,
        action: impl Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync + 'static,
    ) -> Self {
        self.cmds.push(Cmd { key, mods, desc, action: Box::new(action) });
        self
    }

    pub fn k(
        self,
        key: KeyCode,
        desc: &'static str,
        action: impl Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync + 'static,
    ) -> Self {
        self.add(key, KeyModifiers::NONE, desc, action)
    }

    pub fn ctrl(
        self,
        key: KeyCode,
        desc: &'static str,
        action: impl Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync + 'static,
    ) -> Self {
        self.add(key, KeyModifiers::CONTROL, desc, action)
    }

    pub fn alt(
        self,
        key: KeyCode,
        desc: &'static str,
        action: impl Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync + 'static,
    ) -> Self {
        self.add(key, KeyModifiers::ALT, desc, action)
    }

    /// Try to execute the matching command. Returns Some(result) if handled, None if no match.
    pub fn execute(&self, app: &mut App, event: KeyEvent, vp: (usize, usize)) -> Option<CmdResult> {
        for cmd in &self.cmds {
            if cmd.key == event.code && cmd.mods == event.modifiers {
                return Some((cmd.action)(app, vp));
            }
        }
        None
    }

    pub fn hints(&self) -> String {
        self.cmds.iter()
            .filter(|c| !c.desc.is_empty())
            .map(|c| format!("{}:{}", key_display(c.key, c.mods), c.desc))
            .collect::<Vec<_>>()
            .join("  ")
    }
}

fn key_display(key: KeyCode, mods: KeyModifiers) -> String {
    let prefix = if mods.contains(KeyModifiers::CONTROL) { "Ctrl+" }
        else if mods.contains(KeyModifiers::ALT) { "Alt+" }
        else { "" };
    let k = match key {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Up       => "Up".to_string(),
        KeyCode::Down     => "Down".to_string(),
        KeyCode::Left     => "Left".to_string(),
        KeyCode::Right    => "Right".to_string(),
        KeyCode::Enter    => "Enter".to_string(),
        KeyCode::Esc      => "Esc".to_string(),
        KeyCode::Delete   => "Del".to_string(),
        KeyCode::Backspace => "Bksp".to_string(),
        _                 => "?".to_string(),
    };
    format!("{prefix}{k}")
}

// --- helpers shared by map builders ---

fn paint_via_key(code: KeyCode) -> impl Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync {
    move |app: &mut App, _vp| {
        let current = BitFlags::<MapFeatures>::from_bits_truncate(
            app.map.tiles.get(app.cursor.1)
                .and_then(|r| r.get(app.cursor.0))
                .copied()
                .unwrap_or(0),
        );
        let next = key_to_feature(&code, current).bits();
        app.paint(next);
        CmdResult::Continue
    }
}

fn lock_paint_via_key(code: KeyCode) -> impl Fn(&mut App, (usize, usize)) -> CmdResult + Send + Sync {
    move |app: &mut App, _vp| {
        let dummy = BitFlags::<MapFeatures>::default();
        app.paint_tile = key_to_feature(&code, dummy).bits();
        app.mode = Mode::Paint;
        CmdResult::Continue
    }
}

// --- per-mode map builders ---

fn tile_keys(map: CommandMap, paint_fn: fn(KeyCode) -> Box<dyn Fn(&mut App, (usize,usize)) -> CmdResult + Send + Sync>) -> CommandMap {
    map
        .add(KeyCode::Char('f'), KeyModifiers::NONE, "floor",          paint_fn(KeyCode::Char('f')))
        .add(KeyCode::Char('g'), KeyModifiers::NONE, "grass",          paint_fn(KeyCode::Char('g')))
        .add(KeyCode::Char('w'), KeyModifiers::NONE, "water",          paint_fn(KeyCode::Char('w')))
        .add(KeyCode::Char('m'), KeyModifiers::NONE, "mud",            paint_fn(KeyCode::Char('m')))
        .add(KeyCode::Char('s'), KeyModifiers::NONE, "snow",           paint_fn(KeyCode::Char('s')))
        .add(KeyCode::Char('r'), KeyModifiers::NONE, "rock",           paint_fn(KeyCode::Char('r')))
        .add(KeyCode::Char('i'), KeyModifiers::NONE, "toggle-impass",  paint_fn(KeyCode::Char('i')))
        .add(KeyCode::Char('e'), KeyModifiers::NONE, "toggle-enemy-wall", paint_fn(KeyCode::Char('e')))
        .add(KeyCode::Char('p'), KeyModifiers::NONE, "player-spawn",   paint_fn(KeyCode::Char('p')))
        .add(KeyCode::Char('z'), KeyModifiers::NONE, "enemy-spawn",    paint_fn(KeyCode::Char('z')))
        .add(KeyCode::Char('x'), KeyModifiers::NONE, "enemy-exit",     paint_fn(KeyCode::Char('x')))
        .add(KeyCode::Delete,    KeyModifiers::NONE, "void",           paint_fn(KeyCode::Delete))
        .add(KeyCode::Backspace, KeyModifiers::NONE, "",               paint_fn(KeyCode::Backspace))
}

pub fn normal_map() -> CommandMap {
    fn wrap_paint(code: KeyCode) -> Box<dyn Fn(&mut App, (usize,usize)) -> CmdResult + Send + Sync> {
        Box::new(paint_via_key(code))
    }
    let base = CommandMap::new("Normal")
        .k(KeyCode::Up,    "", |app, vp| { app.move_cursor(0, -1, vp.0, vp.1); CmdResult::Continue })
        .k(KeyCode::Down,  "", |app, vp| { app.move_cursor(0,  1, vp.0, vp.1); CmdResult::Continue })
        .k(KeyCode::Left,  "", |app, vp| { app.move_cursor(-1, 0, vp.0, vp.1); CmdResult::Continue })
        .k(KeyCode::Right, "", |app, vp| { app.move_cursor( 1, 0, vp.0, vp.1); CmdResult::Continue })
        .ctrl(KeyCode::Char('c'), "", |app, _| { app.mode = Mode::Command; CmdResult::Continue })
        .ctrl(KeyCode::Char('a'), "", |app, _| { app.mode = Mode::Alt;     CmdResult::Continue })
        .alt(KeyCode::Char('a'),  "", |app, _| { app.mode = Mode::Alt;     CmdResult::Continue });
    tile_keys(base, wrap_paint)
}

pub fn alt_map() -> CommandMap {
    fn wrap_lock(code: KeyCode) -> Box<dyn Fn(&mut App, (usize,usize)) -> CmdResult + Send + Sync> {
        Box::new(lock_paint_via_key(code))
    }
    let base = CommandMap::new("Alt")
        .alt(KeyCode::Char('a'), "back",  |app, _| { app.mode = Mode::Normal; CmdResult::Continue })
        .k(KeyCode::Esc,         "back",  |app, _| { app.mode = Mode::Normal; CmdResult::Continue });
    tile_keys(base, wrap_lock)
}

pub fn paint_map() -> CommandMap {
    CommandMap::new("Paint")
        .alt(KeyCode::Char('a'), "exit-paint", |app, _|    { app.mode = Mode::Normal; CmdResult::Continue })
        .k(KeyCode::Up,    "move+paint", |app, vp| { app.move_cursor(0, -1, vp.0, vp.1); app.paint(app.paint_tile); CmdResult::Continue })
        .k(KeyCode::Down,  "",           |app, vp| { app.move_cursor(0,  1, vp.0, vp.1); app.paint(app.paint_tile); CmdResult::Continue })
        .k(KeyCode::Left,  "",           |app, vp| { app.move_cursor(-1, 0, vp.0, vp.1); app.paint(app.paint_tile); CmdResult::Continue })
        .k(KeyCode::Right, "",           |app, vp| { app.move_cursor( 1, 0, vp.0, vp.1); app.paint(app.paint_tile); CmdResult::Continue })
}

pub fn command_map() -> CommandMap {
    CommandMap::new("Command")
        .k(KeyCode::Esc, "back",  |app, _| { app.mode = Mode::Normal; CmdResult::Continue })
        .k(KeyCode::Char('s'), "save", |app, _| {
            if app.file_path.is_some() {
                if let Err(e) = app.save() { app.status_msg = Some(format!("Save error: {e}")); }
                app.mode = Mode::Normal;
            } else {
                app.prompt = Some(Prompt { kind: PromptKind::SavePath, input: String::new(), label: "Save to" });
            }
            CmdResult::Continue
        })
        .k(KeyCode::Char('l'), "load", |app, _| {
            app.prompt = Some(Prompt { kind: PromptKind::LoadPath, input: String::new(), label: "Load file" });
            CmdResult::Continue
        })
        .k(KeyCode::Char('n'), "new", |app, _| {
            app.prompt = Some(Prompt { kind: PromptKind::NewWidth, input: String::new(), label: "New map width" });
            CmdResult::Continue
        })
        .k(KeyCode::Char('w'), "waves", |app, _| { app.mode = Mode::WaveEditor; CmdResult::Continue })
        .k(KeyCode::Char('q'), "quit", |app, _| {
            if app.dirty {
                app.prompt = Some(Prompt { kind: PromptKind::ConfirmQuit, input: String::new(), label: "Unsaved changes. Quit? (y/n)" });
                CmdResult::Continue
            } else {
                CmdResult::Quit
            }
        })
}

pub fn wave_map() -> CommandMap {
    CommandMap::new("Waves")
        .k(KeyCode::Esc,        "back to map", |app, _| { app.mode = Mode::Normal; CmdResult::Continue })
        .k(KeyCode::Up,         "",            |app, _| { if app.wave_selected > 0 { app.wave_selected -= 1; } CmdResult::Continue })
        .k(KeyCode::Down,       "",            |app, _| { if app.wave_selected + 1 < app.map.waves.len() { app.wave_selected += 1; } CmdResult::Continue })
        .k(KeyCode::Char('d'),  "delete",      |app, _| { app.delete_wave(); CmdResult::Continue })
        .k(KeyCode::Char('a'),  "add",         |app, _| {
            app.new_wave_scratch = (String::new(), String::new(), String::new());
            app.prompt = Some(Prompt { kind: PromptKind::WaveEnemyDef, input: String::new(), label: "Enemy def path" });
            CmdResult::Continue
        })
        .k(KeyCode::Char('e'),  "edit",        |app, _| {
            if !app.map.waves.is_empty() {
                let idx = app.wave_selected;
                let current = app.map.waves[idx].enemy_def.clone();
                app.prompt = Some(Prompt { kind: PromptKind::WaveEditEnemyDef(idx), input: current, label: "Edit enemy def" });
            }
            CmdResult::Continue
        })
}

/// Returns a hint string for modes that don't use a full CommandMap in the status bar.
pub fn hints_for(mode: &Mode, paint_tile_label: &str) -> String {
    match mode {
        Mode::Normal  => normal_map().hints(),
        Mode::Alt     => "pick tile key to lock into paint mode  |  Esc: back".to_string(),
        Mode::Paint   => format!("painting:{}  Alt:exit  arrows:move+paint", paint_tile_label),
        Mode::Command => command_map().hints(),
        Mode::WaveEditor => wave_map().hints(),
    }
}
