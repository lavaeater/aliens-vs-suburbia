# Poly Pizza Browser — Implementation Roadmap

A standalone screen reachable from the main menu that lets you search and browse Poly Pizza models,
preview them in a 3D viewer, apply shaders (Wind Waker etc.), and download them to disk.

---

## Architecture overview

```
GameState::Menu  ──"Browse Models"──►  GameState::PolyPizza
                                            │
                         ┌──────────────────┼──────────────────┐
                         ▼                  ▼                  ▼
                    SearchPanel        ResultsGrid         ViewerPanel
                  (keyword, filters)  (scrollable list)  (3D scene + controls)
```

New game state: `GameState::PolyPizza`. Everything lives in `src/poly_pizza/`.
The viewer uses a dedicated `Camera3d` (order 0) that is only alive in this state.
The UI sits on the existing `Camera2d` (order 1).

HTTP is done off the main thread via `std::thread::spawn` + Bevy channels
(`crossbeam_channel` or `std::sync::mpsc`), so the game loop never blocks.
GLB files are saved to `assets/poly_pizza_cache/<id>.glb` and loaded through
Bevy's normal `AssetServer` — so they survive between sessions.

---

## Step 1 — Crates and .env

Add to `Cargo.toml`:

```toml
ureq = { version = "2", features = ["json"] }   # blocking HTTP, no async needed
serde = { version = "1", features = ["derive"] }
serde_json = "1"
dotenvy = "0.15"                                 # reads .env at startup
```

Load the API key once at startup (in `main.rs` or `AssetsPlugin`):

```rust
dotenvy::dotenv().ok();
let api_key = std::env::var("POLY_PIZZA_API_KEY").expect("POLY_PIZZA_API_KEY not set");
```

Store it as a `Resource`:

```rust
#[derive(Resource)]
pub struct PolyPizzaConfig { pub api_key: String }
```

---

## Step 2 — Data types (`src/poly_pizza/types.rs`)

Rust structs matching the API schema. Field names use `#[serde(rename)]` because
the API uses PascalCase with spaces ("Tri Count", "DPURL"):

```rust
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PizzaModel {
    #[serde(rename = "ID")]    pub id: String,
    #[serde(rename = "Title")] pub title: String,
    #[serde(rename = "Download")] pub download_url: String,
    #[serde(rename = "Thumbnail")] pub thumbnail_url: String,
    #[serde(rename = "Tri Count")] pub tri_count: u32,
    #[serde(rename = "Animated")] pub animated: bool,
    #[serde(rename = "Licence")] pub licence: String,
    #[serde(rename = "Creator")] pub creator: Creator,
    #[serde(rename = "Attribution")] pub attribution: String,
    #[serde(rename = "Category")] pub category: Option<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Creator {
    #[serde(rename = "Username")] pub username: String,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct SearchResponse {
    pub total: u32,
    pub results: Vec<PizzaModel>,
}
```

---

## Step 3 — HTTP client (`src/poly_pizza/client.rs`)

Pure Rust functions (no Bevy) that call the API synchronously.
These run inside `std::thread::spawn`, never on the main thread.

```rust
const BASE: &str = "https://api.poly.pizza/v1.1";

pub fn search(api_key: &str, keyword: &str, page: u32, filters: &SearchFilters)
    -> Result<SearchResponse, Box<dyn std::error::Error + Send + Sync>>

pub fn search_filters(api_key: &str, page: u32, filters: &SearchFilters)
    -> Result<SearchResponse, Box<dyn std::error::Error + Send + Sync>>

pub fn get_model(api_key: &str, id: &str)
    -> Result<PizzaModel, Box<dyn std::error::Error + Send + Sync>>

pub fn get_list(api_key: &str, list_id: &str)
    -> Result<ListResponse, Box<dyn std::error::Error + Send + Sync>>

pub fn get_user(api_key: &str, username: &str)
    -> Result<UserResponse, Box<dyn std::error::Error + Send + Sync>>

pub fn download_glb(url: &str, dest: &Path)
    -> Result<(), Box<dyn std::error::Error + Send + Sync>>
```

All functions set `x-auth-token: <api_key>` header via `ureq`.

---

## Step 4 — Async bridge (`src/poly_pizza/async_bridge.rs`)

Bevy doesn't have async HTTP built in. Bridge pattern:

```rust
// Requests sent from main thread to worker thread
pub enum ApiRequest {
    Search { keyword: String, page: u32, filters: SearchFilters },
    SearchFilters { page: u32, filters: SearchFilters },
    GetList(String),
    GetUser(String),
    DownloadGlb { id: String, url: String },
}

// Responses sent back to main thread
pub enum ApiResponse {
    SearchResults(SearchResponse),
    ListResults(ListResponse),
    UserResults(UserResponse),
    DownloadComplete { id: String, path: PathBuf },
    Error(String),
}

#[derive(Resource)]
pub struct ApiChannels {
    pub sender: Sender<ApiRequest>,
    pub receiver: Receiver<ApiResponse>,
}
```

A background thread loops on the request receiver, calls the appropriate
`client::*` function, and sends the response back. Started once in the plugin's
`build()`.

---

## Step 5 — Game state

Add `PolyPizza` variant to `GameState` in `src/game_state/mod.rs`:

```rust
#[derive(States, Default, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameState {
    #[default] Menu,
    InGame,
    PolyPizza,
}
```

