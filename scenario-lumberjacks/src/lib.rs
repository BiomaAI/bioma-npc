use std::collections::{HashMap, VecDeque};
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::Once;
use std::time::Duration;
use std::{fs, io, mem, process};

use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::asset::{AssetPlugin, AssetServer, LoadState};
use bevy::camera::{Camera, RenderTarget};
use bevy::ecs::system::SystemParam;
use bevy::image::BevyDefault as _;
use bevy::prelude::{
    App, Assets, ButtonInput, Camera2d, ClearColor, Color, Commands, Component, DefaultPlugins,
    Entity, Handle, Image, IntoScheduleConfigs, KeyCode, MessageReader, MessageWriter,
    MinimalPlugins, Node, NonSend, NonSendMut, PluginGroup, PositionType, Query, Res, ResMut,
    Resource, Single, Sprite, Startup, Text, TextColor, TextFont, Transform, Update, Val, Vec2,
    With, default,
};
use bevy::render::view::screenshot::{Screenshot, save_to_disk};
use bevy::render::{
    ExtractSchedule, MainWorld, RenderApp,
    render_resource::{PipelineCache, TextureFormat},
};
use bevy::ui::UiTargetCamera;
use bevy::window::{
    ExitCondition, PrimaryWindow, Window, WindowCloseRequested, WindowPlugin, WindowResolution,
};
use bevy::winit::WinitPlugin;
use bioma_npc_core::AgentId;
use clap::{Arg, ArgAction, Command};
use serde_json::Value;

mod analytics;
mod assets;
mod behaviors;
mod config;
mod fitnesses;
mod graph;
mod hooks;
mod inventory;
mod lumberjacks_domain;
mod metrics;
mod serialization;
mod simulation;
mod tasks;
mod tilemap;
mod util;
mod world;

pub use analytics::*;
pub use behaviors::*;
pub use config::*;
pub use graph::*;
pub use hooks::*;
pub use inventory::*;
pub use lumberjacks_domain::*;
pub use metrics::*;
pub use serialization::*;
pub use tasks::*;
pub use tilemap::*;
pub use util::*;
pub use world::*;

use crate::assets::{SPRITE_FILES, sprite_name_for_tile, sprite_path, workspace_root};
use crate::simulation::{PreparedStep, SimulationState, next_seed};

static INIT: Once = Once::new();
static mut CONFIG: MaybeUninit<Config> = MaybeUninit::uninit();
static mut WORKING_DIR: MaybeUninit<String> = MaybeUninit::uninit();
static mut OUTPUT_PATH: MaybeUninit<String> = MaybeUninit::uninit();
static mut NAME: MaybeUninit<String> = MaybeUninit::uninit();
static mut BATCH: MaybeUninit<bool> = MaybeUninit::uninit();

#[derive(Component)]
struct VisualEntity;

