use std::collections::HashMap;
use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::Once;
use std::{fs, io, mem, process};

use bevy::app::{AppExit, ScheduleRunnerPlugin};
use bevy::asset::{AssetPlugin, AssetServer, LoadState};
use bevy::prelude::{
    default, App, ButtonInput, Camera2d, ClearColor, Color, Commands, Component, DefaultPlugins,
    Entity, Handle, Image, IntoScheduleConfigs, KeyCode, MessageReader, MessageWriter,
    MinimalPlugins, Node, NonSendMut, PluginGroup, PositionType, Query, Res, ResMut, Resource,
    Single, Sprite, Startup, Text, TextColor, TextFont, Transform, Update, Val, With,
};
use bevy::render::{render_resource::PipelineCache, ExtractSchedule, MainWorld, RenderApp};
use bevy::window::{
    ExitCondition, PrimaryWindow, Window, WindowCloseRequested, WindowPlugin, WindowResolution,
};
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

use crate::assets::{sprite_name_for_tile, sprite_path, workspace_root, SPRITE_FILES};
use crate::simulation::{next_seed, SimulationState};

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
    dirty: bool,
}

#[derive(Resource, Default)]
struct VisualAssetsState {
    ready: bool,
    confirmation_frames: usize,
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

fn run_visual() {
    let sim = SimulationState::new(config().display.interactive, None, next_seed(), true);
    let width = ((2 * config().display.padding.0 + sim.width()) as f32 * SPRITE_SIZE).round()
        as u32;
    let height = ((2 * config().display.padding.1 + sim.height()) as f32 * SPRITE_SIZE).round()
        as u32;
    sim.dump_run();
    sim.write_start_artifacts();

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
        .insert_resource(ClearColor(Color::srgb(
            config().display.background.0,
            config().display.background.1,
            config().display.background.2,
        )))
        .insert_resource(VisualAssetsState::default())
        .insert_non_send_resource(VisualState { sim, dirty: true })
        .add_systems(Startup, (setup_camera, load_sprite_handles))
        .add_systems(
            Update,
            (
                handle_close_requests,
                update_visual_asset_state,
                advance_visual_simulation,
                sync_visual_scene,
            )
                .chain(),
        )
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
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
    mut visual: NonSendMut<VisualState>,
    mut exit: MessageWriter<AppExit>,
) {
    if requests.read().next().is_some() {
        visual.sim.finalize();
        exit.write(AppExit::Success);
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

fn advance_visual_simulation(
    input: Res<ButtonInput<KeyCode>>,
    assets_state: Res<VisualAssetsState>,
    mut visual: NonSendMut<VisualState>,
    mut exit: MessageWriter<AppExit>,
    mut window: Single<&mut Window, With<PrimaryWindow>>,
) {
    if !assets_state.ready {
        window.title = format!("{} - loading", name());
        return;
    }

    window.title = visual.sim.window_title();

    if visual.sim.is_finalized() {
        exit.write(AppExit::Success);
        return;
    }

    let should_step = !visual.sim.interactive() || input.pressed(KeyCode::Enter);
    if should_step {
        visual.sim.step();
        visual.dirty = true;
        window.title = visual.sim.window_title();
    }

    if visual.sim.is_finalized() {
        exit.write(AppExit::Success);
    }
}

fn sync_visual_scene(
    mut commands: Commands,
    assets_state: Res<VisualAssetsState>,
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
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(5.0 * sprite_size),
            top: Val::Px(0.0),
            ..default()
        },
    ));

    visual.dirty = false;
}
