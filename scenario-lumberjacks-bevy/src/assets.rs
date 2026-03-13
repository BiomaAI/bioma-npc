use std::collections::BTreeMap;
use std::path::PathBuf;

use bioma_npc_core::AgentId;

use crate::{Action, Tile};

pub const SPRITE_FILES: &[(&str, &str)] = &[
    ("ImpassableRock", "scenario-lumberjacks/assets/ImpassableRock.png"),
    ("OrangeDown", "scenario-lumberjacks/assets/OrangeDown.png"),
    (
        "OrangeDownBarrier",
        "scenario-lumberjacks/assets/OrangeDownBarrier.png",
    ),
    (
        "OrangeDownChopping",
        "scenario-lumberjacks/assets/OrangeDownChopping.png",
    ),
    ("OrangeLeft", "scenario-lumberjacks/assets/OrangeLeft.png"),
    (
        "OrangeLeftBarrier",
        "scenario-lumberjacks/assets/OrangeLeftBarrier.png",
    ),
    (
        "OrangeLeftChopping",
        "scenario-lumberjacks/assets/OrangeLeftChopping.png",
    ),
    ("OrangeRight", "scenario-lumberjacks/assets/OrangeRight.png"),
    (
        "OrangeRightBarrier",
        "scenario-lumberjacks/assets/OrangeRightBarrier.png",
    ),
    (
        "OrangeRightChopping",
        "scenario-lumberjacks/assets/OrangeRightChopping.png",
    ),
    ("OrangeTop", "scenario-lumberjacks/assets/OrangeTop.png"),
    (
        "OrangeTopBarrier",
        "scenario-lumberjacks/assets/OrangeTopBarrier.png",
    ),
    (
        "OrangeTopChopping",
        "scenario-lumberjacks/assets/OrangeTopChopping.png",
    ),
    ("Tree1_3", "scenario-lumberjacks/assets/Tree1_3.png"),
    ("Tree2_3", "scenario-lumberjacks/assets/Tree2_3.png"),
    ("Tree3_3", "scenario-lumberjacks/assets/Tree3_3.png"),
    ("TreeSapling", "scenario-lumberjacks/assets/TreeSapling.png"),
    ("Well", "scenario-lumberjacks/assets/Well.png"),
    ("WoodenBarrier", "scenario-lumberjacks/assets/WoodenBarrier.png"),
    ("YellowDown", "scenario-lumberjacks/assets/YellowDown.png"),
    (
        "YellowDownBarrier",
        "scenario-lumberjacks/assets/YellowDownBarrier.png",
    ),
    (
        "YellowDownChopping",
        "scenario-lumberjacks/assets/YellowDownChopping.png",
    ),
    ("YellowLeft", "scenario-lumberjacks/assets/YellowLeft.png"),
    (
        "YellowLeftBarrier",
        "scenario-lumberjacks/assets/YellowLeftBarrier.png",
    ),
    (
        "YellowLeftChopping",
        "scenario-lumberjacks/assets/YellowLeftChopping.png",
    ),
    ("YellowRight", "scenario-lumberjacks/assets/YellowRight.png"),
    (
        "YellowRightBarrier",
        "scenario-lumberjacks/assets/YellowRightBarrier.png",
    ),
    (
        "YellowRightChopping",
        "scenario-lumberjacks/assets/YellowRightChopping.png",
    ),
    ("YellowTop", "scenario-lumberjacks/assets/YellowTop.png"),
    (
        "YellowTopBarrier",
        "scenario-lumberjacks/assets/YellowTopBarrier.png",
    ),
    (
        "YellowTopChopping",
        "scenario-lumberjacks/assets/YellowTopChopping.png",
    ),
];

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate lives under workspace root")
        .to_path_buf()
}

pub fn sprite_path(name: &str) -> &'static str {
    SPRITE_FILES
        .iter()
        .find_map(|(sprite, path)| (*sprite == name).then_some(*path))
        .unwrap_or_else(|| panic!("unknown sprite: {name}"))
}

pub fn sprite_name_for_tile(
    tile: &Tile,
    actions: &BTreeMap<AgentId, Action>,
) -> Option<String> {
    match tile {
        Tile::Agent(agent) if actions.contains_key(agent) => Some(format!(
            "{}{}",
            if agent.0 % 2 == 0 { "Orange" } else { "Yellow" },
            actions.get(agent).unwrap().sprite_name(),
        )),
        Tile::Tree(height) => Some(format!("Tree{}_3", height.get().min(3))),
        Tile::Agent(agent) => Some(if agent.0 % 2 == 0 {
            "OrangeRight".to_owned()
        } else {
            "YellowRight".to_owned()
        }),
        Tile::Barrier => Some("WoodenBarrier".to_owned()),
        Tile::Impassable => Some("ImpassableRock".to_owned()),
        Tile::Well => Some("Well".to_owned()),
        Tile::Empty => None,
    }
}

pub fn inventory_sprite_name(agent: AgentId) -> &'static str {
    if agent.0.is_multiple_of(2) {
        "OrangeRight"
    } else {
        "YellowRight"
    }
}
