// Pure geometry: logical and physical conversion, and hit-testing a cursor against every window's drop regions
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::models::{Hit, Point, Region};

/// One window's geometry as the windowing system reports it, plus the regions it registered.
///
/// `inner_position` and `inner_size` describe the content area, never the outer frame: a frameless window with a drop shadow reports an outer position above and left of its content, and regions are measured from the content's origin.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowGeometry {
    pub label: String,
    /// Physical pixels on the virtual screen; may be negative on a monitor left of or above the primary.
    pub inner_position: (i32, i32),
    /// Physical pixels.
    pub inner_size: (u32, u32),
    pub scale_factor: f64,
    pub regions: Vec<Region>,
}

/// Where to place a window so that `grab_offset` (logical, measured from its top-left) sits under `cursor` (physical), using the window's own `scale_factor`.
///
/// This is `(cursor_logical - grab_offset) * scale_factor` with `cursor_logical = cursor / scale_factor`, rounded once at the end.
pub fn placement(cursor: Point, grab_offset: Point, scale_factor: f64) -> (i32, i32) {
    (
        (cursor.x - grab_offset.x * scale_factor).round() as i32,
        (cursor.y - grab_offset.y * scale_factor).round() as i32,
    )
}

/// Converts a physical cursor position to logical pixels with one window's scale factor.
pub fn to_logical(physical: Point, scale_factor: f64) -> Point {
    Point {
        x: physical.x / scale_factor,
        y: physical.y / scale_factor,
    }
}

/// Converts a logical position to physical pixels with one window's scale factor.
pub fn to_physical(logical: Point, scale_factor: f64) -> Point {
    Point {
        x: logical.x * scale_factor,
        y: logical.y * scale_factor,
    }
}

/// A rectangle in physical pixels: left, top, right, bottom.
type Rect = (f64, f64, f64, f64);

fn contains(rect: Rect, cursor: Point) -> bool {
    cursor.x >= rect.0 && cursor.x < rect.2 && cursor.y >= rect.1 && cursor.y < rect.3
}

fn area(rect: Rect) -> f64 {
    (rect.2 - rect.0) * (rect.3 - rect.1)
}

fn intersect(a: Rect, b: Rect) -> Option<Rect> {
    let rect = (a.0.max(b.0), a.1.max(b.1), a.2.min(b.2), a.3.min(b.3));
    (rect.0 < rect.2 && rect.1 < rect.3).then_some(rect)
}

