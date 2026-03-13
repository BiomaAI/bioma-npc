use std::hash::Hash;
use std::num::NonZeroU8;

use bioma_npc_core::{
    impl_task_boxed_methods, Context, ContextMut, Domain, IdleTask, Task, TaskDuration,
};
use bioma_npc_utils::{Direction, DIRECTIONS};

use crate::{apply_direction, config, Action, Lumberjacks, Tile, WorldState, WorldStateMut};

const NON_ZERO_U8_1: NonZeroU8 = NonZeroU8::new(1).unwrap();

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Chop {
    pub direction: Direction,
}

impl Task<Lumberjacks> for Chop {
    fn weight(&self, _ctx: Context<Lumberjacks>) -> f32 {
        config().action_weights.chop
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

        if let Some((x, y)) = state_diff.find_agent(agent) {
            let (x, y) = apply_direction(self.direction, x, y);

            match state_diff.get_tile_ref_mut(x, y) {
                Some(tile @ Tile::Tree(NON_ZERO_U8_1)) => {
                    *tile = Tile::Empty;
                }
                Some(Tile::Tree(height)) => {
                    *height = NonZeroU8::new(height.get() - 1).unwrap();
                }
                _ => return Some(Box::new(IdleTask)),
            }

            if config().features.teamwork {
                for direction in DIRECTIONS {
                    let (x, y) = apply_direction(direction, x, y);
                    if let Some(Tile::Agent(agent)) = state_diff.get_tile(x, y) {
                        state_diff.increment_inventory(agent);
                    }
                }
            } else {
                state_diff.increment_inventory(agent);
            }

            Some(Box::new(IdleTask))
        } else {
            unreachable!("Could not find agent on map!")
        }
    }

    fn display_action(&self) -> <Lumberjacks as Domain>::DisplayAction {
        Action::Chop(self.direction)
    }

    fn is_valid(&self, ctx: Context<Lumberjacks>) -> bool {
        let Context {
            state_diff, agent, ..
        } = ctx;
        if let Some((x, y)) = state_diff.find_agent(agent) {
            let (x, y) = apply_direction(self.direction, x, y);
            matches!(state_diff.get_tile(x, y), Some(Tile::Tree(_)))
        } else {
            unreachable!("Could not find agent on map!")
        }
    }

    impl_task_boxed_methods!(Lumberjacks);
}
