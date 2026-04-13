use crate::parser::Schedule;
use chrono::Local;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::sleep;
use std::time::Duration;

pub fn run(
    name: Option<&str>,
    schedule: &Schedule,
    cmd: &str,
    args: &[String],
    stop_signal: Arc<AtomicBool>,
) {
    let job_name = name.unwrap_or(cmd);
    println!("🗓  Job [{}]: {}", job_name, schedule.description());
    if schedule.is_one_shot() {
        println!("ℹ️  [{}] One-shot — will exit after firing once.", job_name);
    }
    println!("   Press Ctrl+C to cancel.\n");

    loop {
        if stop_signal.load(Ordering::Relaxed) {
            break;
        }

        let now = Local::now();
        let next = schedule.next_fire(now);
        let wait_secs = (next - now).num_seconds().max(0) as u64;

        println!(
            "⏳ [{}] Next run: {} (in {})",
            job_name,
            next.format("%a, %b %-d at %-I:%M%P"),
            humanize(wait_secs)
        );

        // Sleep in chunks so Ctrl+C and stop signal are responsive.
        let mut remaining = wait_secs;
        while remaining > 0 {
            if stop_signal.load(Ordering::Relaxed) {
                return;
            }
            let chunk = remaining.min(1); // Check every second for stop signal
            sleep(Duration::from_secs(chunk));
            remaining = remaining.saturating_sub(chunk);
        }

        // Spin-wait for the exact second
        loop {
            if stop_signal.load(Ordering::Relaxed) {
                return;
            }
            if Local::now() >= next {
                break;
            }
            sleep(Duration::from_millis(100));
        }

        println!(
            "\n[{}] ▶ [{}] {} {}",
            Local::now().format("%Y-%m-%d %H:%M:%S"),
            job_name,
            cmd,
            args.join(" ")
        );

        let status = Command::new(cmd).args(args).status();
        match status {
            Ok(s) => println!("✅ [{}] Done ({})\n", job_name, s),
            Err(e) => eprintln!("❌ [{}] Failed to run '{}': {}\n", job_name, cmd, e),
        }

        if schedule.is_one_shot() {
            println!("ℹ️  [{}] One-shot schedule complete. Exiting.", job_name);
            break;
        }

        // Small buffer so it doesn't double-fire immediately if it wakes up slightly late
        let mut buffer = 2;
        while buffer > 0 {
            if stop_signal.load(Ordering::Relaxed) {
                return;
            }
            sleep(Duration::from_secs(1));
            buffer -= 1;
        }
    }
}

fn humanize(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m {}s", secs / 60, secs % 60)
    } else if secs < 86400 {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{}h {}m", h, m)
    } else {
        let d = secs / 86400;
        let h = (secs % 86400) / 3600;
        format!("{}d {}h", d, h)
    }
}
