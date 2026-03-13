use std::collections::BTreeMap;

use std::fs;
use std::process;

use bioma_npc_core::ContextMut;
use bioma_npc_core::MCTSConfiguration;
use bioma_npc_core::{AgentId, Task, MCTS};
use bioma_npc_utils::GlobalDomain;
use ggez::event::EventHandler;
use ggez::glam::Vec2;
use ggez::graphics::Canvas;
use ggez::graphics::{Color, DrawParam, Image, Text};
use ggez::input::keyboard::KeyCode;
use ggez::{Context, GameResult};

use crate::WorldDiff;
use crate::{
    agency_metric_hook, branching_metric_hook, config, diff_memory_metric_hook,
    features_metric_hook, graph_hook, heatmap_hook, islands_metric_hook,
    node_edges_count_metric_hook, output_path, screenshot, screenshot_hook, time_metric_hook,
    total_memory_metric_hook, working_dir, world_serialization_hook, AgentInventory, GeneratorType,
    Lumberjacks, PostMCTSHookArgs, PostMCTSHookFn, PostWorldHookArgs, PostWorldHookFn,
    PreWorldHookArgs, PreWorldHookFn, TileMap, WorldGlobalState, SPRITE_SIZE,
};

type PerWorldHooks = Vec<Box<dyn FnMut(PreWorldHookArgs)>>;
type PostWorldHooks = Vec<Box<dyn FnMut(PostWorldHookArgs)>>;
type PostMCTSHooks = Vec<Box<dyn FnMut(PostMCTSHookArgs)>>;

pub struct GameState {
    interactive: bool,
    seed: u64,
    current_agent: usize,
    run: Option<usize>,
    turn: usize,
    world: WorldGlobalState,
    config: MCTSConfiguration,
    agents: Vec<AgentId>,
    objectives: BTreeMap<AgentId, Box<dyn Task<Lumberjacks>>>,
    pre_world_hooks: PerWorldHooks,
    post_world_hooks: PostWorldHooks,
    post_mcts_hooks: PostMCTSHooks,
    assets: BTreeMap<String, Image>,
    finalized: bool,
}

