use std::hash::Hash;

use bioma_npc_core::{
    Context, ContextMut, Domain, IdleTask, Task, TaskDuration, impl_task_boxed_methods,
};
use bioma_npc_utils::DIRECTIONS;

use crate::{Action, Lumberjacks, Tile, WorldState, WorldStateMut, apply_direction, config};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Refill;

impl Task<Lumberjacks> for Refill {
    fn weight(&self, _ctx: Context<Lumberjacks>) -> f32 {
        config().action_weights.refill
    }

    fn duration(&self, _ctx: Context<Lumberjacks>) -> TaskDuration {
        0
    }

    fn execute(&self, ctx: ContextMut<Lumberjacks>) -> Option<Box<dyn Task<Lumberjacks>>> {
        let ContextMut {
            mut state_diff,
            agent,
            ..
        } = ctx;
        state_diff.increment_time();

        state_diff.set_water(agent, true);
        Some(Box::new(IdleTask))
    }

    fn display_action(&self) -> <Lumberjacks as Domain>::DisplayAction {
        Action::Refill
    }

    fn is_valid(&self, ctx: Context<Lumberjacks>) -> bool {
        let Context {
            state_diff, agent, ..
        } = ctx;
        if let Some((x, y)) = state_diff.find_agent(agent) {
            !state_diff.get_water(agent)
                && DIRECTIONS.iter().any(|direction| {
                    let (x, y) = apply_direction(*direction, x, y);
                    matches!(state_diff.get_tile(x, y), Some(Tile::Well))
                })
        } else {
            unreachable!("Failed to find agent on map");
        }
    }

    impl_task_boxed_methods!(Lumberjacks);
}
