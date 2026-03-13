use std::mem::MaybeUninit;
use std::path::PathBuf;
use std::sync::Once;
use std::{fs, io, mem, process};

use clap::{Arg, ArgAction, Command};

use serde_json::Value;

mod behaviors;
mod config;
mod fitnesses;
mod game;
mod graph;
mod heatmap;
mod hooks;
mod inventory;
mod lumberjacks_domain;
mod metrics;
mod screenshot;
mod serialization;
mod tasks;
mod tilemap;
mod util;
mod world;

pub use behaviors::*;
pub use config::*;
pub use game::*;
pub use graph::*;
pub use heatmap::*;
pub use hooks::*;
pub use inventory::*;
pub use lumberjacks_domain::*;
pub use metrics::*;
pub use screenshot::*;
pub use serialization::*;
pub use tasks::*;
pub use tilemap::*;
pub use util::*;
pub use world::*;

static INIT: Once = Once::new();
static mut CONFIG: MaybeUninit<Config> = MaybeUninit::uninit();
static mut WORKING_DIR: MaybeUninit<String> = MaybeUninit::uninit();
static mut OUTPUT_PATH: MaybeUninit<String> = MaybeUninit::uninit();
static mut NAME: MaybeUninit<String> = MaybeUninit::uninit();
static mut BATCH: MaybeUninit<bool> = MaybeUninit::uninit();

unsafe fn init() {
    INIT.call_once(|| {
        let matches = Command::new("Lumberjacks")
            .version("1.0")
            .author("Sven Knobloch")
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
                    .default_value("Lumberjacks")
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

        NAME = MaybeUninit::new(matches.get_one::<String>("name").unwrap().to_owned());

        OUTPUT_PATH = MaybeUninit::new(
            matches
                .get_one::<String>("output")
                .unwrap_or(&config_dir)
                .to_owned(),
        );

        WORKING_DIR = MaybeUninit::new(
            matches
                .get_one::<String>("working-dir")
                .unwrap_or(&config_dir)
                .to_owned(),
        );

        BATCH = MaybeUninit::new(matches.get_flag("batch"));

        CONFIG = MaybeUninit::new({
            let mut json: Value = match config_path.as_str() {
                "-" => {
                    let stdin = io::stdin();
                    serde_json::from_reader(stdin.lock()).unwrap()
                }
                path => {
                    let config_file = match fs::OpenOptions::new().read(true).open(path) {
                        Ok(file) => file,
                        Err(e) => {
                            println!("Cannot open config file {}: {}", path, e);
                            process::exit(1);
                        }
                    };
                    serde_json::from_reader(&config_file).unwrap()
                }
            };

            if let Some(values) = matches.get_many::<String>("set") {
                values.for_each(|value| {
                    let (k, v) = value
                        .split_once('=')
                        .unwrap_or_else(|| panic!("Invalid format, should be \"some.path=value\""));

                    let mut object = &mut json;

                    let mut keys = k.split('.').peekable();

                    while let Some(key) = keys.next() {
                        if keys.peek().is_some() {
                            // Path, get next map
                            let map = object
                                .as_object_mut()
                                .ok_or_else(|| format!("Invalid 'set' path: {}", k))
                                .unwrap();

                            // Key is not present or an object
                            if !matches!(map.get(key), Some(Value::Object(_))) {
                                map.insert(key.to_owned(), Value::Object(Default::default()));
                            }

                            object = map.get_mut(key).unwrap();
                        } else {
                            // Last element, insert into map
                            let map = object
                                .as_object_mut()
                                .ok_or_else(|| format!("Invalid 'set' path: {}", k))
                                .unwrap();

                            map.insert(
                                key.to_owned(),
                                serde_json::from_str(v)
                                    .map_err(|e| format!("'set' variable not valid: {}", e))
                                    .unwrap(),
                            );
                        }
                    }
                })
            }

            serde_json::from_value(json).unwrap()
        });
    })
}

pub fn name() -> &'static String {
    unsafe {
        init();
        // Safe to transmute, initialized
        #[allow(static_mut_refs)]
        mem::transmute(&NAME)
    }
}

pub fn config() -> &'static Config {
    unsafe {
        init();
        // Safe to transmute, initialized
        #[allow(static_mut_refs)]
        mem::transmute(&CONFIG)
    }
}

pub fn working_dir() -> &'static String {
    unsafe {
        init();
        // Safe to transmute, initialized
        #[allow(static_mut_refs)]
        mem::transmute(&WORKING_DIR)
    }
}

pub fn output_path() -> &'static String {
    unsafe {
        init();
        // Safe to transmute, initialized
        #[allow(static_mut_refs)]
        mem::transmute(&OUTPUT_PATH)
    }
}

pub fn batch() -> bool {
    unsafe {
        init();
        // Safe to transmute, initialized
        #[allow(static_mut_refs)]
        mem::transmute(BATCH)
    }
}
