//! The global clock wakes once per minute; drawing remains demand driven.
use crate::{CalloopData, Compositor};
use smithay::reexports::calloop::{
    LoopHandle,
    timer::{TimeoutAction, Timer},
};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn local_fields() -> (String, Option<u8>, String) {
    let Ok(time) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return ("—:—".into(), None, String::new());
    };
    let Ok(seconds) = time.as_secs().try_into() else {
        return ("—:—".into(), None, String::new());
    };
    let mut local = std::mem::MaybeUninit::<libc::tm>::uninit();
    // SAFETY: localtime_r writes a complete tm into our owned output on success.
    if unsafe { libc::localtime_r(&seconds, local.as_mut_ptr()) }.is_null() {
        return ("—:—".into(), None, String::new());
    }
    // SAFETY: the successful call above initialized local.
    let local = unsafe { local.assume_init() };
    let day = u8::try_from(local.tm_mday)
        .ok()
        .filter(|d| (1..=31).contains(d));
    let weekdays = [
        "Sunday",
        "Monday",
        "Tuesday",
        "Wednesday",
        "Thursday",
        "Friday",
        "Saturday",
    ];
    let months = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let date = usize::try_from(local.tm_wday)
        .ok()
        .and_then(|i| weekdays.get(i))
        .zip(
            usize::try_from(local.tm_mon)
                .ok()
                .and_then(|i| months.get(i)),
        )
        .zip(day)
        .map_or_else(String::new, |((weekday, month), day)| {
            format!("{weekday}, {month} {day}")
        });
    (
        format!("{:02}:{:02}", local.tm_hour, local.tm_min),
        day,
        date,
    )
}
fn next_minute() -> Duration {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Duration::from_secs(60 - now.as_secs() % 60)
        .saturating_sub(Duration::from_nanos(u64::from(now.subsec_nanos())))
}
fn refresh(state: &mut Compositor) {
    let (label, day, date) = local_fields();
    if label != state.shell.clock_label
        || day != state.shell.calendar_day
        || date != state.shell.date_label
    {
        state.shell.clock_label = label;
        state.shell.calendar_day = day;
        state.shell.date_label = date;
        state.ui_dirty = true;
        if !state.protected() && !state.idle.output_off {
            state.redraw = true;
        }
    }
}
pub fn start(
    handle: LoopHandle<'static, CalloopData>,
    state: &mut Compositor,
) -> anyhow::Result<()> {
    refresh(state);
    handle
        .insert_source(Timer::from_duration(next_minute()), |_, _, data| {
            refresh(&mut data.state);
            TimeoutAction::ToDuration(next_minute())
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}
