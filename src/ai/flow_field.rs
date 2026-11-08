//! A flow field: how far every tile is from the goal, walked out once from the goal itself.
//!
//! Aliens all want the same place, so a search per alien repeats the same work: on an
//! 800 x 800 map A* took up to 38 ms each, all of it in the frame a wave spawns. This is one
//! breadth-first walk per change to the map, and after that an alien reads its next tile
//! from an array. A tile the walk never reached has no way to the goal.
//!
//! The walk itself is too slow to repeat for one tower (115 ms on that map in a debug
//! build), so it only runs when a map loads. [`FlowField::opened`] and
//! [`FlowField::closed`] repair what one tile changes, which on open ground is a few tiles.
//!
//! Steps match `Grid::neighbours`: four directions, each costing one. Pure, no ECS;
//! `MapGraph` keeps one and says when it's stale.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, VecDeque};

use pathfinding::grid::Grid;

/// A tile, `(col, row)`.
pub type Tile = (usize, usize);

const UNREACHED: u32 = u32::MAX;

/// What an alien standing on a tile should do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Arrived,
    Towards(Tile),
    /// The tile is closed, or walled off from the goal.
    NoWay,
}

#[derive(Debug, Default)]
pub struct FlowField {
    width: usize,
    height: usize,
    /// Steps to the goal per tile, row by row.
    steps: Vec<u32>,
    goal: Tile,
    /// Whether this was built from the grid as it is now.
    pub(crate) fresh: bool,
}

/// The tiles beside `(col, row)`. Off the left or top edge wraps to `usize::MAX`, which no
/// grid contains.
const fn beside((col, row): Tile) -> [Tile; 4] {
    [
        (col.wrapping_add(1), row),
        (col.wrapping_sub(1), row),
        (col, row.wrapping_add(1)),
        (col, row.wrapping_sub(1)),
    ]
}

impl FlowField {
    pub fn build(grid: &Grid, goal: Tile) -> Self {
        let mut field = Self {
            width: grid.width,
            height: grid.height,
            steps: vec![UNREACHED; grid.width * grid.height],
            goal,
            fresh: true,
        };
        if !grid.has_vertex(goal) {
            return field;
        }
        field.set(goal, 0);
        let mut frontier = vec![goal];
        let mut next = Vec::new();
        let mut distance = 0;
        while !frontier.is_empty() {
            distance += 1;
            for &tile in &frontier {
                for near in beside(tile) {
                    if grid.has_vertex(near) && field.steps(near).is_none() {
                        field.set(near, distance);
                        next.push(near);
                    }
                }
            }
            frontier.clear();
            std::mem::swap(&mut frontier, &mut next);
        }
        field
    }

    /// `tile`, now open in `grid`, may shorten the way for the tiles around it.
    pub fn opened(&mut self, grid: &Grid, tile: Tile) {
        if !self.fresh || self.steps(tile).is_some() {
            return;
        }
        if tile == self.goal {
            self.fresh = false;
            return;
        }
        let Some(from_here) = self.nearest_beside(tile) else {
            return;
        };
        self.set(tile, from_here + 1);

        // Everyone starts a step further than the last, so the first offer is the best.
        let mut wave = VecDeque::from([(tile, from_here + 1)]);
        while let Some((at, distance)) = wave.pop_front() {
            for near in beside(at) {
                if grid.has_vertex(near) && self.steps(near).is_none_or(|s| s > distance + 1) {
                    self.set(near, distance + 1);
                    wave.push_back((near, distance + 1));
                }
            }
        }
    }

    /// `tile`, now closed in `grid`, may have been the way to the goal for tiles behind it.
    pub fn closed(&mut self, grid: &Grid, tile: Tile) {
        if !self.fresh {
            return;
        }
        if tile == self.goal {
            self.fresh = false;
            return;
        }
        let Some(distance) = self.steps(tile) else {
            return;
        };
        let orphans = self.orphans_of(tile, distance);

        // The tiles still standing offer their distance inward, nearest first.
        let mut offers: BinaryHeap<Reverse<(u32, Tile)>> = orphans
            .iter()
            .filter(|orphan| grid.has_vertex(**orphan))
            .filter_map(|orphan| {
                let from_here = self.nearest_beside(*orphan)?;
                Some(Reverse((from_here + 1, *orphan)))
            })
            .collect();
        while let Some(Reverse((distance, at))) = offers.pop() {
            if self.steps(at).is_some() {
                continue;
            }
            self.set(at, distance);
            for near in beside(at) {
                if grid.has_vertex(near) && self.steps(near).is_none() {
                    offers.push(Reverse((distance + 1, near)));
                }
            }
        }
    }

