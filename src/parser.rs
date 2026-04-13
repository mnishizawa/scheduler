use chrono::{Datelike, Duration, Local, TimeZone, Weekday};
use std::fmt;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Ordinal {
    First,
    Second,
    Third,
    Fourth,
    Last,
}

impl fmt::Display for Ordinal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ordinal::First => write!(f, "first"),
            Ordinal::Second => write!(f, "second"),
            Ordinal::Third => write!(f, "third"),
            Ordinal::Fourth => write!(f, "fourth"),
            Ordinal::Last => write!(f, "last"),
        }
    }
}

/// How often the schedule repeats (or fires once).
#[derive(Debug, Clone)]
pub enum Frequency {
    /// Fire once on the next matching occurrence, then exit.
    Next,
    /// Fire every matching occurrence.
    Every,
    /// Fire every other matching weekday (ISO week parity, stateless).
    EveryOther { even_weeks: bool },
    /// Fire on the Nth weekday of the month.
    Ordinal(Ordinal),
}

/// Which day(s) to fire on.
#[derive(Debug, Clone)]
pub enum DaySpec {
    Weekday(Weekday),
    MonthDay(u8),
    EveryDay,
}

#[derive(Debug, Clone)]
pub struct TimeSpec {
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone)]
pub struct Schedule {
    pub frequency: Frequency,
    pub day: DaySpec,
    pub time: TimeSpec,
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Parse a natural-language schedule expression.
///
/// Grammar:
///   <freq> <day> "at" <time>
///
///   freq  = "next" | "every" | "every other" | "the" <ordinal>
///   day   = weekday | "day" | Nth (e.g. "15th")
///   time  = "9am" | "2:30pm" | "noon" | "midnight" | "HH:MM" | "H"
pub fn parse(input: &str) -> Result<Schedule, String> {
    let tokens: Vec<&str> = input.split_whitespace().collect();
    let mut pos = 0;

    let raw_freq = parse_frequency(&tokens, &mut pos)?;
    let day = parse_day_spec(&tokens, &mut pos)?;

    match tokens.get(pos).map(|s| s.to_lowercase()).as_deref() {
        Some("at") => pos += 1,
        other => return Err(format!("Expected 'at', got {:?}", other)),
    }

    if pos >= tokens.len() {
        return Err("Expected time after 'at'".to_string());
    }
    let time = parse_time(tokens[pos])?;

    // Compute EveryOther parity now that we know the time.
    let frequency = if let Frequency::EveryOther { .. } = raw_freq {
        Frequency::EveryOther {
            even_weeks: first_occurrence_even_week(&day, &time),
        }
    } else {
        raw_freq
    };

    Ok(Schedule { frequency, day, time })
}

// ── Helpers (pub(crate)) ───────────────────────────────────────────────────

/// Days from `from` weekday to `to` weekday (0 = same day).
pub fn days_until_weekday(from: Weekday, to: Weekday) -> u32 {
    (to.num_days_from_monday() + 7 - from.num_days_from_monday()) % 7
}

// ── Internal parsers ───────────────────────────────────────────────────────

fn parse_frequency(tokens: &[&str], pos: &mut usize) -> Result<Frequency, String> {
    match tokens.get(*pos).map(|s| s.to_lowercase()).as_deref() {
        Some("next") => {
            *pos += 1;
            Ok(Frequency::Next)
        }
        Some("every") => {
            *pos += 1;
            if tokens.get(*pos).map(|s| s.to_lowercase()).as_deref() == Some("other") {
                *pos += 1;
                Ok(Frequency::EveryOther { even_weeks: false }) // fixed up later
            } else {
                Ok(Frequency::Every)
            }
        }
        Some("the") => {
            *pos += 1;
            let ord = match tokens.get(*pos).map(|s| s.to_lowercase()).as_deref() {
                Some("first") => Ordinal::First,
                Some("second") => Ordinal::Second,
                Some("third") => Ordinal::Third,
                Some("fourth") => Ordinal::Fourth,
                Some("last") => Ordinal::Last,
                other => return Err(format!("Expected ordinal after 'the', got {:?}", other)),
            };
            *pos += 1;
            Ok(Frequency::Ordinal(ord))
        }
        other => Err(format!(
            "Expected 'next', 'every', or 'the', got {:?}",
            other
        )),
    }
}

fn parse_day_spec(tokens: &[&str], pos: &mut usize) -> Result<DaySpec, String> {
    let token = tokens
        .get(*pos)
        .map(|s| s.to_lowercase())
        .ok_or_else(|| "Expected day specification".to_string())?;

    if let Some(w) = parse_weekday(&token) {
        *pos += 1;
        return Ok(DaySpec::Weekday(w));
    }
    if token == "day" {
        *pos += 1;
        return Ok(DaySpec::EveryDay);
    }
    if let Some(d) = parse_month_day(&token) {
        *pos += 1;
        return Ok(DaySpec::MonthDay(d));
    }

    Err(format!(
        "Expected weekday, 'day', or day-of-month (e.g. '15th'), got '{}'",
        token
    ))
}

fn parse_weekday(s: &str) -> Option<Weekday> {
    match s {
        "monday" | "mon" => Some(Weekday::Mon),
        "tuesday" | "tue" => Some(Weekday::Tue),
        "wednesday" | "wed" => Some(Weekday::Wed),
        "thursday" | "thu" => Some(Weekday::Thu),
        "friday" | "fri" => Some(Weekday::Fri),
        "saturday" | "sat" => Some(Weekday::Sat),
        "sunday" | "sun" => Some(Weekday::Sun),
        _ => None,
    }
}

fn parse_month_day(s: &str) -> Option<u8> {
    let stripped = s
        .trim_end_matches("st")
        .trim_end_matches("nd")
        .trim_end_matches("rd")
        .trim_end_matches("th");
    stripped
        .parse::<u8>()
        .ok()
        .filter(|&d| (1..=31).contains(&d))
}

pub fn parse_time(s: &str) -> Result<TimeSpec, String> {
    let s = s.to_lowercase();
    if s == "noon" {
        return Ok(TimeSpec { hour: 12, minute: 0 });
    }
    if s == "midnight" {
        return Ok(TimeSpec { hour: 0, minute: 0 });
    }
    if let Some(rest) = s.strip_suffix("pm") {
        let (h, m) = parse_hm(rest)?;
        let hour = if h == 12 { 12 } else { h + 12 };
        if hour > 23 {
            return Err(format!("Invalid hour: {}", hour));
        }
        return Ok(TimeSpec { hour, minute: m });
    }
    if let Some(rest) = s.strip_suffix("am") {
        let (h, m) = parse_hm(rest)?;
        let hour = if h == 12 { 0 } else { h };
        return Ok(TimeSpec { hour, minute: m });
    }
    // 24-hour format
    let (h, m) = parse_hm(&s)?;
    if h > 23 {
        return Err(format!("Invalid hour: {}", h));
    }
    Ok(TimeSpec { hour: h, minute: m })
}

fn parse_hm(s: &str) -> Result<(u8, u8), String> {
    if let Some((h_str, m_str)) = s.split_once(':') {
        let h = h_str
            .parse::<u8>()
            .map_err(|_| format!("Invalid hour '{}'", h_str))?;
        let m = m_str
            .parse::<u8>()
            .map_err(|_| format!("Invalid minute '{}'", m_str))?;
        if m > 59 {
            return Err(format!("Invalid minute: {}", m));
        }
        Ok((h, m))
    } else {
        let h = s
            .parse::<u8>()
            .map_err(|_| format!("Invalid time '{}'", s))?;
        Ok((h, 0))
    }
}

/// Determine whether the first future occurrence of this schedule falls on an
/// even ISO week number. Used to pin "every other" parity at parse time.
fn first_occurrence_even_week(day: &DaySpec, time: &TimeSpec) -> bool {
    let now = Local::now();
    let first = match day {
        DaySpec::Weekday(w) => {
            let days = days_until_weekday(now.weekday(), *w) as i64;
            let naive = now.date_naive() + Duration::days(days);
            let naive_dt = naive
                .and_hms_opt(time.hour as u32, time.minute as u32, 0)
                .unwrap();
            let candidate = Local.from_local_datetime(&naive_dt).single().unwrap();
            if candidate > now {
                candidate
            } else {
                // same weekday but already past — shift one week forward
                Local
                    .from_local_datetime(
                        &(naive + Duration::days(7))
                            .and_hms_opt(time.hour as u32, time.minute as u32, 0)
                            .unwrap(),
                    )
                    .single()
                    .unwrap()
            }
        }
        _ => now, // fallback: use current week
    };
    first.iso_week().week() % 2 == 0
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_every_monday_at_9am() {
        let s = parse("every monday at 9am").unwrap();
        assert!(matches!(s.frequency, Frequency::Every));
        assert!(matches!(s.day, DaySpec::Weekday(Weekday::Mon)));
        assert_eq!(s.time.hour, 9);
        assert_eq!(s.time.minute, 0);
    }

    #[test]
    fn test_next_tuesday_at_230pm() {
        let s = parse("next tuesday at 2:30pm").unwrap();
        assert!(matches!(s.frequency, Frequency::Next));
        assert!(matches!(s.day, DaySpec::Weekday(Weekday::Tue)));
        assert_eq!(s.time.hour, 14);
        assert_eq!(s.time.minute, 30);
    }

    #[test]
    fn test_every_other_friday_at_noon() {
        let s = parse("every other friday at noon").unwrap();
        assert!(matches!(s.frequency, Frequency::EveryOther { .. }));
        assert!(matches!(s.day, DaySpec::Weekday(Weekday::Fri)));
        assert_eq!(s.time.hour, 12);
    }

    #[test]
    fn test_the_first_wednesday_at_8am() {
        let s = parse("the first wednesday at 8am").unwrap();
        assert!(matches!(s.frequency, Frequency::Ordinal(Ordinal::First)));
        assert!(matches!(s.day, DaySpec::Weekday(Weekday::Wed)));
        assert_eq!(s.time.hour, 8);
    }

    #[test]
    fn test_the_last_thursday_at_5pm() {
        let s = parse("the last thursday at 5pm").unwrap();
        assert!(matches!(s.frequency, Frequency::Ordinal(Ordinal::Last)));
        assert!(matches!(s.day, DaySpec::Weekday(Weekday::Thu)));
        assert_eq!(s.time.hour, 17);
    }

    #[test]
    fn test_every_day_at_8am() {
        let s = parse("every day at 8am").unwrap();
        assert!(matches!(s.frequency, Frequency::Every));
        assert!(matches!(s.day, DaySpec::EveryDay));
        assert_eq!(s.time.hour, 8);
    }

    #[test]
    fn test_time_noon() {
        let t = parse_time("noon").unwrap();
        assert_eq!(t.hour, 12);
        assert_eq!(t.minute, 0);
    }

    #[test]
    fn test_time_midnight() {
        let t = parse_time("midnight").unwrap();
        assert_eq!(t.hour, 0);
        assert_eq!(t.minute, 0);
    }

    #[test]
    fn test_time_12am_is_midnight() {
        let t = parse_time("12am").unwrap();
        assert_eq!(t.hour, 0);
    }

    #[test]
    fn test_time_12pm_is_noon() {
        let t = parse_time("12pm").unwrap();
        assert_eq!(t.hour, 12);
    }

    #[test]
    fn test_time_24h() {
        let t = parse_time("14:30").unwrap();
        assert_eq!(t.hour, 14);
        assert_eq!(t.minute, 30);
    }

    #[test]
    fn test_month_day_15th() {
        let s = parse("every 15th at 9am").unwrap();
        assert!(matches!(s.day, DaySpec::MonthDay(15)));
    }

    #[test]
    fn test_short_weekday_abbrevs() {
        let s = parse("every fri at 6pm").unwrap();
        assert!(matches!(s.day, DaySpec::Weekday(Weekday::Fri)));
        assert_eq!(s.time.hour, 18);
    }
}
