use crate::ai::flow_field::{FlowField, Tile};
use bevy::prelude::Resource;
use pathfinding::grid::Grid;
use std::collections::HashSet;

#[derive(Resource)]
pub struct MapGraph {
    /// Change it through [`Self::open`] and [`Self::close`], so `flow` follows. A change
    /// undone before anyone reads the field may go straight to the grid.
    pub path_finding_grid: Grid,
    pub occupied_tiles: HashSet<(usize, usize)>,
    pub goal: (usize, usize),
    /// Set to true when a tile is re-opened so the recheck system can clear stale MustDestroyTheMap.
    pub path_reopened: bool,
    /// The way to `goal` from everywhere. `refresh_flow` brings it up to date.
    pub flow: FlowField,
}

impl MapGraph {
    pub fn open(&mut self, tile: Tile) {
        self.path_finding_grid.add_vertex(tile);
        self.flow.opened(&self.path_finding_grid, tile);
    }

    pub fn close(&mut self, tile: Tile) {
        self.path_finding_grid.remove_vertex(tile);
        self.flow.closed(&self.path_finding_grid, tile);
    }

    /// Build the field if a new map has made it stale. There is no change detection to
    /// lean on: `update_current_tile_system` writes this resource every frame.
    pub fn refresh_flow(&mut self) {
        if !self.flow.fresh {
            self.flow = FlowField::build(&self.path_finding_grid, self.goal);
        }
    }
}