    /// Unset `tile` and every tile whose only way to the goal led through it, returning
    /// them. Runs by rising distance, so a tile is judged once all that lay nearer is.
    fn orphans_of(&mut self, tile: Tile, distance: u32) -> Vec<Tile> {
        self.set(tile, UNREACHED);
        let mut orphans = vec![tile];
        let mut queue = VecDeque::from([(tile, distance)]);
        while let Some((at, distance)) = queue.pop_front() {
            for near in beside(at) {
                let still_led = beside(near)
                    .into_iter()
                    .any(|other| self.steps(other) == Some(distance));
                if self.steps(near) == Some(distance + 1) && !still_led {
                    self.set(near, UNREACHED);
                    orphans.push(near);
                    queue.push_back((near, distance + 1));
                }
            }
        }
        orphans
    }

    fn nearest_beside(&self, tile: Tile) -> Option<u32> {
        beside(tile).into_iter().filter_map(|near| self.steps(near)).min()
    }

    const fn index(&self, (col, row): Tile) -> Option<usize> {
        if col < self.width && row < self.height {
            Some(row * self.width + col)
        } else {
            None
        }
    }

    fn set(&mut self, tile: Tile, distance: u32) {
        if let Some(index) = self.index(tile)
            && let Some(steps) = self.steps.get_mut(index)
        {
            *steps = distance;
        }
    }

    /// How many steps `tile` is from the goal. `None` when there is no way there.
    pub fn steps(&self, tile: Tile) -> Option<u32> {
        let steps = *self.steps.get(self.index(tile)?)?;
        (steps != UNREACHED).then_some(steps)
    }