#[derive(Resource)]
struct SpriteHandles(HashMap<&'static str, Handle<Image>>);

struct VisualState {
    sim: SimulationState,
    pending_step: Option<PreparedStep>,
    dirty: bool,
}

impl VisualState {
    fn replace_sim(&mut self, sim: SimulationState) {
        self.sim = sim;
        self.pending_step = None;
        self.dirty = true;
    }
}

#[derive(Resource, Clone, Copy)]
struct SceneCamera(Entity);

#[derive(Resource, Clone)]
enum CaptureSource {
    PrimaryWindow,
    Image(Handle<Image>),
}

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
enum RuntimeMode {
    Visual,
    Headless,
}

#[derive(Resource, Default)]
struct VisualAssetsState {
    ready: bool,
    confirmation_frames: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum VisualCaptureKind {
    Start,
    Turn { turn: usize },
    Heatmap { turn: usize, agent: AgentId },
    Result,
}

#[derive(Clone, Debug)]
struct VisualCaptureRequest {
    kind: VisualCaptureKind,
    path: PathBuf,
}

#[derive(Resource, Default)]
struct VisualCaptureState {
    queue: VecDeque<VisualCaptureRequest>,
    active_request: Option<VisualCaptureRequest>,
    start_requested: bool,
    result_requested: bool,
    last_turn_requested: Option<usize>,
    last_heatmap_requested: Option<(usize, AgentId)>,
    exit_when_done: bool,
}

impl VisualCaptureState {
    fn for_mode(mode: RuntimeMode) -> Self {
        let mut state = Self::default();
        if mode == RuntimeMode::Headless && !config().analytics.screenshot {
            state.start_requested = true;
            state.result_requested = true;
        }
        state
    }

    fn reset_for_mode(&mut self, mode: RuntimeMode) {
        *self = Self::for_mode(mode);
    }

    fn is_idle(&self) -> bool {
        self.queue.is_empty() && self.active_request.is_none()
    }
}

#[derive(Resource)]
struct HeadlessBatchState {
    next_run: usize,
    total_runs: usize,
}

impl HeadlessBatchState {
    fn new(total_runs: usize) -> Self {
        Self {
            next_run: 1,
            total_runs,
        }
    }

    fn take_next_run(&mut self) -> Option<usize> {
        let run = (self.next_run < self.total_runs).then_some(self.next_run);
        if run.is_some() {
            self.next_run += 1;
        }
        run
    }

    fn has_pending_runs(&self) -> bool {
        self.next_run < self.total_runs
    }
}

#[derive(Resource, Clone, Copy)]
struct HeadlessRenderTarget {
    width: u32,
    height: u32,
}

#[derive(SystemParam)]
struct HeadlessBatchTransition<'w, 's> {
    scene_camera: Res<'w, SceneCamera>,
    render_targets: Query<'w, 's, &'static mut RenderTarget>,
    images: ResMut<'w, Assets<Image>>,
    capture_source: ResMut<'w, CaptureSource>,
    render_target: ResMut<'w, HeadlessRenderTarget>,
    capture_state: ResMut<'w, VisualCaptureState>,
    batch_state: ResMut<'w, HeadlessBatchState>,
    visual: NonSendMut<'w, VisualState>,
}

struct PipelinesReadyPlugin;

impl bevy::app::Plugin for PipelinesReadyPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PipelinesReady::default());
        app.sub_app_mut(RenderApp)
            .add_systems(ExtractSchedule, update_pipelines_ready);
    }
}

#[derive(Resource, Default)]
struct PipelinesReady(bool);

fn inventory_rows(world: &WorldGlobalState) -> Vec<(AgentId, isize, bool)> {
    let mut agents = world
        .inventory
        .0
        .iter()
        .map(|(agent, inventory)| (*agent, inventory.wood, inventory.water))
        .collect::<Vec<_>>();
    agents.sort_by_key(|(agent, ..)| *agent);
    agents
}

fn enqueue_visual_capture(
    queue: &mut VecDeque<VisualCaptureRequest>,
    request: VisualCaptureRequest,
) {
    if let Some(parent) = request.path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    queue.push_back(request);
}

fn surface_size(sim: &SimulationState) -> (u32, u32) {
    let width =
        ((2 * config().display.padding.0 + sim.width()) as f32 * SPRITE_SIZE).round() as u32;
    let height =
        ((2 * config().display.padding.1 + sim.height()) as f32 * SPRITE_SIZE).round() as u32;
    (width.max(1), height.max(1))
}

fn create_render_target(images: &mut Assets<Image>, width: u32, height: u32) -> Handle<Image> {
    let image = Image::new_target_texture(width, height, TextureFormat::bevy_default(), None);
    images.add(image)
}

fn build_headless_simulation(run: usize) -> SimulationState {
    let sim = SimulationState::new(config().display.interactive, Some(run), next_seed(), false);
    sim.dump_run();
    sim
}

