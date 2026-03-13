use criterion::{criterion_group, criterion_main, Criterion};
use std::hint::black_box;
use std::{collections::BTreeSet, fmt, hash::Hash};

use bioma_npc_core::{
    impl_task_boxed_methods, AgentId, AgentValue, Behavior, Context, ContextMut, Domain,
    MCTSConfiguration, StateDiffRef, Task, TaskDuration, MCTS,
};

pub(crate) struct TestEngine;

#[derive(Debug, Clone, Copy)]
pub(crate) struct State(u16);

#[derive(Debug, Default, Eq, Hash, Clone, PartialEq)]
pub(crate) struct Diff(u16);

#[derive(Debug, Default)]
struct DisplayAction;
impl fmt::Display for DisplayAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "")
    }
}

impl Domain for TestEngine {
    type State = State;
    type Diff = Diff;
    type DisplayAction = DisplayAction;

    fn list_behaviors() -> &'static [&'static dyn Behavior<Self>] {
        &[&TestBehavior]
    }

    fn get_current_value(
        _tick: u64,
        state_diff: StateDiffRef<Self>,
        _agent: AgentId,
    ) -> AgentValue {
        (state_diff.initial_state.0 + state_diff.diff.0).into()
    }

    fn update_visible_agents(_start_tick: u64, ctx: Context<Self>, agents: &mut BTreeSet<AgentId>) {
        agents.insert(ctx.agent);
    }
}

#[derive(Copy, Clone, Debug)]
struct TestBehavior;

impl Behavior<TestEngine> for TestBehavior {
    fn add_own_tasks(&self, _ctx: Context<TestEngine>, tasks: &mut Vec<Box<dyn Task<TestEngine>>>) {
        tasks.push(Box::new(TestTask) as _);
    }

    fn is_valid(&self, _ctx: Context<TestEngine>) -> bool {
        true
    }
}

#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq)]
struct TestTask;

impl Task<TestEngine> for TestTask {
    fn weight(&self, _ctx: Context<TestEngine>) -> f32 {
        1.
    }

    fn duration(&self, _ctx: Context<TestEngine>) -> TaskDuration {
        1
    }

    fn is_valid(&self, _ctx: Context<TestEngine>) -> bool {
        true
    }

    fn execute(&self, ctx: ContextMut<TestEngine>) -> Option<Box<dyn Task<TestEngine>>> {
        ctx.state_diff.diff.0 += 1;
        None
    }

    fn display_action(&self) -> <TestEngine as Domain>::DisplayAction {
        DisplayAction
    }

    impl_task_boxed_methods!(TestEngine);
}

fn mcts_benchmark(c: &mut Criterion) {
    // TODO change these params to match lumberjack.
    const CONFIG: MCTSConfiguration = MCTSConfiguration {
        allow_invalid_tasks: false,
        visits: 10_000,
        depth: 5,
        exploration: 1.414,
        discount_hl: 15.,
        seed: None,
        planning_task_duration: None,
    };

    let agent = AgentId(0);
    let world = State(0);

    c.bench_function("mcts_run", |b| {
        b.iter(|| {
            let mut mcts = MCTS::<TestEngine>::new(black_box(world), agent, CONFIG);
            black_box(mcts.run().unwrap());
        });
    });
}

criterion_group!(benches, mcts_benchmark);
criterion_main!(benches);
