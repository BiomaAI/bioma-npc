use bioma_npc_core::{AgentId, Behavior, Context, Task};

use crate::{domain::EcosystemDomain, task::world::WorldStep};

pub const WORLD_AGENT_ID: AgentId = AgentId(u32::MAX);

pub struct WorldBehavior;
impl Behavior<EcosystemDomain> for WorldBehavior {
    fn add_own_tasks(
        &self,
        _ctx: Context<EcosystemDomain>,
        tasks: &mut Vec<Box<dyn Task<EcosystemDomain>>>,
    ) {
        tasks.push(Box::new(WorldStep));
    }

    fn is_valid(&self, ctx: Context<EcosystemDomain>) -> bool {
        ctx.agent == WORLD_AGENT_ID
    }
}
