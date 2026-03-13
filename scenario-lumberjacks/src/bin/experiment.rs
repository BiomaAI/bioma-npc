use std::fs;
use std::path::PathBuf;
use std::process;
use std::process::{Command, Stdio};

use clap::{Arg, Command as ClapCommand};
use serde_json::Value;

use lumberjacks::Experiment;

fn override_config(value: &mut Value, mut _override: Value) {
    match (value, &mut _override) {
        (Value::Null, Value::Null) => {}
        (Value::Bool(old), Value::Bool(new)) => *old = *new,
        (Value::Number(old), Value::Number(new)) => *old = new.clone(),
        (Value::String(old), Value::String(new)) => *old = new.clone(),
        (Value::Array(old), Value::Array(new)) => *old = new.clone(),
        (Value::Object(old), Value::Object(new)) => {
            for (key, value) in old.iter_mut() {
                if let Some(new_value) = new.remove(key) {
                    override_config(value, new_value);
                }
            }
            old.extend(new.iter().map(|(k, v)| (k.clone(), v.clone())));
        }
        (Value::Object(_), Value::Null) => {}
        _ => unreachable!(),
    }
}

fn main() {
    let matches = ClapCommand::new("Lumberjacks")
        .version("1.0")
        .author("Sven Knobloch")
        .arg(
            Arg::new("bin")
                .required(true)
                .help("Path to lumberjacks binary"),
        )
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
        .get_matches();

    let binary = matches.get_one::<String>("bin").unwrap();
    let experiment_file_path = matches.get_one::<String>("config").unwrap();
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.push(binary);

    let experiment_dir = {
        let mut path = PathBuf::from(&experiment_file_path);
        path.pop();
        path.to_str().unwrap().to_owned()
    };
    let output = matches
        .get_one::<String>("output")
        .unwrap_or(&experiment_dir)
        .to_owned();

    let experiment_file = match fs::OpenOptions::new().read(true).open(experiment_file_path) {
        Ok(file) => file,
        Err(e) => {
            println!(
                "Cannot open experiment file {}: {}",
                experiment_file_path, e
            );
            process::exit(1);
        }
    };
    let Experiment { base, trials, .. } = match serde_json::from_reader(&experiment_file) {
        Ok(experiment) => experiment,
        Err(e) => {
            println!(
                "Cannot read experiment file {}: {}",
                experiment_file_path, e
            );
            process::exit(2);
        }
    };

    let config_file_path = {
        let mut path = PathBuf::from(&experiment_dir);
        path.push(&base);
        path.to_str().unwrap().to_owned()
    };

    let config_file = fs::OpenOptions::new()
        .read(true)
        .open(&config_file_path)
        .map_err(|e| format!("Cannot open config file {}: {}", config_file_path, e))
        .unwrap();

    let config: Value = serde_json::from_reader(&config_file)
        .map_err(|e| format!("Cannot read config file {}: {}", config_file_path, e))
        .unwrap();

    let config_dir = {
        let mut path = PathBuf::from(&config_file_path);
        path.pop();
        path.to_str().unwrap().to_owned()
    };

    let working_dir = matches
        .get_one::<String>("working-dir")
        .unwrap_or(&config_dir)
        .to_owned();

    let children = match trials {
        Value::Object(trials) => trials
            .into_iter()
            .map(|(name, overrides)| {
                let mut config = config.clone();
                override_config(&mut config, overrides);

                println!("Running trial \"{}\"", name);
                let mut child = Command::new(path.to_str().unwrap())
                    .arg("-")
                    .arg(format!("--output={}/{}/", output, name))
                    .arg(format!("--working-dir={}", working_dir))
                    .arg(format!("--name={}", name))
                    .arg("--batch")
                    .stdin(Stdio::piped())
                    .spawn()
                    .expect("Failed to spawn child!");

                serde_json::to_writer(child.stdin.take().unwrap(), &config).unwrap();

                child
            })
            .collect::<Vec<_>>(),
        _ => unreachable!("Trial is not a valid json object!"),
    };

    children
        .into_iter()
        .try_fold((), |_res, mut child| child.wait().map(|_| ()))
        .expect("some experiment didn't exit successfully");
}
