use super::*;
use crate::render::{Buffer, Rgb, color_for};

const SNAP_DISTANCE: usize = 2;

impl Layout {
    pub fn begin_drag(&mut self, x: usize, y: usize) {
        for panel in self.hit_order() {
            if panel == Panel::Logs && !self.logs_visible {
                continue;
            }
            if panel == Panel::Shell && !self.shell_visible {
                continue;
            }
            let rect = self.rect(panel);
            if rect.contains(x, y) {
                self.focus = panel;
                self.bring_to_front(panel);
                let resize = x == rect.x + rect.w.saturating_sub(1)
                    && y == rect.y + rect.h.saturating_sub(1);
                self.drag = Some(Drag {
                    panel,
                    dx: x.saturating_sub(rect.x),
                    dy: y.saturating_sub(rect.y),
                    resize,
                });
                self.drag_preview = Some(rect);
                return;
            }
        }
        self.drag = None;
        self.drag_preview = None;
    }

    pub(super) fn hit_order(&self) -> Vec<Panel> {
        let mut order = self.order.clone();
        order.reverse();
        if self.logs_visible && !order.contains(&Panel::Logs) {
            order.insert(0, Panel::Logs);
        }
        if self.shell_visible {
            order.insert(0, Panel::Shell);
        }
        order
    }

    fn bring_to_front(&mut self, panel: Panel) {
        if let Some(index) = self.order.iter().position(|candidate| *candidate == panel) {
            self.order.remove(index);
            self.order.push(panel);
        }
    }

    pub fn drag_to(&mut self, x: usize, y: usize) {
        let Some(drag) = self.drag else {
            return;
        };
        if drag.resize {
            let mut rect = self.rect(drag.panel);
            let (min_w, min_h) = min_size(drag.panel);
            rect.w = x
                .saturating_sub(rect.x)
                .saturating_add(1)
                .max(min_w)
                .min(self.width.saturating_sub(rect.x));
            rect.h = y
                .saturating_sub(rect.y)
                .saturating_add(1)
                .max(min_h)
                .min(self.height.saturating_sub(rect.y));
            rect = snap_bottom_right(self, drag.panel, rect);
            self.drag_preview = Some(rect);
            set_rect(self, drag.panel, rect);
            return;
        }
        let target_x = snap(x.saturating_sub(drag.dx), self.width);
        let target_y = snap(y.saturating_sub(drag.dy), self.height);
        let mut rect = self.rect(drag.panel);
        rect.x = target_x;
        rect.y = target_y;
        rect = rect.clamp(self.width, self.height);
        rect = snap_to_peers(self, drag.panel, rect);
        self.drag_preview = Some(rect);
        set_rect(self, drag.panel, rect);
    }

    pub fn end_drag(&mut self) {
        self.drag = None;
        self.drag_preview = None;
        self.save_state();
    }

    pub fn is_dragging(&self, panel: Panel) -> bool {
        self.drag.is_some_and(|drag| drag.panel == panel)
    }

    pub fn drag_preview(&self) -> Option<Rect> {
        self.drag_preview
    }

    pub fn draw_drag_preview(&self, buffer: &mut Buffer, palette: &[[u8; 3]]) {
        let Some(rect) = self.drag_preview() else {
            return;
        };
        let color = color_for(palette, 0.92);
        let glow = Rgb::from_slice(palette[0]).lerp(color, 0.18);
        let right = rect.x + rect.w.saturating_sub(1);
        let bottom = rect.y + rect.h.saturating_sub(1);
        for x in rect.x..=right {
            if x % 2 == 0 {
                buffer.set(x, rect.y, '┄', color, glow);
                buffer.set(x, bottom, '┄', color, glow);
            }
        }
        for y in rect.y..=bottom {
            if y % 2 == 0 {
                buffer.set(rect.x, y, '┆', color, glow);
                buffer.set(right, y, '┆', color, glow);
            }
        }
    }
}

fn visible_panels(layout: &Layout, exclude: Panel) -> impl Iterator<Item = (Panel, Rect)> + '_ {
    [
        Panel::Meta,
        Panel::Cover,
        Panel::AudioScope,
        Panel::Lyrics,
        Panel::Logs,
        Panel::Shell,
    ]
    .into_iter()
    .filter(move |panel| *panel != exclude)
    .filter(move |panel| *panel != Panel::Logs || layout.logs_visible)
    .filter(move |panel| *panel != Panel::Shell || layout.shell_visible)
    .filter(move |panel| !layout.is_minimized(*panel))
    .map(move |panel| (panel, layout.rect(panel)))
}

