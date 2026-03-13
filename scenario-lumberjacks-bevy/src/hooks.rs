use std::collections::BTreeMap;

use bioma_npc_core::{AgentId, Task, MCTS};

use crate::{Lumberjacks, WorldGlobalState};

pub type PreWorldHookFn = Box<dyn FnMut(PreWorldHookArgs) + 'static>;
pub type PostWorldHookFn = Box<dyn FnMut(PostWorldHookArgs) + 'static>;
pub type PostMCTSHookFn = Box<dyn FnMut(PostMCTSHookArgs) + 'static>;

// Pre world hooks are called once per game loop before any actions have executed
pub struct PreWorldHookArgs<'a> {
    pub run: Option<usize>,
    pub turn: usize,
    pub world: &'a WorldGlobalState,
}

// Post world hooks are called once per game loop after all actions have executed
pub struct PostWorldHookArgs<'a> {
    pub run: Option<usize>,
    pub turn: usize,
    pub world: &'a WorldGlobalState,
    pub objectives: &'a BTreeMap<AgentId, Box<dyn Task<Lumberjacks>>>,
}

// Post MCTS hooks are called once per agent per loop after it runs this turn
pub struct PostMCTSHookArgs<'a> {
    pub run: Option<usize>,
    pub turn: usize,
    pub world: &'a WorldGlobalState,
    pub agent: AgentId,
    pub mcts: &'a MCTS<Lumberjacks>,
    pub objective: Box<dyn Task<Lumberjacks>>,
}