unsafe fn init() {
    INIT.call_once(|| {
        let matches = Command::new("Lumberjacks Bevy")
            .version("1.0")
            .arg(
                Arg::new("config")
                    .required(true)
                    .help("Sets config file path"),
            )
            .arg(
                Arg::new("working-dir")
                    .required(false)
                    .num_args(1)
                    .value_name("directory")
                    .long("working-dir")
                    .short('d')
                    .help("Overrides working dir"),
            )
            .arg(
                Arg::new("output")
                    .required(false)
                    .num_args(1)
                    .value_name("directory")
                    .short('o')
                    .long("output")
                    .help("Sets output directory"),
            )
            .arg(
                Arg::new("name")
                    .required(false)
                    .num_args(1)
                    .value_name("name")
                    .default_value("Lumberjacks Bevy")
                    .short('n')
                    .long("name")
                    .help("Sets name"),
            )
            .arg(
                Arg::new("batch")
                    .required(false)
                    .action(ArgAction::SetTrue)
                    .short('b')
                    .long("batch")
                    .help("Enables batch mode"),
            )
            .arg(
                Arg::new("set")
                    .required(false)
                    .action(ArgAction::Append)
                    .num_args(1)
                    .short('s')
                    .long("set")
                    .help("Manually override a value in the config"),
            )
            .get_matches();

        let config_path = matches.get_one::<String>("config").unwrap();
        let config_dir = {
            let mut path = PathBuf::from(config_path);
            path.pop();
            path.to_str().unwrap().to_owned()
        };

        unsafe {
            NAME = MaybeUninit::new(matches.get_one::<String>("name").unwrap().to_owned());
        }

        unsafe {
            OUTPUT_PATH = MaybeUninit::new(
                matches
                    .get_one::<String>("output")
                    .unwrap_or(&config_dir)
                    .to_owned(),
            );
        }

        unsafe {
            WORKING_DIR = MaybeUninit::new(
                matches
                    .get_one::<String>("working-dir")
                    .unwrap_or(&config_dir)
                    .to_owned(),
            );
        }

        unsafe {
            BATCH = MaybeUninit::new(matches.get_flag("batch"));
        }

        unsafe {
            CONFIG = MaybeUninit::new({
                let mut json: Value = match config_path.as_str() {
                    "-" => {
                        let stdin = io::stdin();
                        serde_json::from_reader(stdin.lock()).unwrap()
                    }
                    path => {
                        let config_file = match fs::OpenOptions::new().read(true).open(path) {
                            Ok(file) => file,
                            Err(err) => {
                                eprintln!("Cannot open config file {}: {}", path, err);
                                process::exit(1);
                            }
                        };
                        serde_json::from_reader(&config_file).unwrap()
                    }
                };

                if let Some(values) = matches.get_many::<String>("set") {
                    values.for_each(|value| {
                        let (path, raw) = value.split_once('=').unwrap_or_else(|| {
                            panic!("invalid format, expected \"some.path=value\"")
                        });

                        let mut object = &mut json;
                        let mut keys = path.split('.').peekable();

                        while let Some(key) = keys.next() {
                            if keys.peek().is_some() {
                                let map = object
                                    .as_object_mut()
                                    .ok_or_else(|| format!("Invalid 'set' path: {}", path))
                                    .unwrap();

                                if !matches!(map.get(key), Some(Value::Object(_))) {
                                    map.insert(key.to_owned(), Value::Object(Default::default()));
                                }

                                object = map.get_mut(key).unwrap();
                            } else {
                                let map = object
                                    .as_object_mut()
                                    .ok_or_else(|| format!("Invalid 'set' path: {}", path))
                                    .unwrap();

                                map.insert(
                                    key.to_owned(),
                                    serde_json::from_str(raw)
                                        .map_err(|err| format!("'set' variable not valid: {}", err))
                                        .unwrap(),
                                );
                            }
                        }
                    });
                }

                serde_json::from_value(json).unwrap()
            });
        }
    });
}

pub fn name() -> &'static String {
    unsafe {
        init();
        #[allow(static_mut_refs)]
        mem::transmute(&NAME)
    }
}

pub fn config() -> &'static Config {
    unsafe {
        init();
        #[allow(static_mut_refs)]
        mem::transmute(&CONFIG)
    }
}

