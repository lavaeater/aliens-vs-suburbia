use std::collections::{HashSet};
use bevy::prelude::Resource;
use pathfinding::grid::Grid;

#[derive(Resource)]
pub struct MapGraph {
    pub path_finding_grid: Grid,
    pub occupied_tiles: HashSet<(usize, usize)>,
    pub goal: (usize, usize)
}