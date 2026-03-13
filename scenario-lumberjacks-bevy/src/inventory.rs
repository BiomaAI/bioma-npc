use std::collections::BTreeMap;
use std::mem;

use serde::Serialize;

use bioma_npc_core::AgentId;

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize)]
pub struct Inventory(pub BTreeMap<AgentId, AgentInventory>);

#[derive(Clone, Default, Debug, PartialEq, Eq, Serialize, Hash)]
pub struct AgentInventory {
    pub wood: isize,
    pub water: bool,
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