pub fn working_dir() -> &'static String {
    unsafe {
        init();
        #[allow(static_mut_refs)]
        mem::transmute(&WORKING_DIR)
    }
}

pub fn output_path() -> &'static String {
    unsafe {
        init();
        #[allow(static_mut_refs)]
        mem::transmute(&OUTPUT_PATH)
    }
}

pub fn batch() -> bool {
    unsafe {
        init();
        #[allow(static_mut_refs)]
        mem::transmute(BATCH)
    }
}

pub fn run() {
    prepare_output_metadata();
    if batch() {
        run_headless();
    } else {
        run_visual();
    }
}

fn prepare_output_metadata() {
    fs::create_dir_all(output_path()).unwrap();

    let info = serde_json::json!({
        "git-hash": env!("GIT_HASH"),
        "config": config(),
    });

    let file = fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(PathBuf::from(output_path()).join("info.json"))
        .unwrap();

    serde_json::to_writer_pretty(file, &info).unwrap();
}

fn run_headless() {
    let turns = config()
        .turns
        .expect("Running batch mode with no turn limit!");

    if config().analytics.screenshot || config().analytics.heatmaps {
        if config().batch.runs == 0 {
            return;
        }
        run_headless_rendered_batch();
        return;
    }

    App::new()
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_once()))
        .add_systems(Update, move |mut exit: MessageWriter<AppExit>| {
            for run in 0..config().batch.runs {
                let mut sim = SimulationState::new(
                    config().display.interactive,
                    Some(run),
                    next_seed(),
                    config().analytics.screenshot,
                );
                sim.dump_run();
                sim.write_start_artifacts();
                while !sim.is_finalized() && sim.turn() < turns {
                    sim.step();
                }
                sim.finalize();
            }

            exit.write(AppExit::Success);
        })
        .run();
}

fn run_headless_rendered_batch() {
    let sim = build_headless_simulation(0);
    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: workspace_root().display().to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: None,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                })
                .disable::<WinitPlugin>(),
        )
        .add_plugins(PipelinesReadyPlugin)
        .add_plugins(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / 60.0,
        )))
        .insert_resource(RuntimeMode::Headless)
        .insert_resource(ClearColor(Color::srgb(
            config().display.background.0,
            config().display.background.1,
            config().display.background.2,
        )))
        .insert_resource(HeadlessBatchState::new(config().batch.runs))
        .insert_resource(VisualAssetsState::default())
        .insert_resource(VisualCaptureState::for_mode(RuntimeMode::Headless))
        .insert_non_send_resource(VisualState {
            sim,
            pending_step: None,
            dirty: true,
        })
        .add_systems(Startup, (setup_headless_camera, load_sprite_handles))
        .add_systems(
            Update,
            (
                update_visual_asset_state,
                refresh_visual_capture_state,
                queue_base_visual_captures,
                advance_headless_simulation,
                queue_pending_step_captures,
                sync_visual_scene,
                issue_capture_requests,
                advance_headless_batch,
                exit_headless_when_done,
            )
                .chain(),
        )
        .run();
}

fn run_visual() {
    let sim = SimulationState::new(config().display.interactive, None, next_seed(), false);
    let (width, height) = surface_size(&sim);
    sim.dump_run();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: workspace_root().display().to_string(),
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: name().clone(),
                        resolution: WindowResolution::new(width.max(1), height.max(1))
                            .with_scale_factor_override(1.0),
                        ..default()
                    }),
                    close_when_requested: false,
                    exit_condition: ExitCondition::DontExit,
                    ..default()
                }),
        )
        .add_plugins(PipelinesReadyPlugin)
        .insert_resource(RuntimeMode::Visual)
        .insert_resource(CaptureSource::PrimaryWindow)
        .insert_resource(ClearColor(Color::srgb(
            config().display.background.0,
            config().display.background.1,
            config().display.background.2,
        )))
        .insert_resource(VisualAssetsState::default())
        .insert_resource(VisualCaptureState::default())
        .insert_non_send_resource(VisualState {
            sim,
            pending_step: None,
            dirty: true,
        })
        .add_systems(Startup, (setup_visual_camera, load_sprite_handles))
        .add_systems(
            Update,
            (
                handle_close_requests,
                update_visual_asset_state,
                refresh_visual_capture_state,
                queue_base_visual_captures,
                advance_visual_simulation,
                queue_pending_step_captures,
                sync_visual_scene,
                issue_capture_requests,
                exit_after_visual_captures,
            )
                .chain(),
        )
        .run();
}

