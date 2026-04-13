# 🗓 schedule

A naturally simple job scheduler for your CLI. Define tasks using human-readable language like *"every monday at 2:30pm"* or *"the last Friday of every month at 5pm"*.

`schedule` runs as a persistent process, monitoring your defined tasks and executing them in parallel threads.

## Features

- **Natural Language Parsing**: No more cron syntax. Use plain English to define your schedules.
- **Concurrent Execution**: Jobs run in their own threads, ensuring one long-running task doesn't block others.
- **TOML Configuration**: Easy-to-read configuration file for managing multiple jobs.
- **Live Monitoring**: Clear console output showing next run times and job status.

## Installation

### Using Homebrew (Recommended)

```bash
brew tap mnishizawa/scheduler
brew install schedule
```

### Using Cargo

If you have Rust installed, you can install directly from source:

```bash
cargo install --path .
```

## Usage

By default, `schedule` looks for a configuration file at `~/.config/schedule/config.toml`.

```bash
# Start the scheduler using default config
schedule

# Start using a specific config file
schedule path/to/my-jobs.toml
```

### Configuration Example

Create your config file:

```toml
# ~/.config/schedule/config.toml

[[job]]
name     = "Database Backup"
schedule = "every day at 2am"
command  = "/usr/local/bin/backup-db.sh"
args     = ["--compress", "--remote"]

[[job]]
name     = "Weekly Report"
schedule = "every monday at 9:00am"
command  = "python3"
args     = ["scripts/generate_report.py"]
```

### Supported Schedule Patterns

`schedule` supports a wide variety of natural language expressions:

- **Specific Weekdays**: `every monday at 2:30pm`, `next friday at 10am`
- **General Days**: `every day at 8am`, `tomorrow at 9pm`
- **Intervals**: `every other tuesday at 4pm`
- **Relative Dates**: `the first monday at 9am`, `the last friday at 5pm`
- **Monthly Dates**: `every 15th at 9am`

## License

GNU General Public License v3.0 (GPL-3.0)
