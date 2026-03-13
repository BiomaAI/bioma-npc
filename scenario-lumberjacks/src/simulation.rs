use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process;

use bioma_npc_core::{
    graphviz::set_graph_output_depth, AgentId, ContextMut, MCTSConfiguration, Task, MCTS,
};
use bioma_npc_utils::GlobalDomain;
use rand::random;

use crate::analytics;
use crate::{
    agency_metric_hook, branching_metric_hook, config, diff_memory_metric_hook,
    features_metric_hook, graph_hook, islands_metric_hook, name, node_edges_count_metric_hook,
    output_path, time_metric_hook, total_memory_metric_hook, world_serialization_hook, working_dir,
    Action, AgentInventory, GeneratorType, HeatmapOverlay, Lumberjacks, PostMCTSHookArgs,
    PostMCTSHookFn, PostWorldHookArgs, PostWorldHookFn, PreWorldHookArgs, PreWorldHookFn,
    TileMap, WorldDiff, WorldGlobalState, WorldLocalState,
};

pub struct PreparedStep {
    turn: usize,
    agent: AgentId,
    initial_state: WorldLocalState,
    diff: WorldDiff,
    display_action: Action,
    next_objective: Option<Box<dyn Task<Lumberjacks>>>,
    heatmap: Option<HeatmapOverlay>,
}

impl PreparedStep {
    pub fn turn(&self) -> usize {
        self.turn
    }

    pub fn agent(&self) -> AgentId {
        self.agent
    }

    pub fn heatmap(&self) -> Option<&HeatmapOverlay> {
        self.heatmap.as_ref()
    }
}

pub struct SimulationState {
    interactive: bool,
    capture_snapshots: bool,
    seed: u64,
    current_agent: usize,
    run: Option<usize>,
    turn: usize,
    world: WorldGlobalState,
    config: MCTSConfiguration,
    agents: Vec<AgentId>,
    objectives: BTreeMap<AgentId, Box<dyn Task<Lumberjacks>>>,
    pre_world_hooks: Vec<PreWorldHookFn>,
    post_world_hooks: Vec<PostWorldHookFn>,
    post_mcts_hooks: Vec<PostMCTSHookFn>,
    finalized: bool,
}

impl SimulationState {
    pub fn new(interactive: bool, run: Option<usize>, seed: u64, capture_snapshots: bool) -> Self {
        let mut agents = Vec::new();
        let inventory = Default::default();
        let map = match &config().map.generator {
            GeneratorType::File { path, .. } => {
                let file = match fs::OpenOptions::new().read(true).open(format!(
                    "{}/{}",
                    working_dir(),
                    path
                )) {
                    Ok(file) => file,
                    Err(err) => {
                        eprintln!("Cannot open map file {}: {}", path, err);
                        process::exit(2);
                    }
                };
                TileMap::from_io(&mut agents, &file)
            }
        };
        agents.sort();

        let mut world = WorldGlobalState {
            actions: Default::default(),
            inventory,
            map,
        };

        for agent in &agents {
            world.inventory.0.insert(
                *agent,
                AgentInventory {
                    wood: 0,
                    water: false,
                },
            );
        }

        let mcts_config = MCTSConfiguration {
            allow_invalid_tasks: false,
            visits: config().mcts.visits,
            depth: config().mcts.depth,
            exploration: config().mcts.exploration,
            discount_hl: -1.0 / config().mcts.discount.log2(),
            seed: Some(seed),
            ..Default::default()
        };

        let mut state = Self {
            interactive,
            capture_snapshots,
            seed,
            current_agent: 0,
            run,
            turn: 0,
            world,
            config: mcts_config,
            agents,
            objectives: BTreeMap::new(),
            pre_world_hooks: Vec::new(),
            post_world_hooks: Vec::new(),
            post_mcts_hooks: Vec::new(),
            finalized: false,
        };

        set_graph_output_depth(config().analytics.graphs_depth);
        state.register_hooks();
        state
    }

    pub fn interactive(&self) -> bool {
        self.interactive
    }

    pub fn turn(&self) -> usize {
        self.turn
    }

    pub fn at_turn_start(&self) -> bool {
        self.current_agent == 0
    }

    pub fn width(&self) -> usize {
        self.world.map.width
    }

    pub fn height(&self) -> usize {
        self.world.map.height
    }

