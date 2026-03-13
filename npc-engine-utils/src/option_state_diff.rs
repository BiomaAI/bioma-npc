use bioma_npc_core::{Domain, StateDiffRef, StateDiffRefMut};
use std::hash::Hash;

/// In case your domain has a [Diff](Domain::Diff) that is an [Option]
/// of its [State](Domain::State),
/// provides helper functions to retrieve the state in your [tasks](bioma_npc_core::Task).
///
/// The functions [get_cur_state](Self::get_cur_state) and [get_cur_state_mut](Self::get_cur_state_mut)
/// are available when read-only, respectively read-write, access is required.
/// In that case, just use the trait in your task files: `use bioma_npc_utils::OptionDiffDomain;`.
pub trait OptionDiffDomain {
    type Domain: Domain<State = Self::State, Diff = Option<Self::State>>;
    type State: Clone;
    /// Returns either the `diff` if it is not `None`, or the `initial_state`.
    fn get_cur_state(
        state_diff: StateDiffRef<'_, Self::Domain>,
    ) -> &<<Self as OptionDiffDomain>::Domain as Domain>::State {
        if let Some(diff) = state_diff.diff {
            diff
        } else {
            state_diff.initial_state
        }
    }
    /// Returns either the `diff` if it is not `None`, or copies the `initial_state` into the `diff` and returns it.
    fn get_cur_state_mut(
        state_diff: StateDiffRefMut<'_, Self::Domain>,
    ) -> &mut <<Self as OptionDiffDomain>::Domain as Domain>::State {
        if let Some(diff) = state_diff.diff {
            diff
        } else {
            let diff = state_diff.initial_state.clone();
            *state_diff.diff = Some(diff);
            &mut *state_diff.diff.as_mut().unwrap()
        }
    }
}

impl<
        S: std::fmt::Debug + Sized + Clone + Hash + Eq,
        DA: std::fmt::Debug + Default,
        D: Domain<State = S, Diff = Option<S>, DisplayAction = DA>,
    > OptionDiffDomain for D
{
    type Domain = D;
    type State = <D as Domain>::State;
}