fn snap_to_peers(layout: &Layout, panel: Panel, mut rect: Rect) -> Rect {
    let original = rect;
    let peers: Vec<Rect> = visible_panels(layout, panel)
        .map(|(_, rect)| rect)
        .collect();
    if peers.iter().any(|peer| {
        let peer_area = peer.w.saturating_mul(peer.h).max(1);
        let area = original.w.saturating_mul(original.h).max(1);
        overlap_area(original, *peer) * 100 > peer_area.min(area) * 40
    }) {
        // Fully stacked windows remain valid so the glass compositor can be
        // exercised deliberately; only partial edge collisions snap away.
        return original;
    }
    let mut candidates = vec![(original, false)];
    for peer in &peers {
        let peer = *peer;
        let near_x = original.x.abs_diff(peer.x) <= SNAP_DISTANCE
            || (original.x + original.w).abs_diff(peer.x) <= SNAP_DISTANCE
            || original.x.abs_diff(peer.x + peer.w) <= SNAP_DISTANCE;
        let near_y = original.y.abs_diff(peer.y) <= SNAP_DISTANCE
            || (original.y + original.h).abs_diff(peer.y) <= SNAP_DISTANCE
            || original.y.abs_diff(peer.y + peer.h) <= SNAP_DISTANCE;
        if intersects(original, peer) || near_x {
            for x in [peer.x, peer.x + peer.w, peer.x.saturating_sub(rect.w)] {
                rect.x = x;
                rect = rect.clamp(layout.width, layout.height);
                candidates.push((rect, true));
            }
            rect = original;
        }
        if intersects(original, peer) || near_y {
            for y in [peer.y, peer.y + peer.h, peer.y.saturating_sub(rect.h)] {
                rect.y = y;
                rect = rect.clamp(layout.width, layout.height);
                candidates.push((rect, true));
            }
            rect = original;
        }
        if intersects(original, peer) {
            candidates.push((
                Rect::new(peer.x + peer.w, original.y, original.w, original.h)
                    .clamp(layout.width, layout.height),
                true,
            ));
            candidates.push((
                Rect::new(
                    peer.x.saturating_sub(original.w),
                    original.y,
                    original.w,
                    original.h,
                )
                .clamp(layout.width, layout.height),
                true,
            ));
            candidates.push((
                Rect::new(original.x, peer.y + peer.h, original.w, original.h)
                    .clamp(layout.width, layout.height),
                true,
            ));
            candidates.push((
                Rect::new(
                    original.x,
                    peer.y.saturating_sub(original.h),
                    original.w,
                    original.h,
                )
                .clamp(layout.width, layout.height),
                true,
            ));
        }
    }
    candidates
        .into_iter()
        .min_by_key(|(candidate, snapped)| {
            let overlap: usize = peers
                .iter()
                .map(|peer| overlap_area(*candidate, *peer))
                .sum();
            let movement = candidate.x.abs_diff(original.x)
                + candidate.y.abs_diff(original.y)
                + candidate.w.abs_diff(original.w)
                + candidate.h.abs_diff(original.h);
            (overlap, usize::from(!snapped), movement)
        })
        .map_or(original, |(candidate, _)| candidate)
}

fn overlap_area(a: Rect, b: Rect) -> usize {
    let width = (a.x + a.w).min(b.x + b.w).saturating_sub(a.x.max(b.x));
    let height = (a.y + a.h).min(b.y + b.h).saturating_sub(a.y.max(b.y));
    width.saturating_mul(height)
}

fn snap_bottom_right(layout: &Layout, panel: Panel, rect: Rect) -> Rect {
    let right = rect.x + rect.w;
    let bottom = rect.y + rect.h;
    let mut result = rect;
    for (_, peer) in visible_panels(layout, panel) {
        for edge in [peer.x, peer.x + peer.w] {
            if edge.abs_diff(right) <= SNAP_DISTANCE && edge > rect.x {
                result.w = edge - rect.x;
            }
        }
        for edge in [peer.y, peer.y + peer.h] {
            if edge.abs_diff(bottom) <= SNAP_DISTANCE && edge > rect.y {
                result.h = edge - rect.y;
            }
        }
    }
    let (min_w, min_h) = min_size(panel);
    result.w = result.w.max(min_w);
    result.h = result.h.max(min_h);
    result.clamp(layout.width, layout.height)
}
