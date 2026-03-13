use std::collections::BTreeMap;
use std::{f32, fs};

use bioma_npc_core::StateDiffRef;
use ggez::glam::Vec2;
use ggez::graphics::{Color, DrawMode, DrawParam, Mesh, Rect};

use crate::{
    output_path, screenshot::render_rgba_image, PostMCTSHookArgs, PostMCTSHookFn, WorldState,
    SPRITE_SIZE,
};

// TODO
pub fn heatmap_hook() -> PostMCTSHookFn {
    Box::new(
        move |PostMCTSHookArgs {
                  ctx,
                  assets,
                  run,
                  turn,
                  world,
                  agent,
                  mcts,
                  ..
              }| {
            if let Some(ctx) = ctx.as_deref_mut() {
                struct HeatMapEntry {
                    visits: usize,
                    score: f32,
                }

                let mut positions: BTreeMap<(isize, isize), HeatMapEntry> = BTreeMap::new();
                let mut max_visits = 0;
                let mut best_avg_score: f32 = 0.;
                let mut worst_avg_score: f32 = f32::MAX;

                // Get nodes for this agent
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
                            worst_avg_score =
                                worst_avg_score.min(entry.score / entry.visits as f32);
                            max_visits = max_visits.max(edge.visits());
                        }
                    })
                });

                // Heatmap
                {
                    let rect = Mesh::new_rectangle(
                        ctx,
                        DrawMode::fill(),
                        Rect::new(0.0, 0.0, SPRITE_SIZE, SPRITE_SIZE),
                        Color::WHITE,
                    )
                    .unwrap();

                    let flipped_image_data = render_rgba_image(ctx, Color::BLACK, |ctx, canvas| {
                        world.with_map_coordinates(canvas, |canvas| {
                            for (&(x, y), entry) in &positions {
                                let visits = entry.visits as f32 / max_visits as f32;

                                // Cull visits that are not significant to avoid outliers
                                if visits < 0.001 {
                                    continue;
                                }

                                let scores = (entry.score / entry.visits as f32 - worst_avg_score)
                                    / (best_avg_score - worst_avg_score + f32::EPSILON);

                                let mut green = scores;
                                let mut red = 1. - scores;
                                let max = red.max(green);

                                // Normalize to max
                                green /= max;
                                red /= max;

                                if visits > f32::EPSILON && scores > f32::EPSILON {
                                    canvas.draw(
                                        &rect,
                                        DrawParam::default()
                                            .dest(Vec2::new(
                                                x as f32 * SPRITE_SIZE,
                                                y as f32 * SPRITE_SIZE,
                                            ))
                                            .color(Color::new(red, green, 0.0, visits)),
                                    );
                                }
                            }
                        });

                        world.draw(ctx, canvas, assets);
                    });

                    fs::create_dir_all(format!(
                        "{}/{}/heatmaps/agent{}/",
                        output_path(),
                        run.map(|n| n.to_string()).unwrap_or_default(),
                        agent.0
                    ))
                    .unwrap();
                    flipped_image_data
                        .save(format!(
                            "{}/{}/heatmaps/agent{}/{:06}.png",
                            output_path(),
                            run.map(|n| n.to_string()).unwrap_or_default(),
                            agent.0,
                            turn
                        ))
                        .unwrap();
                }
            }
        },
    )
}
