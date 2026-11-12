//! The tiling tree behind the "free" terminal layout.
//!
//! Panes never overlap and never leave a hole: the area is split recursively,
//! so moving one pane makes its neighbours take the room it left — the same
//! model tiling window managers use.

use egui::{Pos2, Rect, Vec2};

/// How a node divides its rectangle.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dir {
    /// Children side by side, split along x.
    Row,
    /// Children stacked, split along y.
    Column,
}

/// Where a dropped pane lands relative to the pane under the cursor.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    Left,
    Right,
    Top,
    Bottom,
    /// Trade places with the target instead of splitting it.
    Center,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Tile {
    Leaf(u64),
    Split {
        dir: Dir,
        /// Share of the parent rectangle taken by `a`, 0.1..0.9.
        ratio: f32,
        a: Box<Tile>,
        b: Box<Tile>,
    },
}

/// A node address: one step per level, `false` into `a`, `true` into `b`.
pub type Path = Vec<bool>;

/// The draggable gutter between two siblings.
#[derive(Clone, Debug, PartialEq)]
pub struct Splitter {
    pub path: Path,
    /// The gutter itself, where the pointer grabs it.
    pub rect: Rect,
    /// The rectangle the two children share, which the ratio divides.
    pub parent: Rect,
    pub dir: Dir,
}

const MIN_RATIO: f32 = 0.1;
const MAX_RATIO: f32 = 0.9;

impl Tile {
    /// A balanced tree over `ids`, halving the longest side at each step so the
    /// result reads as a grid.
    pub fn balanced(ids: &[u64]) -> Option<Self> {
        match ids.len() {
            0 => None,
            1 => Some(Tile::Leaf(ids[0])),
            n => {
                let half = n.div_ceil(2);
                let dir = if n > 2 { Dir::Column } else { Dir::Row };
                let a = Tile::balanced(&ids[..half])?;
                let b = Tile::balanced(&ids[half..])?;
                Some(Tile::Split {
                    dir: if n == 2 { Dir::Row } else { dir },
                    ratio: half as f32 / n as f32,
                    a: Box::new(a),
                    b: Box::new(b),
                })
            }
        }
    }

    pub fn leaves(&self) -> Vec<u64> {
        let mut out = Vec::new();
        self.collect(&mut out);
        out
    }

    fn collect(&self, out: &mut Vec<u64>) {
        match self {
            Tile::Leaf(id) => out.push(*id),
            Tile::Split { a, b, .. } => {
                a.collect(out);
                b.collect(out);
            }
        }
    }

    pub fn contains(&self, id: u64) -> bool {
        match self {
            Tile::Leaf(other) => *other == id,
            Tile::Split { a, b, .. } => a.contains(id) || b.contains(id),
        }
    }

    /// Every pane's rectangle, with `gap` pixels of air between siblings.
    pub fn layout(&self, area: Rect, gap: f32) -> Vec<(u64, Rect)> {
        let mut out = Vec::new();
        self.walk(area, gap, &mut |id, rect| out.push((id, rect)));
        out
    }

    fn walk(&self, area: Rect, gap: f32, visit: &mut impl FnMut(u64, Rect)) {
        match self {
            Tile::Leaf(id) => visit(*id, area),
            Tile::Split { dir, ratio, a, b } => {
                let (first, second) = split_rect(area, *dir, *ratio, gap);
                a.walk(first, gap, visit);
                b.walk(second, gap, visit);
            }
        }
    }

    /// The gutters, deepest last, so the innermost one wins a click.
    pub fn splitters(&self, area: Rect, gap: f32) -> Vec<Splitter> {
        let mut out = Vec::new();
        self.collect_splitters(area, gap, &mut Vec::new(), &mut out);
        out
    }

