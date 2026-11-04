use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SplitAxis {
    Columns, // Vertical split: left | right (divider runs vertically)
    Rows,    // Horizontal split: top / bottom (divider runs horizontally)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Left,
    Right,
    Up,
    Down,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TileNode {
    Leaf(usize),
    Split {
        id: u64,
        axis: SplitAxis,
        ratio: f32, // clamped to 0.15 ..= 0.85, default 0.5
        first: Box<TileNode>,
        second: Box<TileNode>,
    },
}

impl TileNode {
    /// Count the total number of leaf panes in this subtree.
    pub fn pane_count(&self) -> usize {
        match self {
            TileNode::Leaf(_) => 1,
            TileNode::Split { first, second, .. } => first.pane_count() + second.pane_count(),
        }
    }

    /// Check if this subtree contains a specific pane index.
    pub fn contains_pane(&self, pane_ix: usize) -> bool {
        match self {
            TileNode::Leaf(ix) => *ix == pane_ix,
            TileNode::Split { first, second, .. } => {
                first.contains_pane(pane_ix) || second.contains_pane(pane_ix)
            }
        }
    }

    /// Split the target pane by replacing its Leaf with a new Split node.
    pub fn split_pane(
        &mut self,
        target_pane: usize,
        new_pane: usize,
        axis: SplitAxis,
        next_id: &mut u64,
    ) -> bool {
        match self {
            TileNode::Leaf(ix) if *ix == target_pane => {
                let id = *next_id;
                *next_id += 1;
                let current_leaf = TileNode::Leaf(*ix);
                let new_leaf = TileNode::Leaf(new_pane);
                *self = TileNode::Split {
                    id,
                    axis,
                    ratio: 0.50,
                    first: Box::new(current_leaf),
                    second: Box::new(new_leaf),
                };
                true
            }
            TileNode::Split { first, second, .. } => {
                if first.split_pane(target_pane, new_pane, axis, next_id) {
                    true
                } else {
                    second.split_pane(target_pane, new_pane, axis, next_id)
                }
            }
            _ => false,
        }
    }

    /// Adjust ratio of the split containing `active_pane` by `delta`.
    pub fn adjust_active_ratio(&mut self, active_pane: usize, delta: f32) -> bool {
        match self {
            TileNode::Leaf(_) => false,
            TileNode::Split {
                ratio,
                first,
                second,
                ..
            } => {
                if first.contains_pane(active_pane) {
                    // Active is in first child: adjusting ratio in first child first
                    if first.adjust_active_ratio(active_pane, delta) {
                        return true;
                    }
                    *ratio = (*ratio + delta).clamp(0.15, 0.85);
                    true
                } else if second.contains_pane(active_pane) {
                    // Active is in second child: adjusting ratio in second child first
                    if second.adjust_active_ratio(active_pane, delta) {
                        return true;
                    }
                    *ratio = (*ratio - delta).clamp(0.15, 0.85);
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Set ratio of a specific split node by ID.
    pub fn set_split_ratio(&mut self, split_id: u64, new_ratio: f32) -> bool {
        match self {
            TileNode::Leaf(_) => false,
            TileNode::Split {
                id,
                ratio,
                first,
                second,
                ..
            } => {
                if *id == split_id {
                    *ratio = new_ratio.clamp(0.15, 0.85);
                    true
                } else {
                    first.set_split_ratio(split_id, new_ratio)
                        || second.set_split_ratio(split_id, new_ratio)
                }
            }
        }
    }

    /// Toggle axis (Columns <-> Rows) of the split containing `active_pane`.
    pub fn toggle_active_axis(&mut self, active_pane: usize) -> bool {
        match self {
            TileNode::Leaf(_) => false,
            TileNode::Split {
                axis,
                first,
                second,
                ..
            } => {
                if first.contains_pane(active_pane) {
                    if first.toggle_active_axis(active_pane) {
                        return true;
                    }
                    *axis = match *axis {
                        SplitAxis::Columns => SplitAxis::Rows,
                        SplitAxis::Rows => SplitAxis::Columns,
                    };
                    true
                } else if second.contains_pane(active_pane) {
                    if second.toggle_active_axis(active_pane) {
                        return true;
                    }
                    *axis = match *axis {
                        SplitAxis::Columns => SplitAxis::Rows,
                        SplitAxis::Rows => SplitAxis::Columns,
                    };
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Remove a pane from the tree, unwrapping its parent Split to the surviving sibling.
    /// Returns the updated root node (None if the last pane was removed).
    pub fn remove_pane(self, pane_to_remove: usize) -> Option<TileNode> {
        match self {
            TileNode::Leaf(ix) => {
                if ix == pane_to_remove {
                    None
                } else {
                    // Renumber index if it was after the removed pane
                    let new_ix = if ix > pane_to_remove { ix - 1 } else { ix };
                    Some(TileNode::Leaf(new_ix))
                }
            }
            TileNode::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => {
                if first.contains_pane(pane_to_remove) {
                    match first.remove_pane(pane_to_remove) {
                        Some(new_first) => {
                            let new_second = second.decrement_indices_above(pane_to_remove);
                            Some(TileNode::Split {
                                id,
                                axis,
                                ratio,
                                first: Box::new(new_first),
                                second: Box::new(new_second),
                            })
                        }
                        None => {
                            // First branch collapsed completely: second branch takes over!
                            Some(second.decrement_indices_above(pane_to_remove))
                        }
                    }
                } else if second.contains_pane(pane_to_remove) {
                    match second.remove_pane(pane_to_remove) {
                        Some(new_second) => {
                            let new_first = first.decrement_indices_above(pane_to_remove);
                            Some(TileNode::Split {
                                id,
                                axis,
                                ratio,
                                first: Box::new(new_first),
                                second: Box::new(new_second),
                            })
                        }
                        None => {
                            // Second branch collapsed completely: first branch takes over!
                            Some(first.decrement_indices_above(pane_to_remove))
                        }
                    }
                } else {
                    let new_first = first.decrement_indices_above(pane_to_remove);
                    let new_second = second.decrement_indices_above(pane_to_remove);
                    Some(TileNode::Split {
                        id,
                        axis,
                        ratio,
                        first: Box::new(new_first),
                        second: Box::new(new_second),
                    })
                }
            }
        }
    }

    fn decrement_indices_above(self, threshold: usize) -> TileNode {
        match self {
            TileNode::Leaf(ix) => {
                let new_ix = if ix > threshold { ix - 1 } else { ix };
                TileNode::Leaf(new_ix)
            }
            TileNode::Split {
                id,
                axis,
                ratio,
                first,
                second,
            } => TileNode::Split {
                id,
                axis,
                ratio,
                first: Box::new(first.decrement_indices_above(threshold)),
                second: Box::new(second.decrement_indices_above(threshold)),
            },
        }
    }

    /// Return an arbitrary leaf pane from this subtree (first encountered).
    pub fn first_leaf(&self) -> usize {
        match self {
            TileNode::Leaf(ix) => *ix,
            TileNode::Split { first, .. } => first.first_leaf(),
        }
    }

    /// Return the last leaf pane from this subtree.
    pub fn last_leaf(&self) -> usize {
        match self {
            TileNode::Leaf(ix) => *ix,
            TileNode::Split { second, .. } => second.last_leaf(),
        }
    }

    /// Spatial navigation in Hyprland style: Find the adjacent pane in `dir` relative to `active_pane`.
    pub fn find_neighbor(&self, active_pane: usize, dir: Direction) -> Option<usize> {
        self.find_neighbor_internal(active_pane, dir)
    }

    fn find_neighbor_internal(&self, active_pane: usize, dir: Direction) -> Option<usize> {
        match self {
            TileNode::Leaf(_) => None,
            TileNode::Split {
                axis,
                first,
                second,
                ..
            } => {
                let in_first = first.contains_pane(active_pane);
                let in_second = second.contains_pane(active_pane);

                if !in_first && !in_second {
                    return None;
                }

                // Check if this split's axis directly matches the navigation direction
                let matches_axis = match (axis, dir) {
                    (SplitAxis::Columns, Direction::Left | Direction::Right) => true,
                    (SplitAxis::Rows, Direction::Up | Direction::Down) => true,
                    _ => false,
                };

                if matches_axis {
                    if in_first {
                        // Moving forward along axis (Right or Down) crosses into second branch!
                        match dir {
                            Direction::Right | Direction::Down => {
                                // Prefer the internal neighbor first, else cross
                                first
                                    .find_neighbor_internal(active_pane, dir)
                                    .or_else(|| Some(second.first_leaf()))
                            }
                            Direction::Left | Direction::Up => {
                                first.find_neighbor_internal(active_pane, dir)
                            }
                        }
                    } else {
                        // In second: Moving backward along axis (Left or Up) crosses into first branch!
                        match dir {
                            Direction::Left | Direction::Up => {
                                second
                                    .find_neighbor_internal(active_pane, dir)
                                    .or_else(|| Some(first.last_leaf()))
                            }
                            Direction::Right | Direction::Down => {
                                second.find_neighbor_internal(active_pane, dir)
                            }
                        }
                    }
                } else {
                    // Axis doesn't match: recurse down the active branch
                    if in_first {
                        first.find_neighbor_internal(active_pane, dir)
                    } else {
                        second.find_neighbor_internal(active_pane, dir)
                    }
                }
            }
        }
    }

    /// Swap two pane indices in the tree.
    pub fn swap_panes(&mut self, a: usize, b: usize) {
        match self {
            TileNode::Leaf(ix) => {
                if *ix == a {
                    *ix = b;
                } else if *ix == b {
                    *ix = a;
                }
            }
            TileNode::Split { first, second, .. } => {
                first.swap_panes(a, b);
                second.swap_panes(a, b);
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TileTree {
    pub root: TileNode,
    pub next_split_id: u64,
}

impl TileTree {
    pub fn new(initial_pane: usize) -> Self {
        Self {
            root: TileNode::Leaf(initial_pane),
            next_split_id: 1,
        }
    }

    pub fn split(&mut self, target_pane: usize, new_pane: usize, axis: SplitAxis) -> bool {
        self.root
            .split_pane(target_pane, new_pane, axis, &mut self.next_split_id)
    }

    pub fn remove_pane(&mut self, pane_to_remove: usize) -> bool {
        let old_root = self.root.clone();
        if let Some(new_root) = old_root.remove_pane(pane_to_remove) {
            self.root = new_root;
            true
        } else {
            false
        }
    }

    pub fn find_neighbor(&self, active_pane: usize, dir: Direction) -> Option<usize> {
        self.root.find_neighbor(active_pane, dir)
    }

    pub fn adjust_active_ratio(&mut self, active_pane: usize, delta: f32) -> bool {
        self.root.adjust_active_ratio(active_pane, delta)
    }

    pub fn set_split_ratio(&mut self, split_id: u64, ratio: f32) -> bool {
        self.root.set_split_ratio(split_id, ratio)
    }

    pub fn toggle_active_axis(&mut self, active_pane: usize) -> bool {
        self.root.toggle_active_axis(active_pane)
    }

    pub fn swap_panes(&mut self, a: usize, b: usize) {
        self.root.swap_panes(a, b);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_pane_tree() {
        let tree = TileTree::new(0);
        assert_eq!(tree.root, TileNode::Leaf(0));
        assert_eq!(tree.root.pane_count(), 1);
    }

    #[test]
    fn test_hyprland_one_vertical_two_horizontal_layout() {
        let mut tree = TileTree::new(0);

        // 1. Split Pane 0 vertically (Columns) -> Left: 0, Right: 1
        assert!(tree.split(0, 1, SplitAxis::Columns));
        assert_eq!(tree.root.pane_count(), 2);

        // 2. Split Pane 1 horizontally (Rows) -> Left: 0 (vertical), Right: 1 (top) and 2 (bottom)
        assert!(tree.split(1, 2, SplitAxis::Rows));
        assert_eq!(tree.root.pane_count(), 3);

        // Verify structure:
        // Split (Columns)
        //   - Leaf(0)
        //   - Split (Rows)
        //       - Leaf(1)
        //       - Leaf(2)
        match &tree.root {
            TileNode::Split {
                axis: SplitAxis::Columns,
                first,
                second,
                ..
            } => {
                assert_eq!(**first, TileNode::Leaf(0));
                match &**second {
                    TileNode::Split {
                        axis: SplitAxis::Rows,
                        first: r_first,
                        second: r_second,
                        ..
                    } => {
                        assert_eq!(**r_first, TileNode::Leaf(1));
                        assert_eq!(**r_second, TileNode::Leaf(2));
                    }
                    _ => panic!("Expected second node to be Rows split"),
                }
            }
            _ => panic!("Expected root to be Columns split"),
        }

        // Verify spatial navigation across Hyprland layout:
        // From 0: moving Right goes to 1
        assert_eq!(tree.find_neighbor(0, Direction::Right), Some(1));
        // From 1: moving Down goes to 2
        assert_eq!(tree.find_neighbor(1, Direction::Down), Some(2));
        // From 2: moving Up goes to 1
        assert_eq!(tree.find_neighbor(2, Direction::Up), Some(1));
        // From 1: moving Left goes to 0
        assert_eq!(tree.find_neighbor(1, Direction::Left), Some(0));
        // From 2: moving Left goes to 0
        assert_eq!(tree.find_neighbor(2, Direction::Left), Some(0));

        // 3. Remove Pane 1: Pane 2 should take its place, and Left stays 0
        assert!(tree.remove_pane(1));
        assert_eq!(tree.root.pane_count(), 2);
        // After renumbering, old 2 becomes 1:
        assert_eq!(
            tree.root,
            TileNode::Split {
                id: 1,
                axis: SplitAxis::Columns,
                ratio: 0.5,
                first: Box::new(TileNode::Leaf(0)),
                second: Box::new(TileNode::Leaf(1)),
            }
        );
    }

    #[test]
    fn test_ratio_resizing() {
        let mut tree = TileTree::new(0);
        tree.split(0, 1, SplitAxis::Columns);

        // Adjust ratio with delta +0.10
        assert!(tree.adjust_active_ratio(0, 0.10));
        if let TileNode::Split { ratio, .. } = &tree.root {
            assert!((*ratio - 0.60).abs() < 1e-4);
        } else {
            panic!("Expected split");
        }

        // Set ratio directly (e.g. from mouse drag)
        assert!(tree.set_split_ratio(1, 0.75));
        if let TileNode::Split { ratio, .. } = &tree.root {
            assert!((*ratio - 0.75).abs() < 1e-4);
        } else {
            panic!("Expected split");
        }
    }
}
