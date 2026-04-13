use crate::parser::{DaySpec, Frequency, Ordinal, Schedule, TimeSpec, days_until_weekday};
use chrono::{Datelike, Duration, Local, NaiveDate, TimeZone, Weekday, DateTime};

// ── next_fire ──────────────────────────────────────────────────────────────

impl Schedule {
    /// Return the next DateTime at which this schedule should fire,
    /// calculated relative to `from`.
    pub fn next_fire(&self, from: DateTime<Local>) -> DateTime<Local> {
        match &self.frequency {
            Frequency::Next | Frequency::Every => self.next_recurring(from),
            Frequency::EveryOther { even_weeks } => self.next_every_other(from, *even_weeks),
            Frequency::Ordinal(ord) => self.next_ordinal(from, ord),
        }
    }

    /// Returns true if this schedule should only fire once (then exit).
    pub fn is_one_shot(&self) -> bool {
        matches!(self.frequency, Frequency::Next)
    }

    /// Human-readable description of the schedule.
    pub fn description(&self) -> String {
        let freq = match &self.frequency {
            Frequency::Next => "next".to_string(),
            Frequency::Every => "every".to_string(),
            Frequency::EveryOther { .. } => "every other".to_string(),
            Frequency::Ordinal(ord) => format!("the {}", ord),
        };
        let day = match &self.day {
            DaySpec::Weekday(w) => weekday_name(*w).to_string(),
            DaySpec::MonthDay(d) => format!("{}{}", d, ordinal_suffix(*d)),
            DaySpec::EveryDay => "day".to_string(),
        };
        let time = format_time(&self.time);
        format!("{} {} at {}", freq, day, time)
    }

    // ── internal helpers ───────────────────────────────────────────────────

    /// A DateTime for `from`'s calendar date at the schedule's wall-clock time.
    fn at_time_on(&self, date: NaiveDate) -> DateTime<Local> {
        let naive = date
            .and_hms_opt(self.time.hour as u32, self.time.minute as u32, 0)
            .expect("valid time");
        Local.from_local_datetime(&naive).single().expect("unambiguous local time")
    }

    fn next_recurring(&self, from: DateTime<Local>) -> DateTime<Local> {
        let today = self.at_time_on(from.date_naive());
        match &self.day {
            DaySpec::EveryDay => {
                if today > from { today } else { today + Duration::days(1) }
            }
            DaySpec::Weekday(w) => {
                let days = days_until_weekday(from.weekday(), *w) as i64;
                let candidate = today + Duration::days(days);
                if candidate > from { candidate } else { candidate + Duration::days(7) }
            }
            DaySpec::MonthDay(d) => self.next_month_day(from, *d),
        }
    }

    fn next_every_other(&self, from: DateTime<Local>, even_weeks: bool) -> DateTime<Local> {
        if let DaySpec::Weekday(w) = &self.day {
            let days = days_until_weekday(from.weekday(), *w) as i64;
            let today = self.at_time_on(from.date_naive());
            let mut candidate = today + Duration::days(days);
            if candidate <= from {
                candidate = candidate + Duration::days(7);
            }
            loop {
                let is_even = candidate.iso_week().week() % 2 == 0;
                if is_even == even_weeks {
                    return candidate;
                }
                candidate = candidate + Duration::days(7);
            }
        } else {
            self.next_recurring(from)
        }
    }

    fn next_ordinal(&self, from: DateTime<Local>, ord: &Ordinal) -> DateTime<Local> {
        if let DaySpec::Weekday(w) = &self.day {
            let mut month_start = from.date_naive().with_day(1).unwrap();
            loop {
                if let Some(dt) = nth_weekday_in_month(month_start, *w, ord, &self.time) {
                    if dt > from {
                        return dt;
                    }
                }
                month_start = next_month_start(month_start);
            }
        } else {
            self.next_recurring(from)
        }
    }

    fn next_month_day(&self, from: DateTime<Local>, d: u8) -> DateTime<Local> {
        let mut year = from.date_naive().year();
        let mut month = from.date_naive().month();
        loop {
            if let Some(date) = NaiveDate::from_ymd_opt(year, month, d as u32) {
                let dt = self.at_time_on(date);
                if dt > from {
                    return dt;
                }
            }
            // advance one month
            if month == 12 {
                month = 1;
                year += 1;
            } else {
                month += 1;
            }
        }
    }
}

// ── Module-level helpers ────────────────────────────────────────────────────

