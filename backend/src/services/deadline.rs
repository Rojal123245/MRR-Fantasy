//! When a gameweek's lineups stop counting.
//!
//! The league plays to one rule: a gameweek's deadline is the end of Saturday,
//! Eastern. [`handlers::teams::scheduled_lock_status_at`] tests an instant
//! against that boundary to decide whether selection is locked right now. This
//! module computes the same boundary forwards, so it can be stored on a
//! gameweek and compared in SQL.
//!
//! Both must always name the same instant, which
//! `handlers::teams::lock_schedule_tests::the_deadline_and_the_lock_agree`
//! pins.
//!
//! [`handlers::teams::scheduled_lock_status_at`]: crate::handlers::teams

use chrono::{DateTime, Datelike, Days, Utc};
use chrono_tz::America::New_York;
use chrono_tz::Tz;

/// The timezone the league's clock is read in.
pub const LEAGUE_TZ: Tz = New_York;

/// The deadline of a gameweek that opened at `opened_at`: the first Sunday
/// 00:00 Eastern strictly after it.
///
/// Sunday goes a full week forward, and that is the case that matters. A
/// gameweek opens when the previous one is scored, which is Sunday evening
/// after the noon reopening — gameweek 6 opened Sunday 2026-08-30 18:31 ET.
/// Its deadline is the *following* Saturday, not the midnight it is already
/// past. Anchoring on the calendar date rather than the hour is what gets this
/// right: `7 - days_from_sunday` is 7 on a Sunday and 1 on a Saturday, so the
/// result is always a strictly later date whatever the time of day.
///
/// Deliberately wall-clock Eastern: midnight Eastern is midnight Eastern
/// whatever the UTC offset is that week, so the deadline does not slide by an
/// hour across the two Sundays a year the clocks move.
pub fn deadline_after(opened_at: DateTime<Utc>) -> DateTime<Utc> {
    let local = opened_at.with_timezone(&LEAGUE_TZ);
    let ahead = 7 - local.weekday().num_days_from_sunday();

    local
        .date_naive()
        .checked_add_days(Days::new(ahead as u64))
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .and_then(|midnight| midnight.and_local_timezone(LEAGUE_TZ).single())
        // US transitions happen at 02:00, so Sunday 00:00 Eastern always exists
        // and never repeats. `.single()` cannot be None here.
        .expect("Sunday 00:00 Eastern is never skipped or repeated")
        .with_timezone(&Utc)
}

/// The deadline of a gameweek opening now.
pub fn next_deadline() -> DateTime<Utc> {
    deadline_after(Utc::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// An Eastern wall-clock instant. `earliest` resolves the hour that repeats
    /// when the clocks go back; times that do not exist are never constructed.
    fn et(y: i32, m: u32, d: u32, h: u32, min: u32) -> DateTime<Utc> {
        NaiveDate::from_ymd_opt(y, m, d)
            .expect("valid date")
            .and_hms_opt(h, min, 0)
            .expect("valid time")
            .and_local_timezone(LEAGUE_TZ)
            .earliest()
            .expect("a real Eastern instant")
            .with_timezone(&Utc)
    }

    /// Render as Eastern wall clock, which is how the rule is stated.
    fn as_et(t: DateTime<Utc>) -> String {
        t.with_timezone(&LEAGUE_TZ)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    }

    /// Every day of one week points at the same Sunday midnight — except the
    /// Sunday itself, which points at the next one.
    ///
    /// 2026-08-30 is a Sunday. Monday the 24th through Saturday the 29th are
    /// the week that ends on it.
    #[test]
    fn every_day_of_the_week_points_at_the_saturday_that_ends_it() {
        for day in 24..=29 {
            assert_eq!(
                as_et(deadline_after(et(2026, 8, day, 13, 0))),
                "2026-08-30 00:00",
                "a gameweek opened on 2026-08-{day} must close at the end of Saturday the 29th"
            );
        }
    }

    /// The production case this module exists for.
    ///
    /// Gameweek 5 opened Monday 2026-08-24 13:34 ET and was scored the
    /// following Sunday. Its lineups were frozen at the open, so six transfers
    /// made on the Saturday evening never reached the week they were made for.
    #[test]
    fn gameweek_five_closes_at_the_end_of_its_saturday() {
        assert_eq!(
            as_et(deadline_after(et(2026, 8, 24, 13, 34))),
            "2026-08-30 00:00"
        );
    }

    /// A gameweek that opens on a Sunday evening belongs to the *next*
    /// Saturday, not the midnight it has already passed.
    ///
    /// This is not hypothetical: scoring a week opens its successor, and
    /// scoring happens on Sunday evening. Gameweek 6 opened at 18:31 ET on
    /// Sunday 2026-08-30.
    #[test]
    fn a_week_opened_on_sunday_runs_to_the_following_saturday() {
        assert_eq!(
            as_et(deadline_after(et(2026, 8, 30, 18, 31))),
            "2026-09-06 00:00",
            "gameweek 6 opened Sunday evening and must run to Saturday the 5th"
        );
        // Sunday morning, inside the lock window, resolves the same way: that
        // week's deadline has passed either way.
        assert_eq!(
            as_et(deadline_after(et(2026, 8, 30, 3, 0))),
            "2026-09-06 00:00"
        );
    }

    /// A gameweek opened on a Saturday still gets that Saturday's deadline —
    /// the rest of the day, and no more.
    #[test]
    fn a_week_opened_on_saturday_closes_that_midnight() {
        assert_eq!(
            as_et(deadline_after(et(2026, 8, 29, 23, 30))),
            "2026-08-30 00:00"
        );
    }

    /// The deadline is midnight as a manager reads a clock, on both sides of
    /// the two Sundays a year the clocks move.
    ///
    /// 2026-11-01 and 2026-03-08 are the US transition Sundays. Spring forward
    /// skips 02:00-02:59 and autumn repeats 01:00-01:59, so 00:00 is untouched
    /// by both — but the UTC offset of that midnight differs, which is the
    /// thing a naive fixed-offset calculation gets wrong.
    #[test]
    fn the_deadline_is_midnight_eastern_across_both_clock_changes() {
        // The week running into the autumn transition.
        let autumn = deadline_after(et(2026, 10, 27, 12, 0));
        assert_eq!(as_et(autumn), "2026-11-01 00:00");
        assert_eq!(
            autumn.to_rfc3339(),
            "2026-11-01T04:00:00+00:00",
            "still EDT at midnight: the clocks go back at 02:00, not 00:00"
        );

        // The week running into the spring transition.
        let spring = deadline_after(et(2026, 3, 3, 12, 0));
        assert_eq!(as_et(spring), "2026-03-08 00:00");
        assert_eq!(
            spring.to_rfc3339(),
            "2026-03-08T05:00:00+00:00",
            "still EST at midnight: the clocks go forward at 02:00, not 00:00"
        );
    }

    /// The deadline is always in the future when the week opens, so a week can
    /// never be born already past its own deadline.
    #[test]
    fn the_deadline_is_always_after_the_open() {
        for day in 24..=30 {
            for hour in [0, 11, 12, 23] {
                let opened = et(2026, 8, day, hour, 0);
                assert!(
                    deadline_after(opened) > opened,
                    "2026-08-{day} {hour}:00 ET must precede its own deadline"
                );
            }
        }
    }
}