    /// Where to go from `tile`. Of the tiles equally close, the one nearest the goal as the
    /// crow flies, so aliens cross open ground diagonally instead of along its edges.
    pub fn step_from(&self, tile: Tile) -> Step {
        let Some(here) = self.steps(tile) else {
            return Step::NoWay;
        };
        if here == 0 {
            return Step::Arrived;
        }
        beside(tile)
            .into_iter()
            .filter(|near| self.steps(*near) == Some(here - 1))
            .min_by_key(|near| {
                let (dx, dy) = (near.0.abs_diff(self.goal.0), near.1.abs_diff(self.goal.1));
                dx * dx + dy * dy
            })
            .map_or(Step::NoWay, Step::Towards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pathfinding::directed::astar::astar;

    fn open_grid(width: usize, height: usize) -> Grid {
        let mut grid = Grid::new(width, height);
        grid.fill();
        grid
    }

    #[test]
    fn it_counts_the_steps_around_a_wall() {
        let mut grid = open_grid(5, 3);
        grid.remove_vertex((2, 0));
        grid.remove_vertex((2, 1));
        let field = FlowField::build(&grid, (4, 0));
        assert_eq!(field.steps((4, 0)), Some(0));
        assert_eq!(field.steps((0, 0)), Some(8));
        assert_eq!(field.steps((2, 0)), None, "a closed tile");
    }

    #[test]
    fn following_the_steps_reaches_the_goal() {
        let mut grid = open_grid(6, 4);
        grid.remove_vertex((2, 1));
        grid.remove_vertex((2, 2));
        grid.remove_vertex((2, 3));
        let field = FlowField::build(&grid, (5, 3));

        let mut at = (0, 3);
        for _ in 0..field.steps(at).unwrap() {
            let Step::Towards(next) = field.step_from(at) else {
                panic!("stuck at {at:?}");
            };
            assert!(grid.has_vertex(next));
            at = next;
        }
        assert_eq!(field.step_from(at), Step::Arrived);
    }

    #[test]
    fn a_tile_walled_off_from_the_goal_has_no_way() {
        let mut grid = open_grid(5, 1);
        grid.remove_vertex((2, 0));
        let field = FlowField::build(&grid, (4, 0));
        assert_eq!(field.step_from((0, 0)), Step::NoWay);
        assert_eq!(field.step_from((2, 0)), Step::NoWay, "standing in a wall");
        assert_eq!(field.step_from((9, 9)), Step::NoWay, "off the map");
    }

    #[test]
    fn a_closed_goal_is_out_of_reach() {
        let mut grid = open_grid(3, 3);
        grid.remove_vertex((1, 1));
        let field = FlowField::build(&grid, (1, 1));
        assert_eq!(field.step_from((0, 0)), Step::NoWay);
    }

    #[test]
    fn the_edges_of_the_grid_stop_the_walk() {
        let field = FlowField::build(&open_grid(3, 3), (0, 0));
        assert_eq!(field.step_from((1, 0)), Step::Towards((0, 0)));
        assert_eq!(field.steps((2, 2)), Some(4));
    }

    #[test]
    fn ties_go_towards_the_goal_as_the_crow_flies() {
        let field = FlowField::build(&open_grid(9, 3), (8, 1));
        // Right and down are equally many steps; right closes the longer gap.
        assert_eq!(field.step_from((0, 0)), Step::Towards((1, 0)));
    }

    /// Repairing after each change must leave what a fresh walk would.
    #[test]
    fn repairs_match_a_full_walk() {
        let mut seed = 0x9e37_79b9_u32;
        let mut random = move |below: u32| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            (seed % below) as usize
        };
        for _ in 0..12 {
            let mut grid = open_grid(14, 11);
            let goal = (random(14), random(11));
            for _ in 0..40 {
                let tile = (random(14), random(11));
                if tile != goal {
                    grid.remove_vertex(tile);
                }
            }
            let mut field = FlowField::build(&grid, goal);
            for _ in 0..150 {
                let tile = (random(14), random(11));
                if tile == goal {
                    continue;
                }
                if grid.has_vertex(tile) {
                    grid.remove_vertex(tile);
                    field.closed(&grid, tile);
                } else {
                    grid.add_vertex(tile);
                    field.opened(&grid, tile);
                }
                assert_eq!(field.steps, FlowField::build(&grid, goal).steps);
            }
        }
    }

    #[test]
    fn closing_the_only_way_through_cuts_off_what_lies_behind_it() {
        let mut grid = open_grid(5, 1);
        let mut field = FlowField::build(&grid, (4, 0));
        grid.remove_vertex((2, 0));
        field.closed(&grid, (2, 0));
        assert_eq!(field.steps((0, 0)), None);
        grid.add_vertex((2, 0));
        field.opened(&grid, (2, 0));
        assert_eq!(field.steps((0, 0)), Some(4));
    }

    #[test]
    fn closing_or_opening_the_goal_leaves_it_to_a_full_walk() {
        let mut grid = open_grid(3, 3);
        let mut field = FlowField::build(&grid, (1, 1));
        grid.remove_vertex((1, 1));
        field.closed(&grid, (1, 1));
        assert!(!field.fresh);

        let mut field = FlowField::build(&grid, (1, 1));
        grid.add_vertex((1, 1));
        field.opened(&grid, (1, 1));
        assert!(!field.fresh);
    }

    /// The same answers as the search it replaces, on scrambled grids.
    #[test]
    fn it_agrees_with_a_star() {
        let mut seed = 0x2545_f491_u32;
        let mut random = move || {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            seed
        };
        for _ in 0..40 {
            let mut grid = open_grid(24, 18);
            for row in 0..18 {
                for col in 0..24 {
                    if random() % 100 < 35 {
                        grid.remove_vertex((col, row));
                    }
                }
            }
            let goal = (12, 9);
            grid.add_vertex(goal);
            let field = FlowField::build(&grid, goal);
            for tile in &grid {
                let searched = astar(
                    &tile,
                    |t| grid.neighbours(*t).into_iter().map(|t| (t, 1u32)),
                    |t| grid.distance(*t, goal) as u32,
                    |t| *t == goal,
                );
                assert_eq!(field.steps(tile), searched.map(|(_, cost)| cost));
            }
        }
    }
}
