//! 2D pixel-map grid (BUILD_PLAN Phase 7): a rectangular grid of cells, each
//! optionally assigned to a patched fixture, driven by a built-in generator
//! (colour chase, rainbow, wave, plasma, noise) computed per cell every
//! tick. A `PixelMap` bundles its layout and generator together (unlike
//! `effects.rs`'s template/instance split) — a pixel map's whole point is
//! *this* grid running *this* generator, and "the same layout with a
//! different generator" is just editing the one field, not a second
//! instance.
//!
//! Deliberately does **not** implement image/GIF/video playback onto the
//! grid or a sandboxed scriptable generator (BUILD_PLAN also lists these) —
//! decoding media formats and safely sandboxing arbitrary JS are each a
//! substantial subsystem on their own, not a natural extension of "a
//! built-in generator computes a color per cell"; see DECISIONS.md.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::effects::pseudo_random;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum Axis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub enum PixelGenerator {
    /// Marches `colors` across the grid (row-major cell order), one step
    /// per beat.
    #[serde(rename_all = "camelCase")]
    ColourChase { colors: Vec<Rgb>, bpm: f32 },
    /// A hue gradient across the grid's width that scrolls over time.
    #[serde(rename_all = "camelCase")]
    Rainbow { bpm: f32, cycles: f32 },
    /// A brightness wave sweeping along `axis`, modulating `color`.
    #[serde(rename_all = "camelCase")]
    Wave { bpm: f32, axis: Axis, color: Rgb },
    /// The classic layered-sines plasma, mapped through a hue wheel.
    #[serde(rename_all = "camelCase")]
    Plasma { speed: f32 },
    /// Per-cell flicker of `color`, re-rolled a few times a second.
    #[serde(rename_all = "camelCase")]
    Noise { speed: f32, color: Rgb },
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "../../src/lib/showfile/generated/")]
pub struct PixelMap {
    pub id: u32,
    pub name: String,
    pub width: u32,
    pub height: u32,
    /// Row-major, length `width * height`; `None` = an empty cell.
    pub cells: Vec<Option<u32>>,
    pub generator: PixelGenerator,
}

impl PixelMap {
    fn new(id: u32, name: String, width: u32, height: u32) -> Self {
        Self {
            id,
            name,
            width,
            height,
            cells: vec![None; (width * height) as usize],
            generator: PixelGenerator::Rainbow { bpm: 60.0, cycles: 1.0 },
        }
    }

    pub fn set_cell(&mut self, x: u32, y: u32, fixture_number: Option<u32>) -> Result<(), String> {
        let index = self.cell_index(x, y)?;
        self.cells[index] = fixture_number;
        Ok(())
    }

    fn cell_index(&self, x: u32, y: u32) -> Result<usize, String> {
        if x >= self.width || y >= self.height {
            return Err(format!("Cell ({x}, {y}) is outside the {}x{} grid", self.width, self.height));
        }
        Ok((y * self.width + x) as usize)
    }
}

#[derive(Default)]
pub struct PixelMapStore {
    maps: HashMap<u32, PixelMap>,
    next_id: u32,
}

impl PixelMapStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn create(&mut self, name: String, width: u32, height: u32) -> u32 {
        self.next_id += 1;
        let id = self.next_id;
        self.maps.insert(id, PixelMap::new(id, name, width.max(1), height.max(1)));
        id
    }

    pub fn get(&self, id: u32) -> Option<&PixelMap> {
        self.maps.get(&id)
    }

    pub fn get_mut(&mut self, id: u32) -> Option<&mut PixelMap> {
        self.maps.get_mut(&id)
    }

    pub fn delete(&mut self, id: u32) {
        self.maps.remove(&id);
    }

    pub fn list(&self) -> Vec<PixelMap> {
        let mut maps: Vec<PixelMap> = self.maps.values().cloned().collect();
        maps.sort_by_key(|m| m.id);
        maps
    }
}

