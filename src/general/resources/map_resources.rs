use std::collections::{HashSet};
use bevy::prelude::Resource;
use pathfinding::grid::Grid;

#[derive(Resource)]
pub struct MapGraph {
    pub path_finding_grid: Grid,
    pub occupied_tiles: HashSet<(usize, usize)>,
    pub goal: (usize, usize),
    /// Set to true when a tile is re-opened so the recheck system can clear stale MustDestroyTheMap.
    pub path_reopened: bool,
}