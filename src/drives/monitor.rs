//! Background native inventory refresh. No GUI timers, shell commands, or
//! authentication requests: discovery only reads device metadata.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]
use super::Drive;
use std::{
    sync::mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
    time::Duration,
};
type Inventory = Result<Vec<Drive>, String>;

pub struct DriveMonitor {
    updates: Receiver<Inventory>,
    refresh: SyncSender<()>,
}
impl DriveMonitor {
    pub fn request_refresh(&self) -> bool {
        match self.refresh.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => true,
            Err(TrySendError::Disconnected(())) => false,
        }
    }
    pub fn take_update(&mut self) -> Option<Inventory> {
        self.updates.try_iter().last()
    }
}

pub fn watch() -> Option<DriveMonitor> {
    if !super::AVAILABLE {
        return None;
    }
    watch_with(Duration::from_secs(2), super::discover)
}

fn watch_with(
    interval: Duration,
    mut discover: impl FnMut() -> Inventory + Send + 'static,
) -> Option<DriveMonitor> {
    // Bounded mailboxes coalesce bursts and preserve the newest snapshot on the
    // next check if the UI was occupied by a native file dialog.
    let (updates, receiver) = mpsc::sync_channel(1);
    let (refresh, requests) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("drive-monitor".into())
        .spawn(move || {
            let mut previous = None;
            let mut force = true;
            loop {
                let current = discover();
                if force || previous.as_ref() != Some(&current) {
                    match updates.try_send(current.clone()) {
                        Ok(()) => {
                            previous = Some(current);
                        }
                        Err(TrySendError::Full(_)) => {} // Keep previous; retry the newest state next time.
                        Err(TrySendError::Disconnected(_)) => break,
                    }
                }
                match requests.recv_timeout(interval) {
                    Ok(()) => force = true,
                    Err(RecvTimeoutError::Timeout) => force = false,
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            }
        })
        .ok()?;
    // Dropping the monitor disconnects requests, wakes the worker, and releases
    // its context/connection resources instead of leaving a permanent thread.
    Some(DriveMonitor {
        updates: receiver,
        refresh,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    fn drive(name: &str) -> Drive {
        Drive {
            path: format!("/dev/{name}").into(),
            name: name.into(),
            name_is_message: false,
            size: 1024,
            kind: "USB drive".into(),
            location: "Not mounted".into(),
            mounted: false,
            encrypted: false,
            sector_size: 512,
            phone: false,
        }
    }
    #[test]
    fn monitor_reports_plug_unplug_and_manual_refresh_without_unchanged_updates() {
        let state = Arc::new(Mutex::new(Ok(vec![drive("one")])));
        let worker_state = Arc::clone(&state);
        let monitor = watch_with(Duration::from_millis(10), move || {
            worker_state.lock().unwrap().clone()
        })
        .unwrap();
        assert_eq!(
            monitor
                .updates
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        monitor
            .updates
            .recv_timeout(Duration::from_millis(40))
            .unwrap_err();
        *state.lock().unwrap() = Ok(vec![drive("one"), drive("two")]);
        assert_eq!(
            monitor
                .updates
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .len(),
            2
        );
        *state.lock().unwrap() = Ok(vec![drive("two")]);
        assert_eq!(
            monitor
                .updates
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()[0]
                .name,
            "two"
        );
        assert!(monitor.request_refresh());
        assert_eq!(
            monitor
                .updates
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .unwrap()
                .len(),
            1
        );
        *state.lock().unwrap() = Err("temporary discovery error".into());
        monitor
            .updates
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap_err();
        *state.lock().unwrap() = Ok(vec![drive("two")]);
        monitor
            .updates
            .recv_timeout(Duration::from_secs(1))
            .unwrap()
            .unwrap();
    }
}