fn setup_visual_camera(mut commands: Commands) {
    let camera = commands.spawn(Camera2d).id();
    commands.insert_resource(SceneCamera(camera));
}

fn setup_headless_camera(
    mut commands: Commands,
    mut images: ResMut<Assets<Image>>,
    visual: NonSend<VisualState>,
) {
    let (width, height) = surface_size(&visual.sim);
    let target = create_render_target(&mut images, width, height);
    let camera = commands
        .spawn((
            Camera2d,
            Camera::default(),
            RenderTarget::Image(target.clone().into()),
        ))
        .id();
    commands.insert_resource(SceneCamera(camera));
    commands.insert_resource(CaptureSource::Image(target));
    commands.insert_resource(HeadlessRenderTarget { width, height });
}

fn load_sprite_handles(mut commands: Commands, asset_server: Res<AssetServer>) {
    let mut handles = HashMap::new();
    for (sprite, _) in SPRITE_FILES {
        handles.insert(*sprite, asset_server.load(sprite_path(sprite)));
    }
    commands.insert_resource(SpriteHandles(handles));
}

fn handle_close_requests(
    mut requests: MessageReader<WindowCloseRequested>,
    mut capture_state: ResMut<VisualCaptureState>,
    mut visual: NonSendMut<VisualState>,
) {
    if requests.read().next().is_some() {
        visual.pending_step = None;
        visual.sim.finalize();
        if !capture_state.result_requested {
            enqueue_visual_capture(
                &mut capture_state.queue,
                VisualCaptureRequest {
                    kind: VisualCaptureKind::Result,
                    path: PathBuf::from(visual.sim.output_dir()).join("result.png"),
                },
            );
            capture_state.result_requested = true;
        }
        capture_state.exit_when_done = true;
    }
}

fn update_visual_asset_state(
    asset_server: Res<AssetServer>,
    handles: Res<SpriteHandles>,
    pipelines_ready: Res<PipelinesReady>,
    mut assets_state: ResMut<VisualAssetsState>,
    mut visual: NonSendMut<VisualState>,
) {
    if assets_state.ready {
        return;
    }

    let mut all_loaded = true;

    for (name, handle) in &handles.0 {
        match asset_server.get_load_state(handle.id()) {
            Some(LoadState::Loaded) => {}
            Some(LoadState::Failed(err)) => {
                panic!("failed to load sprite asset {name}: {err}");
            }
            _ => {
                all_loaded = false;
            }
        }
    }

    if !all_loaded || !pipelines_ready.0 {
        assets_state.confirmation_frames = 0;
        return;
    }

    assets_state.confirmation_frames += 1;
    if assets_state.confirmation_frames >= 5 {
        assets_state.ready = true;
        visual.dirty = true;
    }
}

fn update_pipelines_ready(mut main_world: ResMut<MainWorld>, pipelines: Res<PipelineCache>) {
    if let Some(mut ready) = main_world.get_resource_mut::<PipelinesReady>() {
        ready.0 = pipelines.waiting_pipelines().count() == 0;
    }
}

fn refresh_visual_capture_state(
    screenshot_requests: Query<Entity, With<Screenshot>>,
    mut capture_state: ResMut<VisualCaptureState>,
) {
    if screenshot_requests.is_empty()
        && let Some(request) = capture_state.active_request.take()
    {
        match request.kind {
            VisualCaptureKind::Start | VisualCaptureKind::Result => {}
            VisualCaptureKind::Turn { turn } => {
                let _ = turn;
            }
            VisualCaptureKind::Heatmap { turn, agent } => {
                let _ = (turn, agent);
            }
        }
    }
}