impl GameState {
    pub fn new(interactive: bool, run: Option<usize>, seed: u64) -> Self {
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
                    Err(e) => {
                        println!("Cannot open map file {}: {}", path, e);
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

        let config = MCTSConfiguration {
            allow_invalid_tasks: false,
            visits: config().mcts.visits,
            depth: config().mcts.depth,
            exploration: config().mcts.exploration,
            discount_hl: -1.0 / config().mcts.discount.log2(),
            seed: Some(seed),
            ..Default::default()
        };
        let objectives = BTreeMap::new();

        let mut state = GameState {
            interactive,
            seed,
            current_agent: 0,
            run,
            turn: 0,
            world,
            agents,
            objectives,
            config,
            pre_world_hooks: Default::default(),
            post_world_hooks: Default::default(),
            post_mcts_hooks: Default::default(),
            assets: Default::default(),
            finalized: false,
        };

        state.register_hooks();

        state
    }

    pub fn register_pre_world_hook(&mut self, f: PreWorldHookFn) {
        self.pre_world_hooks.push(f);
    }

    pub fn register_post_world_hook(&mut self, f: PostWorldHookFn) {
        self.post_world_hooks.push(f);
    }

    pub fn register_post_mcts_hook(&mut self, f: PostMCTSHookFn) {
        self.post_mcts_hooks.push(f);
    }

    pub fn register_hooks(&mut self) {
        if config().analytics.heatmaps {
            self.register_post_mcts_hook(heatmap_hook());
        }

        if config().analytics.graphs {
            self.register_post_mcts_hook(graph_hook());
        }

        if config().analytics.metrics {
            self.register_pre_world_hook(features_metric_hook());
            self.register_pre_world_hook(islands_metric_hook());
            self.register_post_mcts_hook(agency_metric_hook());
        }

        if config().analytics.serialization {
            self.register_pre_world_hook(world_serialization_hook());
        }

        if config().analytics.screenshot {
            self.register_pre_world_hook(screenshot_hook());
        }

        if config().analytics.performance {
            self.register_post_mcts_hook(node_edges_count_metric_hook());
            self.register_post_mcts_hook(diff_memory_metric_hook());
            self.register_post_mcts_hook(total_memory_metric_hook());
            self.register_post_mcts_hook(branching_metric_hook());
            self.register_post_mcts_hook(time_metric_hook());
        }
    }

    pub fn dump_run(&self) {
        fs::create_dir_all(format!(
            "{}/{}/",
            &output_path(),
            self.run.map(|n| n.to_string()).unwrap_or_default()
        ))
        .unwrap();

        let info = serde_json::json!({
            "run": self.run,
            "seed": self.seed,
        });

        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(format!(
                "{}/{}/run.json",
                output_path(),
                self.run.map(|n| n.to_string()).unwrap_or_default()
            ))
            .unwrap();

        serde_json::to_writer_pretty(file, &info).unwrap();
    }

    pub fn dump_result(&self) {
        fs::create_dir_all(format!(
            "{}/{}/",
            output_path(),
            self.run.map(|n| n.to_string()).unwrap_or_default(),
        ))
        .unwrap();

        let file = fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(format!(
                "{}/{}/result.json",
                output_path(),
                self.run.map(|n| n.to_string()).unwrap_or_default(),
            ))
            .unwrap();

        serde_json::to_writer_pretty(file, &self.world).unwrap();
    }

    pub fn output_dir(&self) -> String {
        format!(
            "{}/{}/",
            output_path(),
            self.run.map(|n| n.to_string()).unwrap_or_default(),
        )
    }

    pub fn screenshot(&self, ctx: &mut Context, path: &str) {
        screenshot(ctx, &self.world, &self.assets, path);
    }

    pub fn add_asset(&mut self, name: String, image: Image) {
        self.assets.insert(name, image);
    }

    fn finalize(&mut self, ctx: Option<&mut Context>) {
        if self.finalized {
            return;
        }

        if let Some(ctx) = ctx {
            let dir = self.output_dir();
            self.screenshot(ctx, &format!("{}/result.png", dir));
        }

        self.dump_result();
        self.finalized = true;
    }

    pub fn width(&self) -> usize {
        self.world.map.width
    }

    pub fn height(&self) -> usize {
        self.world.map.height
    }

    pub fn turn(&self) -> usize {
        self.turn
    }

    pub fn update(&mut self, mut ctx: Option<&mut Context>) {
        let turn = self.turn;
        let run = self.run;
        let assets = &self.assets;

        // Start of turn
        if self.current_agent == 0 {
            let world = &self.world;

            self.pre_world_hooks.iter_mut().for_each(|f| {
                f(PreWorldHookArgs {
                    run,
                    ctx: &mut ctx,
                    assets,
                    turn,
                    world,
                })
            });
        }

        let world = &mut self.world;
        let agent = self.agents[self.current_agent];
        let initial_state = Lumberjacks::derive_local_state(world, agent);
        // FIXME: re-introduce objectives as tasks
        let mut mcts = MCTS::new(initial_state, agent, self.config.clone());

        println!("planning start, turn {} {:?}", turn, agent);
        let objective = mcts.run().unwrap();
        println!("planning end");

        self.post_mcts_hooks.iter_mut().for_each(|f| {
            f(PostMCTSHookArgs {
                run,
                ctx: &mut ctx,
                assets,
                turn,
                world,
                agent,
                mcts: &mcts,
                objective: objective.clone(),
            })
        });

        let mut diff = WorldDiff::default();
        let mcts_ctx = ContextMut::with_state_and_diff(0, mcts.initial_state(), &mut diff, agent);
        let new_objective = objective.execute(mcts_ctx);
        Lumberjacks::apply(world, mcts.initial_state(), &diff);
        world.actions.insert(agent, objective.display_action());
        new_objective.map(|objective| self.objectives.insert(agent, objective));

        self.post_world_hooks.iter_mut().for_each(|f| {
            f(PostWorldHookArgs {
                run,
                ctx: &mut ctx,
                assets,
                turn,
                world,
                objectives: &self.objectives,
            })
        });

        self.current_agent += 1;

        if self.current_agent == self.agents.len() {
            self.current_agent = 0;
            self.turn += 1;
        }
    }
}

impl EventHandler for GameState {
    fn update(&mut self, ctx: &mut Context) -> GameResult<()> {
        match config().turns {
            Some(turns) if self.turn >= turns => {
                self.finalize(Some(ctx));
                ctx.request_quit();
                return Ok(());
            }
            _ => (),
        }

        if ctx.keyboard.is_key_pressed(KeyCode::Return) || !self.interactive {
            GameState::update(self, Some(ctx));
        }

        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> GameResult<()> {
        let world = &self.world;

        let mut canvas = Canvas::from_frame(
            ctx,
            Color::new(
                config().display.background.0,
                config().display.background.1,
                config().display.background.2,
                1.0,
            ),
        );

        canvas.draw(
            &Text::new(format!("Turn: {}", self.turn)),
            DrawParam::default()
                .dest(Vec2::new(5.0 * SPRITE_SIZE, 0.0))
                .color(Color::WHITE),
        );
        world.draw(ctx, &mut canvas, &self.assets);

        canvas.finish(ctx)
    }

    fn quit_event(&mut self, ctx: &mut Context) -> GameResult<bool> {
        self.finalize(Some(ctx));
        Ok(false)
    }
}
