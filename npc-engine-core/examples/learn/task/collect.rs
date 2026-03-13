use bioma_npc_core::{impl_task_boxed_methods, Context, ContextMut, Task, TaskDuration};
use bioma_npc_utils::OptionDiffDomain;

use crate::domain::{DisplayAction, LearnDomain};

#[derive(Clone, Hash, PartialEq, Eq, Debug)]
pub struct Collect;
impl Task<LearnDomain> for Collect {
    fn duration(&self, _ctx: Context<LearnDomain>) -> TaskDuration {
        1
    }

    fn execute(&self, ctx: ContextMut<LearnDomain>) -> Option<Box<dyn Task<LearnDomain>>> {
        let state = LearnDomain::get_cur_state_mut(ctx.state_diff);
        debug_assert!(state.map[state.agent_pos as usize] > 0);
        state.map[state.agent_pos as usize] -= 1;
        state.wood_count += 1;
        None
    }

    fn is_valid(&self, ctx: Context<LearnDomain>) -> bool {
        let state = LearnDomain::get_cur_state(ctx.state_diff);
        state.map[state.agent_pos as usize] > 0
    }

    fn display_action(&self) -> DisplayAction {
        DisplayAction::Collect
    }

    impl_task_boxed_methods!(LearnDomain);
}