    fn collect_splitters(&self, area: Rect, gap: f32, path: &mut Path, out: &mut Vec<Splitter>) {
        let Tile::Split { dir, ratio, a, b } = self else {
            return;
        };
        let (first, second) = split_rect(area, *dir, *ratio, gap);
        let rect = match dir {
            Dir::Row => Rect::from_min_max(
                Pos2::new(first.right(), area.top()),
                Pos2::new(second.left(), area.bottom()),
            ),
            Dir::Column => Rect::from_min_max(
                Pos2::new(area.left(), first.bottom()),
                Pos2::new(area.right(), second.top()),
            ),
        };
        out.push(Splitter {
            path: path.clone(),
            rect,
            parent: area,
            dir: *dir,
        });
        path.push(false);
        a.collect_splitters(first, gap, path, out);
        path.pop();
        path.push(true);
        b.collect_splitters(second, gap, path, out);
        path.pop();
    }

    fn node_mut(&mut self, path: &[bool]) -> Option<&mut Tile> {
        match path.split_first() {
            None => Some(self),
            Some((step, rest)) => match self {
                Tile::Leaf(_) => None,
                Tile::Split { a, b, .. } => {
                    if *step {
                        b.node_mut(rest)
                    } else {
                        a.node_mut(rest)
                    }
                }
            },
        }
    }

    /// Moves one gutter. `fraction` is the new share of the first child.
    pub fn set_ratio(&mut self, path: &[bool], fraction: f32) {
        if let Some(Tile::Split { ratio, .. }) = self.node_mut(path) {
            *ratio = fraction.clamp(MIN_RATIO, MAX_RATIO);
        }
    }

    pub fn ratio_at(&self, path: &[bool]) -> Option<f32> {
        let mut node = self;
        for step in path {
            match node {
                Tile::Leaf(_) => return None,
                Tile::Split { a, b, .. } => node = if *step { b } else { a },
            }
        }
        match node {
            Tile::Split { ratio, .. } => Some(*ratio),
            Tile::Leaf(_) => None,
        }
    }

    /// Puts `id` beside `target`. The target leaf becomes a split holding both.
    pub fn insert(&mut self, target: u64, side: Side, id: u64) {
        if side == Side::Center {
            self.swap(target, id);
            return;
        }
        let (dir, first) = match side {
            Side::Left => (Dir::Row, true),
            Side::Right => (Dir::Row, false),
            Side::Top => (Dir::Column, true),
            Side::Bottom => (Dir::Column, false),
            Side::Center => unreachable!(),
        };
        self.replace_leaf(target, |leaf| {
            let new = Box::new(Tile::Leaf(id));
            let old = Box::new(leaf);
            let (a, b) = if first { (new, old) } else { (old, new) };
            Tile::Split {
                dir,
                ratio: 0.5,
                a,
                b,
            }
        });
    }

    fn replace_leaf(&mut self, target: u64, make: impl FnOnce(Tile) -> Tile) {
        match self {
            Tile::Leaf(id) if *id == target => {
                let old = std::mem::replace(self, Tile::Leaf(target));
                *self = make(old);
            }
            Tile::Leaf(_) => {}
            Tile::Split { a, b, .. } => {
                if a.contains(target) {
                    a.replace_leaf(target, make);
                } else if b.contains(target) {
                    b.replace_leaf(target, make);
                }
            }
        }
    }

    /// Puts `id` against one side of the *whole* area, taking half of it —
    /// the full-height column or full-width row a window manager would snap to.
    pub fn insert_root(&mut self, side: Side, id: u64) {
        let (dir, first) = match side {
            Side::Left => (Dir::Row, true),
            Side::Right => (Dir::Row, false),
            Side::Top => (Dir::Column, true),
            Side::Bottom => (Dir::Column, false),
            Side::Center => return,
        };
        let old = std::mem::replace(self, Tile::Leaf(id));
        let new = Box::new(Tile::Leaf(id));
        let old = Box::new(old);
        let (a, b) = if first { (new, old) } else { (old, new) };
        *self = Tile::Split {
            dir,
            ratio: 0.5,
            a,
            b,
        };
    }

    /// Trades the places of two panes.
    pub fn swap(&mut self, first: u64, second: u64) {
        match self {
            Tile::Leaf(id) => {
                if *id == first {
                    *id = second;
                } else if *id == second {
                    *id = first;
                }
            }
            Tile::Split { a, b, .. } => {
                a.swap(first, second);
                b.swap(first, second);
            }
        }
    }
}

