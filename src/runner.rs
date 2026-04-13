use crate::parser::Schedule;
use chrono::Local;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;

pub fn run(name: Option<&str>, schedule: &Schedule, cmd: &str, args: &[String]) {
    let job_name = name.unwrap_or(cmd);
    println!("🗓  Job [{}]: {}", job_name, schedule.description());
    if schedule.is_one_shot() {
        println!("ℹ️  [{}] One-shot — will exit after firing once.", job_name);
    }
    println!("   Press Ctrl+C to cancel.\n");

    loop {
        let now = Local::now();
        let next = schedule.next_fire(now);
        let wait_secs = (next - now).num_seconds().max(0) as u64;

        println!(
            "⏳ [{}] Next run: {} (in {})",
            job_name,
            next.format("%a, %b %-d at %-I:%M%P"),
            humanize(wait_secs)
        );

        // Sleep in chunks so Ctrl+C is responsive and sleep imprecision is absorbed.
        let mut remaining = wait_secs;
        while remaining > 0 {
            let chunk = remaining.min(30);
            sleep(Duration::from_secs(chunk));
            remaining = remaining.saturating_sub(chunk);
        }

        // Spin-wait for the exact second in case sleep returned slightly early.
        loop {
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

        // Small buffer so a slow wake-up can't double-fire in the same minute.
        sleep(Duration::from_secs(2));
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