/// HSV (h: 0.0-1.0, s/v: 0.0-1.0) -> RGB (0.0-1.0 each). A small local
/// implementation rather than pulling in a color crate for one function —
/// the frontend has its own copy (`src/lib/color.ts`) for the same reason
/// noted there: this is well-known, unit-tested conversion math, not a
/// shared dependency worth introducing for either side.
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> Rgb {
    let h = h.rem_euclid(1.0) * 6.0;
    let i = h.floor() as i32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i.rem_euclid(6) {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Rgb { r, g, b }
}

/// One running instance of a `PixelMap`'s generator.
pub struct PixelMapRuntime {
    pub id: u32,
    pub pixelmap_id: u32,
    pub priority: i64,
    elapsed_ms: f64,
}

impl PixelMapRuntime {
    pub fn new(id: u32, pixelmap_id: u32, priority: i64) -> Self {
        Self { id, pixelmap_id, priority, elapsed_ms: 0.0 }
    }

    pub fn tick(&mut self, dt_ms: f64) {
        self.elapsed_ms += dt_ms;
    }

    /// This tick's color for every cell, row-major (matching `PixelMap.cells`).
    pub fn render(&self, map: &PixelMap) -> Vec<Rgb> {
        let t = self.elapsed_ms as f32 / 1000.0;
        let (width, height) = (map.width.max(1) as f32, map.height.max(1) as f32);

        (0..map.cells.len())
            .map(|index| {
                let x = (index as u32 % map.width) as f32;
                let y = (index as u32 / map.width) as f32;
                match &map.generator {
                    PixelGenerator::ColourChase { colors, bpm } => {
                        if colors.is_empty() {
                            return Rgb { r: 0.0, g: 0.0, b: 0.0 };
                        }
                        let beats = t * bpm / 60.0;
                        let position = (index as f32 - beats).floor() as i64;
                        colors[position.rem_euclid(colors.len() as i64) as usize]
                    }
                    PixelGenerator::Rainbow { bpm, cycles } => {
                        let scroll = t * bpm / 60.0;
                        let hue = (x / width) * cycles + scroll;
                        hsv_to_rgb(hue, 1.0, 1.0)
                    }
                    PixelGenerator::Wave { bpm, axis, color } => {
                        let position = match axis {
                            Axis::Horizontal => x / width,
                            Axis::Vertical => y / height,
                        };
                        let phase = position - t * bpm / 60.0;
                        let brightness = (1.0 - (phase * std::f32::consts::TAU).cos()) / 2.0;
                        Rgb { r: color.r * brightness, g: color.g * brightness, b: color.b * brightness }
                    }
                    PixelGenerator::Plasma { speed } => {
                        let time = t * speed;
                        let value = (x * 0.2 + time).sin()
                            + (y * 0.2 + time).sin()
                            + ((x + y) * 0.2 + time).sin()
                            + ((x * x + y * y).sqrt() * 0.15 - time).sin();
                        hsv_to_rgb((value / 4.0 + 1.0) / 2.0, 1.0, 1.0)
                    }
                    PixelGenerator::Noise { speed, color } => {
                        let bucket = (t * speed.max(0.01)).floor() as i64;
                        let brightness = pseudo_random(bucket.wrapping_mul(7919).wrapping_add(index as i64));
                        Rgb { r: color.r * brightness, g: color.g * brightness, b: color.b * brightness }
                    }
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_cell_rejects_out_of_bounds_coordinates() {
        let mut map = PixelMap::new(1, "Grid".to_string(), 4, 4);
        assert!(map.set_cell(3, 3, Some(1)).is_ok());
        assert!(map.set_cell(4, 0, Some(1)).is_err());
        assert!(map.set_cell(0, 4, Some(1)).is_err());
    }

    #[test]
    fn hsv_to_rgb_matches_known_primary_colors() {
        let red = hsv_to_rgb(0.0, 1.0, 1.0);
        assert!((red.r - 1.0).abs() < 0.01 && red.g < 0.01 && red.b < 0.01);
        let green = hsv_to_rgb(1.0 / 3.0, 1.0, 1.0);
        assert!(green.r < 0.01 && (green.g - 1.0).abs() < 0.01 && green.b < 0.01);
        let blue = hsv_to_rgb(2.0 / 3.0, 1.0, 1.0);
        assert!(blue.r < 0.01 && blue.g < 0.01 && (blue.b - 1.0).abs() < 0.01);
    }

    #[test]
    fn rainbow_spreads_hue_across_the_grid_width_at_a_single_instant() {
        let mut map = PixelMap::new(1, "Grid".to_string(), 4, 1);
        map.generator = PixelGenerator::Rainbow { bpm: 0.0, cycles: 1.0 }; // bpm 0 -> frozen in time
        let runtime = PixelMapRuntime::new(1, 1, 1);
        let rendered = runtime.render(&map);
        // Different columns should get different hues (not all identical).
        assert_ne!(rendered[0], rendered[1]);
        assert_ne!(rendered[1], rendered[2]);
    }

    #[test]
    fn colour_chase_marches_forward_over_time() {
        let mut map = PixelMap::new(1, "Grid".to_string(), 3, 1);
        map.generator = PixelGenerator::ColourChase {
            colors: vec![Rgb { r: 1.0, g: 0.0, b: 0.0 }, Rgb { r: 0.0, g: 1.0, b: 0.0 }, Rgb { r: 0.0, g: 0.0, b: 1.0 }],
            bpm: 60.0, // 1 beat/sec -> 1 step/sec
        };
        let mut runtime = PixelMapRuntime::new(1, 1, 1);
        let at_start = runtime.render(&map);
        runtime.tick(1000.0); // one full second = one full step
        let after_one_step = runtime.render(&map);
        assert_ne!(at_start, after_one_step);
        // Cell 1 after one step shows what cell 0 showed at t=0 (chase moved
        // the pattern forward by exactly one cell).
        assert_eq!(after_one_step[1], at_start[0]);
    }

    #[test]
    fn wave_oscillates_brightness_between_zero_and_the_full_color() {
        let mut map = PixelMap::new(1, "Grid".to_string(), 1, 1);
        map.generator = PixelGenerator::Wave { bpm: 60.0, axis: Axis::Horizontal, color: Rgb { r: 1.0, g: 1.0, b: 1.0 } };
        let mut runtime = PixelMapRuntime::new(1, 1, 1);
        let trough = runtime.render(&map)[0];
        assert!(trough.r < 0.01);
        runtime.tick(500.0); // half a beat at 60bpm = half the wave's cycle
        let peak = runtime.render(&map)[0];
        assert!(peak.r > 0.99);
    }

    #[test]
    fn plasma_and_noise_vary_across_cells_instead_of_being_uniform() {
        let mut map = PixelMap::new(1, "Grid".to_string(), 4, 4);
        map.generator = PixelGenerator::Plasma { speed: 1.0 };
        let runtime = PixelMapRuntime::new(1, 1, 1);
        let plasma = runtime.render(&map);
        assert!(plasma.iter().any(|c| *c != plasma[0]), "plasma should not render one flat color");

        map.generator = PixelGenerator::Noise { speed: 1.0, color: Rgb { r: 1.0, g: 1.0, b: 1.0 } };
        let noise = runtime.render(&map);
        assert!(noise.iter().any(|c| *c != noise[0]), "noise should not render one flat color");
    }
}