/// Drops a pane, collapsing the split it lived in. `None` when nothing is left.
pub fn remove(tree: Option<Tile>, id: u64) -> Option<Tile> {
    match tree? {
        Tile::Leaf(leaf) => (leaf != id).then_some(Tile::Leaf(leaf)),
        Tile::Split { dir, ratio, a, b } => {
            let a = remove(Some(*a), id);
            let b = remove(Some(*b), id);
            match (a, b) {
                (Some(a), Some(b)) => Some(Tile::Split {
                    dir,
                    ratio,
                    a: Box::new(a),
                    b: Box::new(b),
                }),
                (Some(only), None) | (None, Some(only)) => Some(only),
                (None, None) => None,
            }
        }
    }
}

/// Keeps the tree in step with the open sessions: closed panes leave, new ones
/// are split off the pane that has the most room.
pub fn sync(tree: &mut Option<Tile>, alive: &[u64], area: Rect, gap: f32) {
    let known: Vec<u64> = tree.as_ref().map(Tile::leaves).unwrap_or_default();
    for gone in known.iter().filter(|id| !alive.contains(id)) {
        *tree = remove(tree.take(), *gone);
    }
    let known: Vec<u64> = tree.as_ref().map(Tile::leaves).unwrap_or_default();
    for id in alive.iter().filter(|id| !known.contains(id)) {
        match tree.as_mut() {
            None => *tree = Some(Tile::Leaf(*id)),
            Some(existing) => {
                let (target, rect) = existing
                    .layout(area, gap)
                    .into_iter()
                    .max_by(|a, b| area_of(a.1).total_cmp(&area_of(b.1)))
                    .unwrap_or((*id, area));
                let side = if rect.width() >= rect.height() {
                    Side::Right
                } else {
                    Side::Bottom
                };
                existing.insert(target, side, *id);
            }
        }
    }
}

fn area_of(rect: Rect) -> f32 {
    rect.width() * rect.height()
}

/// Which part of `rect` the pointer is in, as a drop target.
pub fn drop_side(rect: Rect, pointer: Pos2) -> Side {
    let x = ((pointer.x - rect.left()) / rect.width().max(1.0)).clamp(0.0, 1.0);
    let y = ((pointer.y - rect.top()) / rect.height().max(1.0)).clamp(0.0, 1.0);
    let edge = 0.3;
    let from_edge = [
        (x, Side::Left),
        (1.0 - x, Side::Right),
        (y, Side::Top),
        (1.0 - y, Side::Bottom),
    ];
    let (distance, side) = from_edge
        .into_iter()
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .unwrap_or((1.0, Side::Center));
    if distance <= edge { side } else { Side::Center }
}

/// A drop within `margin` of the outer border docks against the whole area
/// instead of splitting the pane under the cursor.
pub fn root_side(area: Rect, pointer: Pos2, margin: f32) -> Option<Side> {
    if !area.contains(pointer) {
        return None;
    }
    let distances = [
        (pointer.x - area.left(), Side::Left),
        (area.right() - pointer.x, Side::Right),
        (pointer.y - area.top(), Side::Top),
        (area.bottom() - pointer.y, Side::Bottom),
    ];
    distances
        .into_iter()
        .filter(|(distance, _)| *distance <= margin)
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, side)| side)
}

/// The band a drop would fill, for the indicator painted under the cursor.
pub fn drop_preview(rect: Rect, side: Side) -> Rect {
    let half = Vec2::new(rect.width() / 2.0, rect.height() / 2.0);
    match side {
        Side::Left => Rect::from_min_size(rect.min, Vec2::new(half.x, rect.height())),
        Side::Right => Rect::from_min_size(
            Pos2::new(rect.center().x, rect.top()),
            Vec2::new(half.x, rect.height()),
        ),
        Side::Top => Rect::from_min_size(rect.min, Vec2::new(rect.width(), half.y)),
        Side::Bottom => Rect::from_min_size(
            Pos2::new(rect.left(), rect.center().y),
            Vec2::new(rect.width(), half.y),
        ),
        Side::Center => rect,
    }
}

