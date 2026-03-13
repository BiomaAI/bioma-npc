use bioma_npc_core::{Behavior, Context, IdleTask, Task};
use bioma_npc_utils::DIRECTIONS;

use crate::{domain::EcosystemDomain, state::Access, task::r#move::Move};

pub struct Animal;

impl Behavior<EcosystemDomain> for Animal {
    fn add_own_tasks(
        &self,
        ctx: Context<EcosystemDomain>,
        tasks: &mut Vec<Box<dyn Task<EcosystemDomain>>>,
    ) {
        for direction in DIRECTIONS {
            let task = Move(direction);
            if task.is_valid(ctx) {
                tasks.push(Box::new(task));
            }
        }
        tasks.push(Box::new(IdleTask));
    }

    fn is_valid(&self, ctx: Context<EcosystemDomain>) -> bool {
        ctx.state_diff
            .get_agent(ctx.agent)
            .filter(|agent_state| {
                // debug_assert!(agent_state.alive, "Behavior validity check called on a dead agent");
                agent_state.alive()
            })
            .is_some()
    }
}
