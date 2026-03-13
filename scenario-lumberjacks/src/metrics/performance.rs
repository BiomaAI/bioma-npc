use std::collections::HashMap;
use std::mem;
use std::time::Duration;

use crate::{Barrier, Chop, Move, Plant, PostMCTSHookArgs, PostMCTSHookFn, Refill, Wait, Water};
use bioma_npc_core::{AgentId, IdleTask, PlanningTask, Task};
use bioma_npc_utils::Direction;

fn task_footprint(task: &dyn Task<crate::Lumberjacks>) -> usize {
    if task.downcast_ref::<Barrier>().is_some() {
        mem::size_of::<Barrier>()
    } else if task.downcast_ref::<Chop>().is_some() {
        mem::size_of::<Chop>()
    } else if let Some(task) = task.downcast_ref::<Move>() {
        mem::size_of::<Move>() + task.path.len() * mem::size_of::<Direction>()
    } else if task.downcast_ref::<Plant>().is_some() {
        mem::size_of::<Plant>()
    } else if task.downcast_ref::<Refill>().is_some() {
        mem::size_of::<Refill>()
    } else if task.downcast_ref::<Wait>().is_some() {
        mem::size_of::<Wait>()
    } else if task.downcast_ref::<Water>().is_some() {
        mem::size_of::<Water>()
    } else if task.downcast_ref::<IdleTask>().is_some() {
        mem::size_of::<IdleTask>()
    } else if task.downcast_ref::<PlanningTask>().is_some() {
        mem::size_of::<PlanningTask>()
    } else {
        // Future planner internals should not crash analytics.
        mem::size_of_val(task)
    }
}

pub fn node_edges_count_metric_hook() -> PostMCTSHookFn {
    let mut stats = HashMap::<AgentId, (usize, usize, usize)>::default();

    Box::new(move |PostMCTSHookArgs { agent, mcts, .. }| {
        let (nodes, edges, count) = stats.entry(agent).or_default();

        *nodes += mcts.node_count();
        *edges += mcts.edge_count();
        *count += 1;

        log::info!(
            "Agent {} Avg # Nodes: {} ({} samples)",
            agent.0,
            *nodes as f32 / *count as f32,
            count
        );
        log::info!(
            "Agent {} Avg # Edges: {} ({} samples)",
            agent.0,
            *edges as f32 / *count as f32,
            count
        );
    })
}

pub fn diff_memory_metric_hook() -> PostMCTSHookFn {
    let mut stats = HashMap::<AgentId, (usize, usize)>::default();

    Box::new(move |PostMCTSHookArgs { agent, mcts, .. }| {
        let (diff_size, count) = stats.entry(agent).or_default();

        for (node, _) in mcts.nodes() {
            *diff_size += node.diff().diff_size();
            *count += 1;
        }

        log::info!(
            "Agent {} Avg Diff Size: {} bytes ({} samples)",
            agent.0,
            *diff_size as f32 / *count as f32,
            count
        );
    })
}

pub fn total_memory_metric_hook() -> PostMCTSHookFn {
    let mut stats = HashMap::<AgentId, (usize, usize)>::default();

    Box::new(move |PostMCTSHookArgs { agent, mcts, .. }| {
        let (total_size, count) = stats.entry(agent).or_default();

        *total_size += mcts.size(task_footprint);

        *count += 1;

        log::info!(
            "Agent {} Avg Total Size: {} bytes ({} samples)",
            agent.0,
            *total_size as f32 / *count as f32,
            count
        );
    })
}

pub fn branching_metric_hook() -> PostMCTSHookFn {
    let mut stats = HashMap::<AgentId, (usize, usize)>::default();

    Box::new(move |PostMCTSHookArgs { agent, mcts, .. }| {
        let (branching, count) = stats.entry(agent).or_default();

        for (_, edges) in mcts.nodes() {
            *branching += edges.branching_factor();
            *count += 1;
        }

        log::info!(
            "Agent {} Avg Branching Factor: {} ({} samples)",
            agent.0,
            *branching as f32 / *count as f32,
            count
        );
    })
}

pub fn time_metric_hook() -> PostMCTSHookFn {
    let mut stats = HashMap::<AgentId, (Duration, usize)>::default();

    Box::new(move |PostMCTSHookArgs { agent, mcts, .. }| {
        let (time, count) = stats.entry(agent).or_default();

        *time += mcts.time();
        *count += 1;

        log::info!(
            "Agent {} Avg Time: {:?} ({} samples)",
            agent.0,
            *time / *count as u32,
            count
        );
    })
}

#[cfg(test)]
mod tests {
    use super::task_footprint;
    use crate::{Move, Wait};
    use bioma_npc_core::{IdleTask, PlanningTask};
    use bioma_npc_utils::Direction;
    use std::num::NonZeroU64;

    #[test]
    fn accounts_for_move_path_storage() {
        let task = Move {
            path: vec![Direction::Up, Direction::Right],
            x: 1,
            y: 2,
        };

        assert!(task_footprint(&task) > std::mem::size_of::<Move>());
    }

    #[test]
    fn handles_planner_internal_tasks() {
        assert_eq!(task_footprint(&Wait), std::mem::size_of::<Wait>());
        assert_eq!(task_footprint(&IdleTask), std::mem::size_of::<IdleTask>());
        assert_eq!(
            task_footprint(&PlanningTask(NonZeroU64::new(1).unwrap())),
            std::mem::size_of::<PlanningTask>()
        );
    }
}
