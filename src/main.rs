mod parser;
mod runner;
mod schedule;

use serde::Deserialize;
use std::{env, fs, path::PathBuf};

/// Shape of the TOML config file.
///
/// Example schedule.toml:
///   schedule = "every monday at 2:30pm"
///   command  = "echo"
///   args     = ["hello", "world"]
#[derive(Deserialize)]
struct Config {
    #[serde(rename = "job", default)]
    jobs: Vec<JobConfig>,
}

#[derive(Deserialize, Clone)]
struct JobConfig {
    name: Option<String>,
    schedule: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

fn main() {
    let args: Vec<String> = env::args().collect();

    // Basic CLI flag handling
    if args.contains(&"--help".to_string()) || args.contains(&"-h".to_string()) {
        println!("schedule - Natural language job scheduler");
        println!();
        println!("USAGE:");
        println!("  schedule [CONFIG_PATH]");
        println!();
        println!("FLAGS:");
        println!("  -h, --help      Prints help information");
        println!("  -v, --version   Prints version information");
        println!();
        println!("ARGS:");
        println!("  [CONFIG_PATH]   Path to the config file (default: ~/.config/schedule/config.toml)");
        return;
    }

    if args.contains(&"--version".to_string()) || args.contains(&"-v".to_string()) {
        println!("schedule {}", env!("CARGO_PKG_VERSION"));
        return;
    }

    let config_path = resolve_config_path(&args);

    let raw = fs::read_to_string(&config_path).unwrap_or_else(|e| {
        eprintln!("❌ Could not read '{}': {}", config_path.display(), e);
        eprintln!();
        eprintln!("Create ~/.config/schedule/config.toml, e.g.:");
        eprintln!();
        eprintln!("  [[job]]");
        eprintln!("  name     = \"Hello Task\"");
        eprintln!("  schedule = \"every monday at 2:30pm\"");
        eprintln!("  command  = \"echo\"");
        eprintln!("  args     = [\"hello\", \"world\"]");
        std::process::exit(1);
    });

    let config: Config = toml::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("❌ Invalid config '{}': {}", config_path.display(), e);
        std::process::exit(1);
    });

    if config.jobs.is_empty() {
        eprintln!("❌ No [[job]] entries found in '{}'.", config_path.display());
        std::process::exit(1);
    }

    let mut handles = Vec::new();

    for job in config.jobs {
        let sched = parser::parse(&job.schedule).unwrap_or_else(|e| {
            eprintln!("❌ Invalid schedule expression '{}': {}", job.schedule, e);
            eprintln!();
            eprintln!("Supported patterns:");
            eprintln!("  next <weekday> at <time>");
            eprintln!("  every <weekday|day> at <time>");
            eprintln!("  every other <weekday> at <time>");
            eprintln!("  the <first|second|third|fourth|last> <weekday> at <time>");
            eprintln!("  every <Nth> at <time>          (e.g. \"every 15th at 9am\")");
            std::process::exit(1);
        });

        let handle = std::thread::spawn(move || {
            runner::run(job.name.as_deref(), &sched, &job.command, &job.args);
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }
}

/// Use the path given on the command line, or fall back to
/// `~/.config/schedule/config.toml`.
fn resolve_config_path(args: &[String]) -> PathBuf {
    if args.len() > 1 && !args[1].starts_with('-') {
        PathBuf::from(&args[1])
    } else {
        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home)
            .join(".config")
            .join("schedule")
            .join("config.toml")
    }
}