fn queue_base_visual_captures(
    assets_state: Res<VisualAssetsState>,
    mode: Res<RuntimeMode>,
    mut capture_state: ResMut<VisualCaptureState>,
    visual: NonSendMut<VisualState>,
) {
    if !assets_state.ready {
        return;
    }

    let capture_snapshots = *mode == RuntimeMode::Visual || config().analytics.screenshot;
    if !capture_snapshots {
        capture_state.start_requested = true;
        capture_state.result_requested = true;
        return;
    }

    if !capture_state.start_requested {
        enqueue_visual_capture(
            &mut capture_state.queue,
            VisualCaptureRequest {
                kind: VisualCaptureKind::Start,
                path: PathBuf::from(visual.sim.output_dir()).join("start.png"),
            },
        );
        capture_state.start_requested = true;
    }

    if config().analytics.screenshot
        && visual.pending_step.is_none()
        && !visual.sim.is_finalized()
        && visual.sim.at_turn_start()
        && capture_state.last_turn_requested != Some(visual.sim.turn())
    {
        let turn = visual.sim.turn();
        enqueue_visual_capture(
            &mut capture_state.queue,
            VisualCaptureRequest {
                kind: VisualCaptureKind::Turn { turn },
                path: PathBuf::from(visual.sim.output_dir())
                    .join("screenshots")
                    .join(format!("turn{:06}.png", turn)),
            },
        );
        capture_state.last_turn_requested = Some(turn);
    }

    if visual.sim.is_finalized() && !capture_state.result_requested {
        enqueue_visual_capture(
            &mut capture_state.queue,
            VisualCaptureRequest {
                kind: VisualCaptureKind::Result,
                path: PathBuf::from(visual.sim.output_dir()).join("result.png"),
            },
        );
        capture_state.result_requested = true;
        capture_state.exit_when_done = true;
    }
}

fn advance_visual_simulation(
    input: Res<ButtonInput<KeyCode>>,
    assets_state: Res<VisualAssetsState>,
    capture_state: Res<VisualCaptureState>,
    screenshot_requests: Query<Entity, With<Screenshot>>,
    mut visual: NonSendMut<VisualState>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if !assets_state.ready {
        window.title = format!("{} - loading", name());
        return;
    }

    window.title = visual.sim.window_title();

    if visual.sim.is_finalized() {
        return;
    }

    if !capture_state.start_requested
        || !capture_state.queue.is_empty()
        || capture_state.active_request.is_some()
        || !screenshot_requests.is_empty()
    {
        return;
    }

    if let Some(step) = visual.pending_step.take() {
        visual.sim.apply_prepared_step(step);
        visual.dirty = true;
        window.title = visual.sim.window_title();
        return;
    }

    let should_step = !visual.sim.interactive() || input.pressed(KeyCode::Enter);
    if should_step && let Some(step) = visual.sim.prepare_step() {
        if step.heatmap().is_some() {
            visual.pending_step = Some(step);
        } else {
            visual.sim.apply_prepared_step(step);
        }
        visual.dirty = true;
        window.title = visual.sim.window_title();
    }
}

fn queue_pending_step_captures(
    assets_state: Res<VisualAssetsState>,
    mut capture_state: ResMut<VisualCaptureState>,
    visual: NonSendMut<VisualState>,
) {
    if !assets_state.ready {
        return;
    }

    let Some(step) = visual.pending_step.as_ref() else {
        return;
    };

    let key = (step.turn(), step.agent());
    if step.heatmap().is_some() && capture_state.last_heatmap_requested != Some(key) {
        enqueue_visual_capture(
            &mut capture_state.queue,
            VisualCaptureRequest {
                kind: VisualCaptureKind::Heatmap {
                    turn: step.turn(),
                    agent: step.agent(),
                },
                path: PathBuf::from(visual.sim.output_dir())
                    .join("heatmaps")
                    .join(format!("agent{}", step.agent().0))
                    .join(format!("{:06}.png", step.turn())),
            },
        );
        capture_state.last_heatmap_requested = Some(key);
    }
}

