use std::collections::BTreeMap;
use std::mem;

use ggez::glam::Vec2;
use ggez::graphics::{Canvas, Color, DrawParam, Image, Text};
use ggez::Context;
use serde::Serialize;

use bioma_npc_core::AgentId;

use crate::SPRITE_SIZE;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize)]
pub struct Inventory(pub BTreeMap<AgentId, AgentInventory>);

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Hash)]
pub struct AgentInventory {
    pub wood: isize,
    pub water: bool,
}

impl Inventory {
    pub fn draw(&self, ctx: &Context, canvas: &mut Canvas, assets: &BTreeMap<String, Image>) {
        let mut agents = self
            .0
            .iter()
            .map(|(k, v)| (*k, v.wood, v.water))
            .collect::<Vec<(AgentId, isize, bool)>>();

        agents.sort_by_key(|(k, ..)| *k);

        for (i, (agent, wood, water)) in agents.iter().enumerate() {
            let sprite_name = if agent.0 % 2 == 0 {
                "OrangeRight".to_owned()
            } else {
                "YellowRight".to_owned()
            };

            canvas.draw(
                assets.get(&sprite_name).unwrap(),
                DrawParam::default()
                    .dest(Vec2::new(0.0, i as f32 * SPRITE_SIZE))
                    .color(Color::WHITE),
            );

            let mut text = Text::new(format!(":{}, {}", wood, water));
            text.set_scale(SPRITE_SIZE * 0.6);
            let text_height = text.measure(ctx).unwrap().y;
            canvas.draw(
                &text,
                DrawParam::default().dest(Vec2::new(
                    SPRITE_SIZE,
                    i as f32 * SPRITE_SIZE + (SPRITE_SIZE - text_height) / 2.,
                )),
            );
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct InventorySnapshot(pub BTreeMap<AgentId, AgentInventory>);

#[derive(Clone, Debug, Default, Hash, PartialEq, Eq)]
pub struct InventoryDiff(pub BTreeMap<AgentId, AgentInventory>);

impl InventoryDiff {
    pub fn diff_size(&self) -> usize {
        self.0.len() * mem::size_of::<(AgentId, AgentInventory)>()
    }
}
