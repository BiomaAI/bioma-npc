use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::OnceLock;

use bioma_npc_core::{AgentId, StateDiffRef, MCTS};
use image::{GenericImage, Rgba, RgbaImage};
use crate::{config, output_path, Lumberjacks, WorldGlobalState, WorldState, SPRITE_SIZE};

use crate::assets::{inventory_sprite_name, sprite_name_for_tile, workspace_root, SPRITE_FILES};

fn sprite_cache() -> &'static BTreeMap<&'static str, RgbaImage> {
    static CACHE: OnceLock<BTreeMap<&'static str, RgbaImage>> = OnceLock::new();
    CACHE.get_or_init(|| {
        SPRITE_FILES
            .iter()
            .map(|(name, path)| {
                let image = image::open(workspace_root().join(path))
                    .unwrap_or_else(|err| panic!("failed to load sprite {path}: {err}"))
                    .to_rgba8();
                (*name, image)
            })
            .collect()
    })
}

fn background_color() -> Rgba<u8> {
    let bg = config().display.background;
    Rgba([
        (bg.0.clamp(0.0, 1.0) * 255.0).round() as u8,
        (bg.1.clamp(0.0, 1.0) * 255.0).round() as u8,
        (bg.2.clamp(0.0, 1.0) * 255.0).round() as u8,
        255,
    ])
}

fn sprite_size() -> u32 {
    SPRITE_SIZE.round() as u32
}

fn ensure_parent(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
}

fn blend(base: &mut Rgba<u8>, overlay: Rgba<u8>) {
    let alpha = overlay[3] as f32 / 255.0;
    let inv_alpha = 1.0 - alpha;
    for idx in 0..3 {
        base[idx] = (base[idx] as f32 * inv_alpha + overlay[idx] as f32 * alpha).round() as u8;
    }
}

fn draw_overlay_rect(image: &mut RgbaImage, left: u32, top: u32, size: u32, color: Rgba<u8>) {
    for y in top..top.saturating_add(size).min(image.height()) {
        for x in left..left.saturating_add(size).min(image.width()) {
            let pixel = image.get_pixel_mut(x, y);
            blend(pixel, color);
        }
    }
}