fn sync_visual_scene(
    mut commands: Commands,
    assets_state: Res<VisualAssetsState>,
    camera: Res<SceneCamera>,
    mut visual: NonSendMut<VisualState>,
    handles: Res<SpriteHandles>,
    existing: Query<Entity, With<VisualEntity>>,
) {
    if !visual.dirty {
        return;
    }

    for entity in existing.iter() {
        commands.entity(entity).despawn();
    }

    if !assets_state.ready {
        commands.spawn((
            VisualEntity,
            Text::new("Loading..."),
            TextFont {
                font_size: SPRITE_SIZE * 0.9,
                ..default()
            },
            TextColor(Color::WHITE),
            UiTargetCamera(camera.0),
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                top: Val::Px(12.0),
                ..default()
            },
        ));
        visual.dirty = false;
        return;
    }

    let world = visual.sim.world();
    let sprite_size = SPRITE_SIZE;
    let padding = config().display.padding;
    let inventory_rows = inventory_rows(world);
    let total_width = (world.map.width + 2 * padding.0) as f32 * sprite_size;
    let total_height =
        (world.map.height + 2 * padding.1.max(world.inventory.0.len())) as f32 * sprite_size;
    let screen_left = -total_width / 2.0;
    let screen_top = total_height / 2.0;
    let origin_x = -total_width / 2.0 + padding.0 as f32 * sprite_size + sprite_size / 2.0;
    let origin_y = total_height / 2.0 - padding.1 as f32 * sprite_size - sprite_size / 2.0;

    for (row, tiles) in world.map.tiles.iter().enumerate() {
        for (col, tile) in tiles.iter().enumerate() {
            let Some(sprite_name) = sprite_name_for_tile(tile, &world.actions) else {
                continue;
            };
            let Some(handle) = handles.0.get(sprite_name.as_str()) else {
                continue;
            };

            commands.spawn((
                VisualEntity,
                Sprite::from_image(handle.clone()),
                Transform::from_xyz(
                    origin_x + col as f32 * sprite_size,
                    origin_y - row as f32 * sprite_size,
                    0.0,
                ),
            ));
        }
    }

    if let Some(step) = visual.pending_step.as_ref()
        && let Some(heatmap) = step.heatmap()
    {
        for cell in &heatmap.cells {
            commands.spawn((
                VisualEntity,
                Sprite::from_color(
                    Color::srgba(cell.red, cell.green, 0.0, cell.alpha),
                    Vec2::splat(sprite_size),
                ),
                Transform::from_xyz(
                    origin_x + cell.x as f32 * sprite_size,
                    origin_y - cell.y as f32 * sprite_size,
                    0.5,
                ),
            ));
        }
    }

    if config().display.inventory {
        for (row, (agent, wood, water)) in inventory_rows.into_iter().enumerate() {
            let Some(handle) = handles.0.get(crate::assets::inventory_sprite_name(agent)) else {
                continue;
            };

            commands.spawn((
                VisualEntity,
                Sprite::from_image(handle.clone()),
                Transform::from_xyz(
                    screen_left + sprite_size / 2.0,
                    screen_top - row as f32 * sprite_size - sprite_size / 2.0,
                    1.0,
                ),
            ));

            commands.spawn((
                VisualEntity,
                Text::new(format!(":{wood}, {water}")),
                TextFont {
                    font_size: sprite_size * 0.6,
                    ..default()
                },
                TextColor(Color::WHITE),
                UiTargetCamera(camera.0),
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(sprite_size),
                    top: Val::Px(row as f32 * sprite_size + sprite_size * 0.15),
                    ..default()
                },
            ));
        }
    }

    commands.spawn((
        VisualEntity,
        Text::new(format!("Turn: {}", visual.sim.turn())),
        TextFont {
            font_size: sprite_size * 0.7,
            ..default()
        },
        TextColor(Color::WHITE),
        UiTargetCamera(camera.0),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(5.0 * sprite_size),
            top: Val::Px(0.0),
            ..default()
        },
    ));

    visual.dirty = false;
}