    pub fn world(&self) -> &WorldGlobalState {
        &self.world
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    pub fn window_title(&self) -> String {
        let mode = if self.interactive { "manual" } else { "auto" };
        format!("{} - turn {} ({mode})", name(), self.turn)
    }

    pub fn dump_run(&self) {
        fs::create_dir_all(self.output_dir()).unwrap();

        let info = serde_json::json!({
            "run": self.run,
            "seed": self.seed,
        });

        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(Path::new(&self.output_dir()).join("run.json"))
            .unwrap();

        serde_json::to_writer_pretty(file, &info).unwrap();
    }

    pub fn dump_result(&self) {
        fs::create_dir_all(self.output_dir()).unwrap();

        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(Path::new(&self.output_dir()).join("result.json"))
            .unwrap();

        serde_json::to_writer_pretty(file, &self.world).unwrap();
    }

    pub fn output_dir(&self) -> String {
        format!(
            "{}/{}/",
            output_path(),
            self.run.map(|value| value.to_string()).unwrap_or_default(),
        )
    }

    pub fn write_start_artifacts(&self) {
        if self.capture_snapshots {
            analytics::save_world_png(self.world(), Path::new(&self.output_dir()).join("start.png"));
        }
    }

    pub fn finalize(&mut self) {
        if self.finalized {
            return;
        }

        if self.capture_snapshots {
            analytics::save_world_png(self.world(), Path::new(&self.output_dir()).join("result.png"));
        }

        self.dump_result();
        self.finalized = true;
    }

    pub fn prepare_step(&mut self) -> Option<PreparedStep> {
        if self.finalized {
            return None;
        }

        let turn = self.turn;
        let run = self.run;

        if self.current_agent == 0 {
            let world = &self.world;
            self.pre_world_hooks
                .iter_mut()
                .for_each(|hook| hook(PreWorldHookArgs { run, turn, world }));
        }

        let agent = self.agents[self.current_agent];
        let initial_state = Lumberjacks::derive_local_state(&self.world, agent);
        let mut mcts = MCTS::new(initial_state, agent, self.config.clone());

        println!("planning start, turn {} {:?}", turn, agent);
        let objective = mcts.run().unwrap();
        println!("planning end");

        let heatmap = config()
            .analytics
            .heatmaps
            .then(|| analytics::build_heatmap_overlay(agent, &mcts))
            .flatten();

        let world = &self.world;
        self.post_mcts_hooks.iter_mut().for_each(|hook| {
            hook(PostMCTSHookArgs {
                run,
                turn,
                world,
                agent,
                mcts: &mcts,
                objective: objective.clone(),
            })
        });

        let mut diff = WorldDiff::default();
        let mcts_ctx = ContextMut::with_state_and_diff(0, mcts.initial_state(), &mut diff, agent);
        let next_objective = objective.execute(mcts_ctx);
        let display_action = objective.display_action();

        Some(PreparedStep {
            turn,
            agent,
            initial_state: mcts.initial_state().clone(),
            diff,
            display_action,
            next_objective,
            heatmap,
        })
    }

    pub fn apply_prepared_step(&mut self, step: PreparedStep) {
        if self.finalized {
            return;
        }

        debug_assert_eq!(step.turn, self.turn);
        debug_assert_eq!(step.agent, self.agents[self.current_agent]);

        Lumberjacks::apply(&mut self.world, &step.initial_state, &step.diff);
        self.world.actions.insert(step.agent, step.display_action);
        step.next_objective
            .map(|next| self.objectives.insert(step.agent, next));

        let world = &self.world;
        self.post_world_hooks.iter_mut().for_each(|hook| {
            hook(PostWorldHookArgs {
                run: self.run,
                turn: self.turn,
                world,
                objectives: &self.objectives,
            })
        });

        self.current_agent += 1;
        if self.current_agent == self.agents.len() {
            self.current_agent = 0;
            self.turn += 1;
        }

        if let Some(turns) = config().turns
            && self.turn >= turns
        {
            self.finalize();
        }
    }

    pub fn step(&mut self) {
        if self.finalized {
            return;
        }

        if self.current_agent == 0 && config().analytics.screenshot {
            analytics::save_turn_screenshot(self.world(), self.run, self.turn);
        }

        let Some(step) = self.prepare_step() else {
            return;
        };

        if let Some(heatmap) = step.heatmap() {
            analytics::save_heatmap_overlay(
                self.world(),
                Path::new(output_path())
                    .join(self.run.map(|n| n.to_string()).unwrap_or_default())
                    .join("heatmaps")
                    .join(format!("agent{}", step.agent().0))
                    .join(format!("{:06}.png", step.turn())),
                heatmap,
            );
        }

        self.apply_prepared_step(step);
    }

    fn register_hooks(&mut self) {
        if config().analytics.graphs {
            self.post_mcts_hooks.push(graph_hook());
        }

        if config().analytics.metrics {
            self.pre_world_hooks.push(features_metric_hook());
            self.pre_world_hooks.push(islands_metric_hook());
            self.post_mcts_hooks.push(agency_metric_hook());
        }

        if config().analytics.serialization {
            self.pre_world_hooks.push(world_serialization_hook());
        }

        if config().analytics.performance {
            self.post_mcts_hooks.push(node_edges_count_metric_hook());
            self.post_mcts_hooks.push(diff_memory_metric_hook());
            self.post_mcts_hooks.push(total_memory_metric_hook());
            self.post_mcts_hooks.push(branching_metric_hook());
            self.post_mcts_hooks.push(time_metric_hook());
        }
    }
}

pub fn next_seed() -> u64 {
    config().mcts.seed.unwrap_or_else(random)
}
