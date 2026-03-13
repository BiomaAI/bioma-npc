use std::fmt;

use bioma_npc_core::AgentId;

#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum Player {
    O,
    X,
}

impl Player {
    pub fn from_agent(agent: AgentId) -> Self {
        match agent {
            AgentId(0) => Player::O,
            AgentId(1) => Player::X,
            AgentId(id) => panic!("Invalid AgentId {id}"),
        }
    }

    pub fn to_agent(self) -> AgentId {
        match self {
            Player::O => AgentId(0),
            Player::X => AgentId(1),
        }
    }
}

impl fmt::Display for Player {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Player::O => write!(f, "O"),
            Player::X => write!(f, "X"),
        }
    }
}