fn issue_capture_requests(
    mut commands: Commands,
    capture_source: Res<CaptureSource>,
    mut capture_state: ResMut<VisualCaptureState>,
    screenshot_requests: Query<Entity, With<Screenshot>>,
) {
    if capture_state.active_request.is_some() || !screenshot_requests.is_empty() {
        return;
    }

    let Some(request) = capture_state.queue.pop_front() else {
        return;
    };
    let path = request.path.clone();
    capture_state.active_request = Some(request);

    match capture_source.as_ref() {
        CaptureSource::PrimaryWindow => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path));
        }
        CaptureSource::Image(image) => {
            commands
                .spawn(Screenshot::image(image.clone()))
                .observe(save_to_disk(path));
        }
    }
}

fn advance_headless_batch(
    assets_state: Res<VisualAssetsState>,
    screenshot_requests: Query<Entity, With<Screenshot>>,
    mut transition: HeadlessBatchTransition,
) {
    if !assets_state.ready
        || !transition.visual.sim.is_finalized()
        || (config().analytics.screenshot && !transition.capture_state.result_requested)
        || !transition.capture_state.is_idle()
        || !screenshot_requests.is_empty()
    {
        return;
    }

    let Some(run) = transition.batch_state.take_next_run() else {
        return;
    };

    let sim = build_headless_simulation(run);
    let (width, height) = surface_size(&sim);
    if transition.render_target.width != width || transition.render_target.height != height {
        let target = create_render_target(&mut transition.images, width, height);
        let mut target_component = transition
            .render_targets
            .get_mut(transition.scene_camera.0)
            .expect("headless scene camera should exist");
        *target_component = RenderTarget::Image(target.clone().into());
        *transition.capture_source = CaptureSource::Image(target);
        transition.render_target.width = width;
        transition.render_target.height = height;
    }

    transition.visual.replace_sim(sim);
    transition
        .capture_state
        .reset_for_mode(RuntimeMode::Headless);
}

fn exit_after_visual_captures(
    capture_state: Res<VisualCaptureState>,
    screenshot_requests: Query<Entity, With<Screenshot>>,
    mut exit: MessageWriter<AppExit>,
) {
    if capture_state.exit_when_done
        && capture_state.queue.is_empty()
        && capture_state.active_request.is_none()
        && screenshot_requests.is_empty()
    {
        exit.write(AppExit::Success);
    }
}

fn advance_headless_simulation(
    assets_state: Res<VisualAssetsState>,
    capture_state: Res<VisualCaptureState>,
    screenshot_requests: Query<Entity, With<Screenshot>>,
    mut visual: NonSendMut<VisualState>,
) {
    if !assets_state.ready || visual.sim.is_finalized() {
        return;
    }

    if !capture_state.start_requested
        || !capture_state.queue.is_empty()
        || capture_state.active_request.is_some()
        || !screenshot_requests.is_empty()
    {
        return;
    }

    if let Some(step) = visual.pending_step.take() {
        visual.sim.apply_prepared_step(step);
        visual.dirty = true;
        return;
    }

    if let Some(step) = visual.sim.prepare_step() {
        if step.heatmap().is_some() {
            visual.pending_step = Some(step);
        } else {
            visual.sim.apply_prepared_step(step);
        }
        visual.dirty = true;
    }
}

fn exit_headless_when_done(
    batch_state: Res<HeadlessBatchState>,
    capture_state: Res<VisualCaptureState>,
    screenshot_requests: Query<Entity, With<Screenshot>>,
    visual: NonSend<VisualState>,
    mut exit: MessageWriter<AppExit>,
) {
    if batch_state.has_pending_runs() {
        return;
    }

    if config().analytics.screenshot && !capture_state.result_requested {
        return;
    }

    if visual.sim.is_finalized() && capture_state.is_idle() && screenshot_requests.is_empty() {
        exit.write(AppExit::Success);
    }
}
