use bevy::prelude::Resource;
use pathfinding::grid::Grid;

#[derive(Resource)]
pub struct MapGraph {
    pub grid: Grid,
    pub goal: (usize, usize)
}