fn glyph_rows(ch: char) -> Option<[u8; 7]> {
    match ch {
        '0' => Some([0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110]),
        '1' => Some([0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        '2' => Some([0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111]),
        '3' => Some([0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110]),
        '4' => Some([0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010]),
        '5' => Some([0b11111, 0b10000, 0b10000, 0b11110, 0b00001, 0b00001, 0b11110]),
        '6' => Some([0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110]),
        '7' => Some([0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000]),
        '8' => Some([0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110]),
        '9' => Some([0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b11100]),
        ':' => Some([0b00000, 0b00100, 0b00100, 0b00000, 0b00100, 0b00100, 0b00000]),
        ',' => Some([0b00000, 0b00000, 0b00000, 0b00000, 0b00110, 0b00100, 0b01000]),
        '-' => Some([0b00000, 0b00000, 0b00000, 0b11111, 0b00000, 0b00000, 0b00000]),
        ' ' => Some([0, 0, 0, 0, 0, 0, 0]),
        'a' => Some([0b00000, 0b00000, 0b01110, 0b00001, 0b01111, 0b10001, 0b01111]),
        'e' => Some([0b00000, 0b00000, 0b01110, 0b10001, 0b11111, 0b10000, 0b01110]),
        'f' => Some([0b00110, 0b01000, 0b01000, 0b11100, 0b01000, 0b01000, 0b01000]),
        'l' => Some([0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110]),
        'r' => Some([0b00000, 0b00000, 0b10110, 0b11001, 0b10000, 0b10000, 0b10000]),
        's' => Some([0b00000, 0b00000, 0b01111, 0b10000, 0b01110, 0b00001, 0b11110]),
        't' => Some([0b00100, 0b00100, 0b11111, 0b00100, 0b00100, 0b00101, 0b00010]),
        'u' => Some([0b00000, 0b00000, 0b10001, 0b10001, 0b10001, 0b10011, 0b01101]),
        _ => None,
    }
}

fn draw_bitmap_text(
    image: &mut RgbaImage,
    left: u32,
    top: u32,
    scale: u32,
    color: Rgba<u8>,
    text: &str,
) {
    let scale = scale.max(1);
    let mut cursor_x = left;

    for ch in text.chars() {
        let Some(rows) = glyph_rows(ch) else {
            cursor_x += 6 * scale;
            continue;
        };

        for (row, bits) in rows.iter().enumerate() {
            for col in 0..5 {
                if bits & (1 << (4 - col)) == 0 {
                    continue;
                }

                let x = cursor_x + col as u32 * scale;
                let y = top + row as u32 * scale;
                for py in y..(y + scale).min(image.height()) {
                    for px in x..(x + scale).min(image.width()) {
                        *image.get_pixel_mut(px, py) = color;
                    }
                }
            }
        }

        cursor_x += 6 * scale;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct HeatmapCell {
    pub x: isize,
    pub y: isize,
    pub red: f32,
    pub green: f32,
    pub alpha: f32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct HeatmapOverlay {
    pub cells: Vec<HeatmapCell>,
}

impl HeatmapOverlay {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

pub fn render_world_image(world: &WorldGlobalState) -> RgbaImage {
    let sprite_size = sprite_size();
    let padding = config().display.padding;
    let width = (world.map.width + 2 * padding.0) as u32 * sprite_size;
    let height = (world.map.height + 2 * padding.1.max(world.inventory.0.len())) as u32 * sprite_size;
    let mut image = RgbaImage::from_pixel(width.max(1), height.max(1), background_color());
    let cache = sprite_cache();

    if config().display.inventory {
        let mut agents = world
            .inventory
            .0
            .iter()
            .map(|(agent, inventory)| (*agent, inventory.wood, inventory.water))
            .collect::<Vec<_>>();
        agents.sort_by_key(|(agent, ..)| *agent);
        for (row, (agent, wood, water)) in agents.into_iter().enumerate() {
            let sprite = cache
                .get(inventory_sprite_name(agent))
                .expect("inventory sprite present");
            image.copy_from(sprite, 0, row as u32 * sprite_size).unwrap();
            let text = format!(":{wood}, {water}");
            let text_height = 7 * 3;
            let text_top = row as u32 * sprite_size + (sprite_size.saturating_sub(text_height)) / 2;
            draw_bitmap_text(
                &mut image,
                sprite_size,
                text_top,
                3,
                Rgba([255, 255, 255, 255]),
                &text,
            );
        }
    }

    let offset_x = padding.0 as u32 * sprite_size;
    let offset_y = padding.1 as u32 * sprite_size;

    for (row, tiles) in world.map.tiles.iter().enumerate() {
        for (col, tile) in tiles.iter().enumerate() {
            let Some(sprite_name) = sprite_name_for_tile(tile, &world.actions) else {
                continue;
            };
            let sprite = cache
                .get(sprite_name.as_str())
                .unwrap_or_else(|| panic!("missing sprite data for {}", sprite_name));
            image
                .copy_from(
                    sprite,
                    offset_x + col as u32 * sprite_size,
                    offset_y + row as u32 * sprite_size,
                )
                .unwrap();
        }
    }

    image
}

pub fn save_world_png(world: &WorldGlobalState, path: impl AsRef<Path>) {
    let path = path.as_ref();
    ensure_parent(path);
    render_world_image(world).save(path).unwrap();
}

pub fn save_turn_screenshot(world: &WorldGlobalState, run: Option<usize>, turn: usize) {
    save_world_png(
        world,
        Path::new(output_path())
            .join(run.map(|n| n.to_string()).unwrap_or_default())
            .join("screenshots")
            .join(format!("turn{:06}.png", turn)),
    );
}

pub fn build_heatmap_overlay(
    agent: AgentId,
    mcts: &MCTS<Lumberjacks>,
) -> Option<HeatmapOverlay> {
    struct HeatMapEntry {
        visits: usize,
        score: f32,
    }

    let mut positions: BTreeMap<(isize, isize), HeatMapEntry> = BTreeMap::new();
    let mut max_visits = 0usize;
    let mut best_avg_score = 0.0f32;
    let mut worst_avg_score = f32::MAX;

    mcts.nodes().for_each(|(_, edges)| {
        edges.into_iter().for_each(|(_, edge)| {
            let edge = edge.lock().unwrap();
            let child = edge.child();
            if child.agent() == agent {
                let state_diff = StateDiffRef::new(mcts.initial_state(), child.diff());
                let (x, y) = state_diff.find_agent(mcts.agent()).unwrap();

                let visits = edge.visits();
                let score = edge.q_value(mcts.agent());
                let entry = positions
                    .entry((x, y))
                    .and_modify(|entry| {
                        entry.visits += visits;
                        entry.score += score;
                    })
                    .or_insert(HeatMapEntry { visits, score });

                best_avg_score = best_avg_score.max(entry.score / entry.visits as f32);
                worst_avg_score = worst_avg_score.min(entry.score / entry.visits as f32);
                max_visits = max_visits.max(entry.visits);
            }
        })
    });

    if max_visits == 0 {
        return None;
    }

    let mut overlay = HeatmapOverlay::default();

    for (&(x, y), entry) in &positions {
        let visits = entry.visits as f32 / max_visits as f32;
        if visits < 0.001 {
            continue;
        }

        let scores = (entry.score / entry.visits as f32 - worst_avg_score)
            / (best_avg_score - worst_avg_score + f32::EPSILON);

        let mut green = scores;
        let mut red = 1.0 - scores;
        let max = red.max(green).max(f32::EPSILON);
        green /= max;
        red /= max;

        overlay.cells.push(HeatmapCell {
            x,
            y,
            red: red.clamp(0.0, 1.0),
            green: green.clamp(0.0, 1.0),
            alpha: visits.clamp(0.0, 1.0),
        });
    }

    if overlay.is_empty() {
        None
    } else {
        Some(overlay)
    }
}

pub fn save_heatmap_overlay(
    world: &WorldGlobalState,
    path: impl AsRef<Path>,
    overlay: &HeatmapOverlay,
) {
    let sprite_size = sprite_size();
    let padding = config().display.padding;
    let offset_x = padding.0 as u32 * sprite_size;
    let offset_y = padding.1 as u32 * sprite_size;
    let mut image = render_world_image(world);

    for cell in &overlay.cells {
        let color = Rgba([
            (cell.red * 255.0).round() as u8,
            (cell.green * 255.0).round() as u8,
            0,
            (cell.alpha * 255.0).round() as u8,
        ]);

        draw_overlay_rect(
            &mut image,
            offset_x + cell.x.max(0) as u32 * sprite_size,
            offset_y + cell.y.max(0) as u32 * sprite_size,
            sprite_size,
            color,
        );
    }

    let path = path.as_ref();
    ensure_parent(path);
    image.save(path).unwrap();
}

pub fn save_heatmap(
    world: &WorldGlobalState,
    run: Option<usize>,
    turn: usize,
    agent: AgentId,
    mcts: &MCTS<Lumberjacks>,
) {
    let Some(overlay) = build_heatmap_overlay(agent, mcts) else {
        return;
    };

    let path = Path::new(output_path())
        .join(run.map(|n| n.to_string()).unwrap_or_default())
        .join("heatmaps")
        .join(format!("agent{}", agent.0))
        .join(format!("{:06}.png", turn));
    save_heatmap_overlay(world, path, &overlay);
}