/// The ratio a pointer at `pos` asks for while dragging a gutter.
pub fn ratio_for(parent: Rect, dir: Dir, pos: Pos2, gap: f32) -> f32 {
    match dir {
        Dir::Row => (pos.x - parent.left()) / (parent.width() - gap).max(1.0),
        Dir::Column => (pos.y - parent.top()) / (parent.height() - gap).max(1.0),
    }
    .clamp(MIN_RATIO, MAX_RATIO)
}

fn split_rect(area: Rect, dir: Dir, ratio: f32, gap: f32) -> (Rect, Rect) {
    let ratio = ratio.clamp(MIN_RATIO, MAX_RATIO);
    match dir {
        Dir::Row => {
            let usable = (area.width() - gap).max(0.0);
            let first = usable * ratio;
            (
                Rect::from_min_size(area.min, Vec2::new(first, area.height())),
                Rect::from_min_max(Pos2::new(area.left() + first + gap, area.top()), area.max),
            )
        }
        Dir::Column => {
            let usable = (area.height() - gap).max(0.0);
            let first = usable * ratio;
            (
                Rect::from_min_size(area.min, Vec2::new(area.width(), first)),
                Rect::from_min_max(Pos2::new(area.left(), area.top() + first + gap), area.max),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area() -> Rect {
        Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(800.0, 600.0))
    }

    #[test]
    fn a_single_pane_takes_everything() {
        let tree = Tile::balanced(&[7]).unwrap();
        assert_eq!(tree.layout(area(), 10.0), vec![(7, area())]);
    }

    #[test]
    fn panes_tile_without_overlap_or_holes() {
        for n in 1..9u64 {
            let ids: Vec<u64> = (1..=n).collect();
            let tree = Tile::balanced(&ids).unwrap();
            let rects = tree.layout(area(), 8.0);
            assert_eq!(rects.len(), n as usize);
            let covered: f32 = rects.iter().map(|(_, r)| r.width() * r.height()).sum();
            let whole = area().width() * area().height();
            // Only the gutters are left uncovered.
            assert!(covered > whole * 0.9, "{n} panes covered only {covered}");
            for (i, (_, a)) in rects.iter().enumerate() {
                for (_, b) in rects.iter().skip(i + 1) {
                    assert!(!a.shrink(1.0).intersects(b.shrink(1.0)), "{a:?} hits {b:?}");
                }
            }
        }
    }

    #[test]
    fn dropping_beside_a_pane_splits_it() {
        let mut tree = Tile::balanced(&[1, 2]).unwrap();
        tree.insert(2, Side::Bottom, 3);
        assert_eq!(tree.leaves(), vec![1, 2, 3]);
        let rects = tree.layout(area(), 0.0);
        let two = rects.iter().find(|(id, _)| *id == 2).unwrap().1;
        let three = rects.iter().find(|(id, _)| *id == 3).unwrap().1;
        assert!(three.top() >= two.bottom() - 0.01, "3 should sit under 2");
        assert_eq!(two.left(), three.left());
    }

    #[test]
    fn dropping_on_the_middle_swaps_the_two() {
        let mut tree = Tile::balanced(&[1, 2]).unwrap();
        let before = tree.layout(area(), 0.0);
        tree.swap(1, 2);
        let after = tree.layout(area(), 0.0);
        // The two rectangles stay put; the panes inside them trade places.
        assert_eq!(before[0].0, after[1].0);
        assert_eq!(before[1].0, after[0].0);
        assert_eq!(before[0].1, after[0].1);
    }

    #[test]
    fn removing_a_pane_gives_its_room_to_the_others() {
        let tree = Tile::balanced(&[1, 2, 3]).unwrap();
        let tree = remove(Some(tree), 2).unwrap();
        assert_eq!(tree.leaves(), vec![1, 3]);
        let covered: f32 = tree
            .layout(area(), 0.0)
            .iter()
            .map(|(_, r)| r.width() * r.height())
            .sum();
        assert!((covered - area().width() * area().height()).abs() < 1.0);
    }

    #[test]
    fn removing_the_last_pane_empties_the_tree() {
        assert_eq!(remove(Some(Tile::Leaf(5)), 5), None);
    }

    #[test]
    fn sync_adds_newcomers_and_drops_closed_panes() {
        let mut tree = None;
        sync(&mut tree, &[1, 2], area(), 8.0);
        assert_eq!(tree.as_ref().unwrap().leaves(), vec![1, 2]);
        sync(&mut tree, &[2, 3], area(), 8.0);
        let mut leaves = tree.as_ref().unwrap().leaves();
        leaves.sort_unstable();
        assert_eq!(leaves, vec![2, 3]);
    }

    #[test]
    fn a_splitter_sits_between_its_two_children() {
        let tree = Tile::balanced(&[1, 2]).unwrap();
        let splitters = tree.splitters(area(), 10.0);
        assert_eq!(splitters.len(), 1);
        let gutter = &splitters[0];
        assert_eq!(gutter.dir, Dir::Row);
        assert!((gutter.rect.width() - 10.0).abs() < 0.01);
        assert!((gutter.rect.center().x - area().center().x).abs() < 1.0);
    }

    #[test]
    fn dragging_a_splitter_moves_the_shared_edge() {
        let mut tree = Tile::balanced(&[1, 2]).unwrap();
        tree.set_ratio(&[], 0.25);
        let rects = tree.layout(area(), 0.0);
        assert!((rects[0].1.width() - 200.0).abs() < 1.0);
        assert!((rects[1].1.width() - 600.0).abs() < 1.0);
        // Out-of-range drags stop at a usable size.
        tree.set_ratio(&[], 0.0);
        assert_eq!(tree.ratio_at(&[]), Some(MIN_RATIO));
    }

    #[test]
    fn a_gutter_drag_reads_the_ratio_off_the_pointer() {
        let parent = area();
        let half = ratio_for(parent, Dir::Row, parent.center(), 0.0);
        assert!((half - 0.5).abs() < 0.01);
        let edge = ratio_for(parent, Dir::Column, Pos2::new(0.0, -500.0), 0.0);
        assert_eq!(edge, MIN_RATIO);
    }

    #[test]
    fn docking_against_the_area_takes_a_whole_half() {
        // Same flow the drop uses: the pane leaves its old place first.
        let tree = Tile::balanced(&[1, 2, 3]).unwrap();
        let mut tree = remove(Some(tree), 3).unwrap();
        tree.insert_root(Side::Left, 3);
        let rects = tree.layout(area(), 0.0);
        let three = rects.iter().find(|(id, _)| *id == 3).unwrap().1;
        assert_eq!(three.height(), area().height(), "ocupa a altura toda");
        assert!((three.width() - area().width() / 2.0).abs() < 1.0);
        assert!(three.left() <= area().left() + 0.01);
        // The leaf is not duplicated: it only lives in its new place.
        let mut leaves = tree.leaves();
        leaves.sort_unstable();
        assert_eq!(leaves, vec![1, 2, 3]);
    }

    #[test]
    fn only_the_outer_border_docks_against_the_area() {
        let area = area();
        assert_eq!(
            root_side(area, Pos2::new(5.0, 300.0), 40.0),
            Some(Side::Left)
        );
        assert_eq!(
            root_side(area, Pos2::new(400.0, 598.0), 40.0),
            Some(Side::Bottom)
        );
        assert_eq!(root_side(area, area.center(), 40.0), None);
    }

    #[test]
    fn the_drop_side_follows_the_pointer() {
        let rect = area();
        assert_eq!(drop_side(rect, Pos2::new(10.0, 300.0)), Side::Left);
        assert_eq!(drop_side(rect, Pos2::new(790.0, 300.0)), Side::Right);
        assert_eq!(drop_side(rect, Pos2::new(400.0, 10.0)), Side::Top);
        assert_eq!(drop_side(rect, Pos2::new(400.0, 590.0)), Side::Bottom);
        assert_eq!(drop_side(rect, rect.center()), Side::Center);
    }

    #[test]
    fn the_preview_shows_the_half_that_would_be_taken() {
        let rect = area();
        assert_eq!(drop_preview(rect, Side::Left).width(), rect.width() / 2.0);
        assert_eq!(drop_preview(rect, Side::Bottom).top(), rect.center().y);
        assert_eq!(drop_preview(rect, Side::Center), rect);
    }
}
