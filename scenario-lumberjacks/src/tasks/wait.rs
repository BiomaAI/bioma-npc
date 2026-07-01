use std::hash::Hash;

use bioma_npc_core::{
    Context, ContextMut, Domain, IdleTask, Task, TaskDuration, impl_task_boxed_methods,
};

use crate::{Action, Lumberjacks, WorldStateMut, config};

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Wait;

impl Task<Lumberjacks> for Wait {
    fn weight(&self, _ctx: Context<Lumberjacks>) -> f32 {
        config().action_weights.wait
    }

    fn duration(&self, _ctx: Context<Lumberjacks>) -> TaskDuration {
        0
    }

    fn execute(&self, ctx: ContextMut<Lumberjacks>) -> Option<Box<dyn Task<Lumberjacks>>> {
        let ContextMut { mut state_diff, .. } = ctx;
        state_diff.increment_time();

        Some(Box::new(IdleTask))
    }

    fn display_action(&self) -> <Lumberjacks as Domain>::DisplayAction {
        Action::Wait
    }

    fn is_valid(&self, _ctx: Context<Lumberjacks>) -> bool {
        true
    }

    impl_task_boxed_methods!(Lumberjacks);
}