Add a "Browse Models" button to the main menu (`spawn_ui.rs`) that sends
`GotoState(GameState::PolyPizza)`.

---

## Step 6 — UI screen (`src/poly_pizza/ui.rs`)

Layout (all Bevy UI nodes, same `lava_ui_builder` + feathers style already used):

```
┌─────────────────────────────────────────────────┐
│  [← Back]   🍕 Poly Pizza Browser               │
├──────────────┬──────────────────────────────────┤
│ Search bar   │                                  │
│ [keyword   ] │  Results grid (4 cols)            │
│ Category ▾   │  ┌────┐ ┌────┐ ┌────┐ ┌────┐    │
│ Animated  ☐  │  │thumb│ │    │ │    │ │    │    │
│ CC0 only  ☐  │  │title│ │    │ │    │ │    │    │
│ [Search]     │  └────┘ └────┘ └────┘ └────┘    │
│              │  [< Prev]  page 1/3  [Next >]    │
│ ─────────── │                                  │
│ Browse list  ├──────────────────────────────────┤
│ [List ID   ] │  3D VIEWER                       │
│ [Go]         │  (model rotates, orbit from API) │
│              │  [Wind Waker ☐] [Flat ☐]         │
│ Browse user  │  [Download ↓]                    │
│ [Username  ] │  Attribution: …                  │
│ [Go]         │                                  │
└──────────────┴──────────────────────────────────┘
```

Components / resources:
- `PolyPizzaUiState` resource: current search query, page, selected model, pending request flag
- Marker components for each interactive widget so systems can query them

---

## Step 7 — Viewer (`src/poly_pizza/viewer.rs`)

When a model is selected from the results:

1. If `assets/poly_pizza_cache/<id>.glb` already exists → load it directly with `asset_server.load()`
2. Otherwise → send `ApiRequest::DownloadGlb` → on `DownloadComplete` → load it

Spawned as:
```rust
commands.spawn((
    SceneRoot(handle),
    Transform::IDENTITY,
    ViewerModel,   // marker for cleanup
    WindWakerShader or not, depending on toggle
));
```

A dedicated `Camera3d` with `order: 0` lives only in `GameState::PolyPizza`.
Mouse drag rotates the model (orbit camera). Scroll wheel zooms.
The API provides `Orbit { phi, theta, radius }` — use these as the initial camera position.

Shader toggles are checkboxes in the UI. Changing a toggle despawns and respawns
the model entity with/without the `WindWakerShader` component.

---

## Step 8 — Plugin wiring (`src/poly_pizza/plugin.rs`)

```rust
pub struct PolyPizzaPlugin;

impl Plugin for PolyPizzaPlugin {
    fn build(&self, app: &mut App) {
        app
            .init_resource::<PolyPizzaUiState>()
            .add_systems(OnEnter(GameState::PolyPizza), (spawn_polypizza_ui, spawn_viewer_camera))
            .add_systems(OnExit(GameState::PolyPizza), cleanup_polypizza)
            .add_systems(Update, (
                handle_search_submit,
                handle_api_responses,     // drains the mpsc receiver each frame
                update_results_grid,
                handle_model_select,
                handle_download_button,
                orbit_viewer_camera,
                handle_shader_toggles,
                handle_back_button,
            ).run_if(in_state(GameState::PolyPizza)));
    }
}
```

Register in `GamePlugin` alongside the other plugins.

---

## Step 9 — Cache directory

On first download, create `assets/poly_pizza_cache/` if it doesn't exist.
Add `assets/poly_pizza_cache/` to `.gitignore` — downloaded models should not
be committed.

---

## Suggested build order

```
1. Crates + .env + PolyPizzaConfig resource        (Step 1)
2. Data types + client functions                    (Steps 2–3)
   → smoke-test with a cargo test that calls search("alien")
3. Async bridge + ApiChannels resource              (Step 4)
4. Add PolyPizza game state + Back button on menu   (Step 5)
5. Bare-bones UI screen (just search bar + list)    (Step 6, first pass)
6. Wire search → API → results grid                 (Step 8 partial)
7. Viewer camera + model loading                    (Step 7)
8. Shader toggles                                   (Step 7 cont.)
9. Download button + cache                          (Steps 7, 9)
10. Browse-by-list and browse-by-user panels        (Step 6 cont.)
11. Pagination (Prev/Next)
12. Polish: loading spinner, error messages, attribution display
```

---

## Key design decisions and tradeoffs

**Blocking HTTP on a background thread** — `ureq` is synchronous, which keeps
the dependency tree small and avoids `tokio`. The tradeoff is that you need the
channel bridge (Step 4). If you later want concurrent requests, `rayon` or a
thread pool work fine with this pattern.

**GLB cache on disk** — Avoids re-downloading on every session and lets Bevy's
`AssetServer` handle hot-reload. Tradeoff: disk grows unbounded; a simple LRU
eviction (e.g. delete oldest 50 files if cache exceeds 200 MB) can be added later.

**No thumbnail images in the grid** — Bevy UI can display `Image` handles but
loading WEBP thumbnails requires spawning another download per result (32 images
per page). First pass: text-only grid with title + creator + tri count. Add
thumbnails as a polish step once the basic flow works — the `UiImage` approach
is the same as the GLB download pattern.

**Shader toggles despawn/respawn the model** — Simpler than mutating the material
in place (which requires finding the right extended material handle). The respawn
is fast since the GLB is already in Bevy's asset cache.
