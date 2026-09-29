use jiff::Zoned;
use jiff::tz::TimeZone;
use std::ffi::CString;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Monotonic elapsed-time accumulator shared by the timer and the stopwatch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counter {
    base: Duration,
    started: Option<Instant>,
}

impl Counter {
    pub fn elapsed(&self, now: Instant) -> Duration {
        self.base
            + self
                .started
                .map_or(Duration::ZERO, |s| now.saturating_duration_since(s))
    }

    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    pub fn start(&mut self, now: Instant) {
        self.started.get_or_insert(now);
    }

    pub fn stop(&mut self, now: Instant) {
        self.base = self.elapsed(now);
        self.started = None;
    }

    pub fn toggle(&mut self, now: Instant) {
        if self.running() {
            self.stop(now)
        } else {
            self.start(now)
        }
    }

    pub fn clear(&mut self) {
        *self = Counter::default();
    }
}

/// Hours, minutes, seconds of a duration, hours capped at two digits.
pub fn hms(d: Duration) -> [u8; 3] {
    let s = d.as_secs();
    [(s / 3600 % 100) as u8, (s / 60 % 60) as u8, (s % 60) as u8]
}

pub fn time_of_day(z: &Zoned, hour24: bool) -> [u8; 3] {
    let h = z.hour() as u8;
    let h = if hour24 { h } else { (h + 11) % 12 + 1 };
    [h, z.minute() as u8, z.second() as u8]
}

pub fn now_in(tz: &str) -> Zoned {
    if tz.is_empty() {
        return Zoned::now();
    }
    match TimeZone::get(tz) {
        Ok(tz) => jiff::Timestamp::now().to_zoned(tz),
        _ => Zoned::now(),
    }
}

static LOCALE: Mutex<Option<String>> = Mutex::new(None);

/// Switch LC_TIME. An empty name means the environment's locale. Returns false if unsupported.
pub fn set_locale(name: &str) -> bool {
    let mut cur = LOCALE.lock().unwrap_or_else(|e| e.into_inner());
    if cur.as_deref() == Some(name) {
        return true;
    }
    let Ok(c) = CString::new(name) else {
        return false;
    };
    // SAFETY: setlocale is only called from here, under the LOCALE lock.
    let ok = unsafe { !libc::setlocale(libc::LC_TIME, c.as_ptr()).is_null() };
    if ok {
        *cur = Some(name.to_string());
    }
    ok
}

/// strftime with the current LC_TIME locale, fed from a jiff time so any timezone works.
pub fn format_date(fmt: &str, z: &Zoned) -> String {
    if fmt.is_empty() {
        return String::new();
    }
    let abbrev = z
        .time_zone()
        .to_offset_info(z.timestamp())
        .abbreviation()
        .to_string();
    let (Ok(cfmt), Ok(zone)) = (CString::new(fmt), CString::new(abbrev)) else {
        return String::new();
    };
    // SAFETY: tm is plain data; zeroed is a valid bit pattern for every field.
    let mut tm: libc::tm = unsafe { std::mem::zeroed() };
    tm.tm_sec = z.second() as i32;
    tm.tm_min = z.minute() as i32;
    tm.tm_hour = z.hour() as i32;
    tm.tm_mday = z.day() as i32;
    tm.tm_mon = z.month() as i32 - 1;
    tm.tm_year = z.year() as i32 - 1900;
    tm.tm_wday = z.weekday().to_sunday_zero_offset() as i32;
    tm.tm_yday = z.day_of_year() as i32 - 1;
    tm.tm_gmtoff = z.offset().seconds() as _;
    tm.tm_zone = zone.as_ptr() as _;
    let mut buf = [0u8; 512];
    let _guard = LOCALE.lock().unwrap_or_else(|e| e.into_inner());
    // SAFETY: buf is writable for its length, cfmt and zone are NUL-terminated and outlive the call.
    let n = unsafe { libc::strftime(buf.as_mut_ptr() as *mut _, buf.len(), cfmt.as_ptr(), &tm) };
    String::from_utf8_lossy(&buf[..n]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: &str) -> Zoned {
        s.parse().unwrap()
    }

    #[test]
    fn counter_runs_pauses_and_clears() {
        let t0 = Instant::now();
        let mut c = Counter::default();
        c.start(t0);
        assert_eq!(
            c.elapsed(t0 + Duration::from_secs(5)),
            Duration::from_secs(5)
        );
        c.stop(t0 + Duration::from_secs(5));
        assert_eq!(
            c.elapsed(t0 + Duration::from_secs(100)),
            Duration::from_secs(5)
        );
        c.toggle(t0 + Duration::from_secs(100));
        assert_eq!(
            c.elapsed(t0 + Duration::from_secs(102)),
            Duration::from_secs(7)
        );
        c.start(t0 + Duration::from_secs(200));
        assert_eq!(
            c.elapsed(t0 + Duration::from_secs(102)),
            Duration::from_secs(7)
        );
        c.clear();
        assert_eq!(
            (c.elapsed(t0 + Duration::from_secs(300)), c.running()),
            (Duration::ZERO, false)
        );
    }

    #[test]
    fn twelve_and_twenty_four_hour() {
        let midnight = at("2026-01-01T00:05:09[UTC]");
        let noon = at("2026-01-01T12:00:00[UTC]");
        let evening = at("2026-01-01T23:59:59[UTC]");
        assert_eq!(time_of_day(&midnight, true), [0, 5, 9]);
        assert_eq!(time_of_day(&midnight, false), [12, 5, 9]);
        assert_eq!(time_of_day(&noon, false), [12, 0, 0]);
        assert_eq!(time_of_day(&evening, false), [11, 59, 59]);
        assert_eq!(time_of_day(&evening, true), [23, 59, 59]);
    }

    #[test]
    fn hms_splits_duration() {
        assert_eq!(hms(Duration::from_secs(3 * 3600 + 25 * 60 + 7)), [3, 25, 7]);
        assert_eq!(hms(Duration::from_secs(123 * 3600)), [23, 0, 0]);
    }

    #[test]
    fn formats_dates_in_c_locale() {
        assert!(set_locale("C"));
        let z = at("2026-09-29T14:03:00-07:00[America/Vancouver]");
        assert_eq!(
            format_date("%a %b %d %Y %H:%M %p %Z %j", &z),
            "Tue Sep 29 2026 14:03 PM PDT 272"
        );
        assert_eq!(format_date("", &z), "");
        assert!(!set_locale("xx_NOPE.UTF-8"));
    }

    #[test]
    fn now_in_honours_timezone() {
        assert_eq!(now_in("Asia/Kolkata").offset().seconds(), 5 * 3600 + 1800);
        assert_eq!(now_in("").time_zone(), &TimeZone::system());
    }
}