/// Finds the registered region under `cursor` (physical pixels).
///
/// The windowing system does not report stacking order, so overlaps are resolved by size: the region with the smallest visible area (its rectangle clipped to its window's content area) wins, which favours the more specific target. Equal areas go to the earlier window in `windows`, then to the earlier region within that window. A region never matches outside its window's content area.
pub fn hit_test(cursor: Point, windows: &[WindowGeometry]) -> Option<Hit> {
    let mut best: Option<(f64, &WindowGeometry, &Region)> = None;
    for window in windows {
        let origin = (
            f64::from(window.inner_position.0),
            f64::from(window.inner_position.1),
        );
        let content = (
            origin.0,
            origin.1,
            origin.0 + f64::from(window.inner_size.0),
            origin.1 + f64::from(window.inner_size.1),
        );
        for region in &window.regions {
            let rect = (
                origin.0 + region.x * window.scale_factor,
                origin.1 + region.y * window.scale_factor,
                origin.0 + (region.x + region.width) * window.scale_factor,
                origin.1 + (region.y + region.height) * window.scale_factor,
            );
            let Some(visible) = intersect(rect, content) else {
                continue;
            };
            if !contains(visible, cursor) {
                continue;
            }
            let candidate = area(visible);
            // Strictly smaller wins, so the earlier window and region keep ties.
            if best.is_none_or(|(area, _, _)| candidate < area) {
                best = Some((candidate, window, region));
            }
        }
    }
    best.map(|(_, window, region)| Hit {
        window: window.label.clone(),
        region: region.id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn region(id: &str, x: f64, y: f64, width: f64, height: f64) -> Region {
        Region {
            id: id.into(),
            x,
            y,
            width,
            height,
        }
    }

    fn window(
        label: &str,
        position: (i32, i32),
        size: (u32, u32),
        scale: f64,
        regions: Vec<Region>,
    ) -> WindowGeometry {
        WindowGeometry {
            label: label.into(),
            inner_position: position,
            inner_size: size,
            scale_factor: scale,
            regions,
        }
    }

    fn at(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn hit(window: &str, region: &str) -> Option<Hit> {
        Some(Hit {
            window: window.into(),
            region: region.into(),
        })
    }

    #[test]
    fn placement_matches_the_measured_x11_case() {
        // Pointer at logical (1000, 500) on a 2x window with a (120, 14) grab offset: the spike put the inner origin at (1760, 972).
        assert_eq!(
            placement(at(2000.0, 1000.0), at(120.0, 14.0), 2.0),
            (1760, 972)
        );
    }

    #[test]
    fn placement_uses_the_given_scale_and_rounds_once() {
        assert_eq!(
            placement(at(1000.0, 1000.0), at(10.0, 10.0), 1.5),
            (985, 985)
        );
        assert_eq!(placement(at(100.5, 50.4), at(0.0, 0.0), 1.0), (101, 50));
        assert_eq!(
            placement(at(-300.0, -20.0), at(10.0, 10.0), 1.0),
            (-310, -30)
        );
    }

    #[test]
    fn conversions_round_trip() {
        for scale in [1.0, 1.25, 1.5, 2.0, 3.0] {
            let logical = at(123.5, 67.25);
            let back = to_logical(to_physical(logical, scale), scale);
            assert!((back.x - logical.x).abs() < 1e-9 && (back.y - logical.y).abs() < 1e-9);
        }
        assert_eq!(to_physical(at(10.0, 20.0), 1.5), at(15.0, 30.0));
        assert_eq!(to_logical(at(30.0, 60.0), 2.0), at(15.0, 30.0));
    }

    #[test]
    fn a_region_is_measured_from_the_inner_origin() {
        let windows = [window(
            "a",
            (100, 200),
            (800, 600),
            1.0,
            vec![region("strip", 10.0, 5.0, 300.0, 40.0)],
        )];
        assert_eq!(hit_test(at(110.0, 205.0), &windows), hit("a", "strip"));
        assert_eq!(hit_test(at(409.0, 244.0), &windows), hit("a", "strip"));
        assert_eq!(hit_test(at(410.0, 244.0), &windows), None);
        assert_eq!(hit_test(at(109.0, 205.0), &windows), None);
    }

    #[test]
    fn the_shadow_inset_is_not_part_of_the_content() {
        // A frameless window whose outer frame starts 37 logical px (74 physical at 2x) above and left of its content: the geometry handed in is the inner one, so the region is measured from there.
        let inner = (1000, 774);
        let windows = [window(
            "shadowed",
            inner,
            (1600, 1000),
            2.0,
            vec![region("tabs", 0.0, 0.0, 400.0, 36.0)],
        )];
        // Inside the shadow, above the content: no hit.
        assert_eq!(hit_test(at(1100.0, 750.0), &windows), None);
        // The first content pixel: hit.
        assert_eq!(
            hit_test(at(1000.0, 774.0), &windows),
            hit("shadowed", "tabs")
        );
        // 36 logical = 72 physical down from the origin is the region's exclusive edge.
        assert_eq!(
            hit_test(at(1100.0, 845.0), &windows),
            hit("shadowed", "tabs")
        );
        assert_eq!(hit_test(at(1100.0, 846.0), &windows), None);
    }

    #[test]
    fn a_region_is_clipped_to_its_windows_content() {
        let windows = [window(
            "a",
            (0, 0),
            (500, 300),
            1.0,
            vec![region("wide", 400.0, 0.0, 400.0, 100.0)],
        )];
        assert_eq!(hit_test(at(450.0, 50.0), &windows), hit("a", "wide"));
        assert_eq!(hit_test(at(550.0, 50.0), &windows), None);
    }

    #[test]
    fn negative_coordinates_on_a_monitor_to_the_left() {
        let windows = [
            window(
                "left",
                (-1920, -100),
                (1000, 700),
                1.0,
                vec![region("r", 0.0, 0.0, 200.0, 50.0)],
            ),
            window(
                "main",
                (0, 0),
                (1000, 700),
                1.0,
                vec![region("r", 0.0, 0.0, 200.0, 50.0)],
            ),
        ];
        assert_eq!(hit_test(at(-1800.0, -90.0), &windows), hit("left", "r"));
        assert_eq!(hit_test(at(-1.0, -1.0), &windows), None);
        assert_eq!(hit_test(at(10.0, 10.0), &windows), hit("main", "r"));
    }

    #[test]
    fn mixed_scales_on_two_monitors() {
        // A 2x monitor at the origin and a 1x monitor to its right at x = 3840.
        let windows = [
            window(
                "hidpi",
                (100, 100),
                (2000, 1400),
                2.0,
                vec![region("strip", 0.0, 0.0, 500.0, 40.0)],
            ),
            window(
                "lodpi",
                (3900, 50),
                (1000, 700),
                1.0,
                vec![region("strip", 0.0, 0.0, 500.0, 40.0)],
            ),
        ];
        // 500 logical at 2x is 1000 physical wide; 40 logical is 80 tall.
        assert_eq!(hit_test(at(1099.0, 179.0), &windows), hit("hidpi", "strip"));
        assert_eq!(hit_test(at(1100.0, 150.0), &windows), None);
        assert_eq!(hit_test(at(4399.0, 89.0), &windows), hit("lodpi", "strip"));
        assert_eq!(hit_test(at(4400.0, 60.0), &windows), None);
        // Between the monitors' windows.
        assert_eq!(hit_test(at(3000.0, 60.0), &windows), None);
    }

    #[test]
    fn the_smallest_visible_area_wins_an_overlap() {
        let windows = [
            window(
                "back",
                (0, 0),
                (1000, 800),
                1.0,
                vec![region("page", 0.0, 0.0, 1000.0, 800.0)],
            ),
            window(
                "front",
                (100, 100),
                (400, 300),
                1.0,
                vec![region("strip", 0.0, 0.0, 400.0, 40.0)],
            ),
        ];
        assert_eq!(hit_test(at(150.0, 120.0), &windows), hit("front", "strip"));
        assert_eq!(hit_test(at(150.0, 200.0), &windows), hit("back", "page"));
        // Window order does not matter when the areas differ.
        let reversed = [windows[1].clone(), windows[0].clone()];
        assert_eq!(hit_test(at(150.0, 120.0), &reversed), hit("front", "strip"));
    }

    #[test]
    fn equal_areas_go_to_the_earlier_window_then_region() {
        let windows = [
            window(
                "first",
                (0, 0),
                (500, 500),
                1.0,
                vec![
                    region("a", 0.0, 0.0, 100.0, 100.0),
                    region("b", 0.0, 0.0, 100.0, 100.0),
                ],
            ),
            window(
                "second",
                (0, 0),
                (500, 500),
                1.0,
                vec![region("c", 0.0, 0.0, 100.0, 100.0)],
            ),
        ];
        assert_eq!(hit_test(at(50.0, 50.0), &windows), hit("first", "a"));
    }

    #[test]
    fn nothing_registered_or_nothing_under_the_cursor_is_no_hit() {
        assert_eq!(hit_test(at(0.0, 0.0), &[]), None);
        let empty = [window("a", (0, 0), (100, 100), 1.0, vec![])];
        assert_eq!(hit_test(at(5.0, 5.0), &empty), None);
        let degenerate = [window(
            "a",
            (0, 0),
            (100, 100),
            1.0,
            vec![region("zero", 10.0, 10.0, 0.0, 0.0)],
        )];
        assert_eq!(hit_test(at(10.0, 10.0), &degenerate), None);
    }
}
