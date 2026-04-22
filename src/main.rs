mod parser;
mod runner;
mod schedule;

use clap::Parser;
use notify::{Watcher, RecursiveMode, watcher, RawEvent};
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{env, fs, path::PathBuf, process::Command};
use std::sync::mpsc;
use std::time::Duration;

/// Natural language job scheduler
#[derive(Parser, Debug)]
#[clap(version, about, long_about = None)]
struct Args {
    /// Config path OR schedule expression
    #[clap(index = 1)]
    arg1: Option<String>,

    /// Command to run (if arg1 is a schedule)
    #[clap(index = 2)]
    arg2: Option<String>,

    /// Command arguments
    #[clap(index = 3, multiple_values = true)]
    rest: Vec<String>,

    /// Persist the job(s) to the default configuration after successful execution
    #[clap(long)]
    persist: bool,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct Config {
    #[serde(rename = "job", default)]
    jobs: Vec<JobConfig>,
}

#[derive(Deserialize, Serialize, Clone, Debug)]
struct JobConfig {
    name: Option<String>,
    schedule: String,
    command: String,
    #[serde(default)]
    args: Vec<String>,
}

fn main() {
    let args = Args::parse();
    let default_config_path = resolve_default_config_path();

    let (jobs, config_path_to_watch) = if let Some(a1) = &args.arg1 {
        // Isolated context: don't load default config if any args are provided.
        if let Some(a2) = &args.arg2 {
            // Case 3: schedule + command + [args]
            let mut job_args = Vec::new();
            job_args.push(a2.clone());
            job_args.extend(args.rest.clone());
            
            let command = job_args.remove(0);
            
            let job = JobConfig {
                name: None,
                schedule: a1.clone(),
                command,
                args: job_args,
            };
            (vec![job], None)
        } else {
            // Case 2: specific config path
            let path = PathBuf::from(a1);
            let config = load_config(&path);
            (config.jobs, Some(path))
        }
    } else {
        // Case 1: no args, run default config
        let config = load_config(&default_config_path);
        (config.jobs, Some(default_config_path.clone()))
    };

    println!("🔍 Validating configured jobs...");
    validate_jobs(&jobs);

    if args.persist {
        // "Test-and-Save" logic:
        // Execute the command(s) once immediately.
        // If successful, append to default config and exit.
        println!("🚀 Verifying job(s) before persisting...");
        for job in &jobs {
            let status = Command::new(&job.command)
                .args(&job.args)
                .status()
                .unwrap_or_else(|e| {
                    eprintln!("❌ Failed to start command '{}': {}", job.command, e);
                    std::process::exit(1);
                });

            if !status.success() {
                eprintln!("❌ Command failed with status: {}. Aborting persistence.", status);
                std::process::exit(1);
            }
        }

        println!("✅ Verification successful. Persisting to {}...", default_config_path.display());
        persist_jobs(&jobs, &default_config_path);
        println!("✨ Jobs persisted. Exiting.");
        return;
    }

    // Main execution loop with hot-reloading
    let stop_signal = Arc::new(AtomicBool::new(false));
    let (tx_notify, rx_notify) = mpsc::channel();

    let mut config_path_to_watch_current = config_path_to_watch;
    let mut current_jobs = jobs;

    let mut _watcher = None;
    if let Some(path) = &config_path_to_watch_current {
        let mut watcher = watcher(tx_notify, Duration::from_millis(500)).unwrap();
        let _ = watcher.watch(path, RecursiveMode::NonRecursive);
        _watcher = Some(watcher);
    }

    loop {
        stop_signal.store(false, Ordering::Relaxed);
        let mut handles = Vec::new();

        println!("🔄 Starting schedule with {} jobs...", current_jobs.len());
        
        for job in current_jobs.clone() {
            let stop = Arc::clone(&stop_signal);
            let sched = parser::parse(&job.schedule).unwrap_or_else(|e| {
                eprintln!("❌ Invalid schedule expression '{}': {}", job.schedule, e);
                std::process::exit(1);
            });

            let handle = std::thread::spawn(move || {
                runner::run(job.name.as_deref(), &sched, &job.command, &job.args, stop);
            });
            handles.push(handle);
        }

        // Wait for a reload signal or for all jobs to finish (one-shots)
        let mut reload = false;
        while !reload {
            match rx_notify.recv_timeout(Duration::from_millis(100)) {
                Ok(_) => {
                    println!("\n🔔 Config change detected! Reloading...");
                    reload = true;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
            // Check if all handles finished (only happens if all are one-shots)
            if handles.iter().all(|h| h.is_finished()) {
                break;
            }
        }

        stop_signal.store(true, Ordering::Relaxed);
        for handle in handles {
            let _ = handle.join();
        }

        if reload {
            // Re-load the config and continue the loop
            if let Some(path) = &config_path_to_watch_current {
                // Wait a bit for file write to stabilize
                std::thread::sleep(Duration::from_millis(500));
                let new_config = load_config(path);
                current_jobs = new_config.jobs;
                
                println!("🔍 Validating reloaded jobs...");
                validate_jobs(&current_jobs);
            }
        } else {
            break;
        }
    }
}

fn load_config(path: &PathBuf) -> Config {
    if !path.exists() {
        return Config { jobs: Vec::new() };
    }
    let raw = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("❌ Could not read '{}': {}", path.display(), e);
        std::process::exit(1);
    });
    toml::from_str(&raw).unwrap_or_else(|e| {
        eprintln!("❌ Invalid config '{}': {}", path.display(), e);
        std::process::exit(1);
    })
}

fn resolve_default_config_path() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("schedule")
        .join("config.toml")
}

fn persist_jobs(new_jobs: &[JobConfig], path: &PathBuf) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    let mut config = if path.exists() {
        load_config(path)
    } else {
        Config { jobs: Vec::new() }
    };

    config.jobs.extend(new_jobs.iter().cloned());
    
    let toml_string = toml::to_string_pretty(&config).expect("valid toml");
    fs::write(path, toml_string).unwrap_or_else(|e| {
        eprintln!("❌ Failed to write config to '{}': {}", path.display(), e);
        std::process::exit(1);
    });
}

fn validate_jobs(jobs: &[JobConfig]) {
    for job in jobs {
        let job_name = job.name.as_deref().unwrap_or(&job.command);
        let trimmed_cmd = job.command.trim();

        if trimmed_cmd != job.command {
            eprintln!(
                "⚠️  Warning: Job '{}' has leading or trailing whitespace in its command (\"{}\"). This will likely cause execution to fail.",
                job_name, job.command
            );
        } else if which::which(&job.command).is_err() {
            eprintln!(
                "⚠️  Warning: Command '{}' for job '{}' was not found in PATH or is not executable. It may fail when scheduled.",
                job.command, job_name
            );
        }

        for (i, arg) in job.args.iter().enumerate() {
            if arg.trim() != arg {
                eprintln!(
                    "⚠️  Warning: Argument {} (\"{}\") for job '{}' has leading or trailing whitespace. It may be interpreted literally by the program.",
                    i + 1, arg, job_name
                );
            }
            
            // Check if multiple arguments are accidentally combined into one string
            if arg.starts_with('-') && arg.contains(' ') && !arg.contains('=') && !arg.contains('{') {
                eprintln!(
                    "⚠️  Warning: Argument {} (\"{}\") for job '{}' looks like multiple arguments combined into one string. If this is a flag with a separate value, considering splitting it into multiple items in the args array.",
                    i + 1, arg, job_name
                );
            }
        }
    }
}
