use crate::{Lumberjacks, WorldState};
use bioma_npc_core::{AgentId, StateDiffRef};

pub(crate) fn minimalist(state: StateDiffRef<Lumberjacks>, _agent: AgentId) -> f32 {
    -(state.trees().len() as f32)
}