fn nth_weekday_in_month(
    month_start: NaiveDate,
    weekday: Weekday,
    ord: &Ordinal,
    time: &TimeSpec,
) -> Option<DateTime<Local>> {
    let month = month_start.month();
    let mut occurrences: Vec<NaiveDate> = Vec::new();
    let mut d = month_start;
    while d.month() == month {
        if d.weekday() == weekday {
            occurrences.push(d);
        }
        d = d.succ_opt()?;
    }
    let date = match ord {
        Ordinal::First => occurrences.get(0),
        Ordinal::Second => occurrences.get(1),
        Ordinal::Third => occurrences.get(2),
        Ordinal::Fourth => occurrences.get(3),
        Ordinal::Last => occurrences.last(),
    }?;
    let naive_dt = date.and_hms_opt(time.hour as u32, time.minute as u32, 0)?;
    Local.from_local_datetime(&naive_dt).single()
}

fn next_month_start(d: NaiveDate) -> NaiveDate {
    let (year, month) = if d.month() == 12 {
        (d.year() + 1, 1)
    } else {
        (d.year(), d.month() + 1)
    };
    NaiveDate::from_ymd_opt(year, month, 1).unwrap()
}

fn weekday_name(w: Weekday) -> &'static str {
    match w {
        Weekday::Mon => "monday",
        Weekday::Tue => "tuesday",
        Weekday::Wed => "wednesday",
        Weekday::Thu => "thursday",
        Weekday::Fri => "friday",
        Weekday::Sat => "saturday",
        Weekday::Sun => "sunday",
    }
}

fn ordinal_suffix(d: u8) -> &'static str {
    match d {
        1 | 21 | 31 => "st",
        2 | 22 => "nd",
        3 | 23 => "rd",
        _ => "th",
    }
}

pub fn format_time(t: &TimeSpec) -> String {
    if t.hour == 0 && t.minute == 0 {
        return "midnight".to_string();
    }
    if t.hour == 12 && t.minute == 0 {
        return "noon".to_string();
    }
    let (h, suffix) = if t.hour < 12 {
        (t.hour, "am")
    } else {
        (t.hour - 12, "pm")
    };
    if t.minute == 0 {
        format!("{}{}", h, suffix)
    } else {
        format!("{}:{:02}{}", h, t.minute, suffix)
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use chrono::{DateTime, Local, TimeZone};

    fn make_dt(year: i32, month: u32, day: u32, h: u32, m: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(year, month, day, h, m, 0)
            .single()
            .unwrap()
    }

    #[test]
    fn every_day_fires_today_if_future() {
        let s = parse("every day at 9am").unwrap();
        // 8am on a Monday → next fire is 9am today
        let from = make_dt(2025, 1, 6, 8, 0); // Mon 2025-01-06
        let next = s.next_fire(from);
        assert_eq!(next, make_dt(2025, 1, 6, 9, 0));
    }

    #[test]
    fn every_day_fires_tomorrow_if_past() {
        let s = parse("every day at 9am").unwrap();
        let from = make_dt(2025, 1, 6, 10, 0); // already past 9am
        let next = s.next_fire(from);
        assert_eq!(next, make_dt(2025, 1, 7, 9, 0));
    }

    #[test]
    fn every_weekday_correct_day() {
        // From Monday → next Friday
        let s = parse("every friday at 9am").unwrap();
        let from = make_dt(2025, 1, 6, 8, 0); // Mon
        let next = s.next_fire(from);
        assert_eq!(next, make_dt(2025, 1, 10, 9, 0)); // Fri
    }

    #[test]
    fn every_weekday_same_day_past_rolls_to_next_week() {
        let s = parse("every monday at 9am").unwrap();
        let from = make_dt(2025, 1, 6, 10, 0); // Mon 10am, past schedule
        let next = s.next_fire(from);
        assert_eq!(next, make_dt(2025, 1, 13, 9, 0)); // next Monday
    }

    #[test]
    fn next_is_one_shot() {
        let s = parse("next tuesday at 9am").unwrap();
        assert!(s.is_one_shot());
    }

    #[test]
    fn not_one_shot() {
        let s = parse("every monday at 9am").unwrap();
        assert!(!s.is_one_shot());
    }

    #[test]
    fn the_first_monday_of_month() {
        // From 2025-01-02 (Thu) — first Monday is 2025-01-06
        let s = parse("the first monday at 9am").unwrap();
        let from = make_dt(2025, 1, 2, 8, 0);
        let next = s.next_fire(from);
        assert_eq!(next, make_dt(2025, 1, 6, 9, 0));
    }

    #[test]
    fn the_last_friday_of_month() {
        // From 2025-01-01 — last Friday of Jan 2025 = 2025-01-31
        let s = parse("the last friday at 5pm").unwrap();
        let from = make_dt(2025, 1, 1, 8, 0);
        let next = s.next_fire(from);
        assert_eq!(next, make_dt(2025, 1, 31, 17, 0));
    }

    #[test]
    fn description_format() {
        let s = parse("every other friday at 2:30pm").unwrap();
        let desc = s.description();
        assert!(desc.starts_with("every other friday at"));
        assert!(desc.contains("2:30pm"));
    }
}
