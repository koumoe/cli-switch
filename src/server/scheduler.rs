use std::time::Duration;

use tokio::sync::watch;

const LOCAL_TIME_FORMAT: &str = "[hour]:[minute]:[second]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WaitOutcome {
    Fired,
    Reconfigured,
    Closed,
}

impl WaitOutcome {
    pub(crate) const fn closed(self) -> bool {
        matches!(self, Self::Closed)
    }
}

pub(crate) trait TriggerSpec {
    fn next_deadline(&self, now: time::OffsetDateTime) -> time::OffsetDateTime;
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct IntervalTrigger {
    interval: Duration,
}

impl IntervalTrigger {
    pub(crate) const fn new(interval: Duration) -> Self {
        Self { interval }
    }
}

impl TriggerSpec for IntervalTrigger {
    fn next_deadline(&self, now: time::OffsetDateTime) -> time::OffsetDateTime {
        now + time::Duration::try_from(self.interval).unwrap_or(time::Duration::ZERO)
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct DailyLocalTimeTrigger {
    time: time::Time,
}

impl DailyLocalTimeTrigger {
    pub(crate) const fn new(time: time::Time) -> Self {
        Self { time }
    }

    pub(crate) fn parse(raw: &str) -> anyhow::Result<Self> {
        Ok(Self::new(parse_local_time(raw)?))
    }

    pub(crate) fn is_due(self, now: time::OffsetDateTime) -> bool {
        now.time() >= self.time
    }

    pub(crate) fn today_deadline(self, now: time::OffsetDateTime) -> time::OffsetDateTime {
        now.date().with_time(self.time).assume_offset(now.offset())
    }

    pub(crate) fn tomorrow_deadline(self, now: time::OffsetDateTime) -> time::OffsetDateTime {
        let tomorrow = now.date().next_day().unwrap_or(now.date());
        tomorrow.with_time(self.time).assume_offset(now.offset())
    }
}

impl TriggerSpec for DailyLocalTimeTrigger {
    fn next_deadline(&self, now: time::OffsetDateTime) -> time::OffsetDateTime {
        if self.is_due(now) {
            self.tomorrow_deadline(now)
        } else {
            self.today_deadline(now)
        }
    }
}

pub(crate) fn local_now() -> time::OffsetDateTime {
    let offset = time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC);
    time::OffsetDateTime::now_utc().to_offset(offset)
}

pub(crate) fn parse_local_time(raw: &str) -> anyhow::Result<time::Time> {
    let fmt = time::format_description::parse(LOCAL_TIME_FORMAT)?;
    Ok(time::Time::parse(raw.trim(), &fmt)?)
}

pub(crate) async fn wait_for_change(notify: &mut watch::Receiver<u64>) -> WaitOutcome {
    match notify.changed().await {
        Ok(_) => WaitOutcome::Reconfigured,
        Err(_) => WaitOutcome::Closed,
    }
}

pub(crate) async fn wait_until(
    deadline: time::OffsetDateTime,
    notify: &mut watch::Receiver<u64>,
) -> WaitOutcome {
    let remaining = deadline - local_now();
    if remaining <= time::Duration::ZERO {
        return WaitOutcome::Fired;
    }
    let delay = Duration::try_from(remaining).unwrap_or(Duration::ZERO);
    tokio::select! {
        _ = tokio::time::sleep(delay) => WaitOutcome::Fired,
        changed = notify.changed() => {
            if changed.is_err() {
                WaitOutcome::Closed
            } else {
                WaitOutcome::Reconfigured
            }
        }
    }
}

pub(crate) async fn wait_for_trigger<T: TriggerSpec>(
    trigger: &T,
    notify: &mut watch::Receiver<u64>,
) -> WaitOutcome {
    wait_until(trigger.next_deadline(local_now()), notify).await
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{DailyLocalTimeTrigger, IntervalTrigger, TriggerSpec};

    fn sample_now(hour: u8, minute: u8, second: u8) -> time::OffsetDateTime {
        let date = time::Date::from_calendar_date(2026, time::Month::March, 28).unwrap();
        let time = time::Time::from_hms(hour, minute, second).unwrap();
        date.with_time(time)
            .assume_offset(time::UtcOffset::from_hms(8, 0, 0).unwrap())
    }

    #[test]
    fn interval_trigger_returns_relative_deadline() {
        let now = sample_now(12, 0, 0);
        let trigger = IntervalTrigger::new(Duration::from_secs(90));
        assert_eq!(
            trigger.next_deadline(now),
            now + time::Duration::seconds(90)
        );
    }

    #[test]
    fn daily_trigger_uses_today_when_not_due() {
        let now = sample_now(0, 4, 0);
        let trigger = DailyLocalTimeTrigger::new(time::Time::from_hms(0, 5, 0).unwrap());
        assert_eq!(trigger.next_deadline(now), sample_now(0, 5, 0));
    }

    #[test]
    fn daily_trigger_uses_tomorrow_when_due() {
        let now = sample_now(0, 5, 0);
        let trigger = DailyLocalTimeTrigger::new(time::Time::from_hms(0, 5, 0).unwrap());
        let tomorrow = time::Date::from_calendar_date(2026, time::Month::March, 29).unwrap();
        let expected = tomorrow
            .with_time(time::Time::from_hms(0, 5, 0).unwrap())
            .assume_offset(time::UtcOffset::from_hms(8, 0, 0).unwrap());
        assert_eq!(trigger.next_deadline(now), expected);
    }
}
