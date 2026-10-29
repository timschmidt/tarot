//! Application artwork is resolved and rasterized on one bounded worker.
mod decode;
mod theme;

use anyhow::{Context, Result};
use smithay::reexports::calloop::{LoopHandle, channel};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread::JoinHandle,
};
use tarot_core::icons::{MAX_PENDING, Raster};

pub struct Server {
    requests: Option<mpsc::SyncSender<(String, String)>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
impl Server {
    pub fn request(&self, id: String, icon: String) -> bool {
        self.requests
            .as_ref()
            .is_some_and(|s| s.try_send((id, icon)).is_ok())
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        self.requests.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

pub fn start(
    handle: LoopHandle<'static, crate::CalloopData>,
    state: &mut crate::Compositor,
) -> Result<()> {
    let (requests, receiver) = mpsc::sync_channel::<(String, String)>(MAX_PENDING);
    let (results, events) = channel::channel::<(String, Option<Raster>)>();
    handle
        .insert_source(events, |event, _, data| {
            if let channel::Event::Msg((id, image)) = event {
                data.state.shell.icons.complete(id, image);
                data.state.ui_dirty = true;
                if !data.state.protected() {
                    data.state.redraw = true;
                }
            }
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let pixels = (tarot_core::design::LAUNCHER_ICON * state.scale.clamp(1, 4) as f32).ceil() as u32;
    let stop = Arc::new(AtomicBool::new(false));
    let stopping = stop.clone();
    let worker = std::thread::Builder::new()
        .name("tarot-icons".into())
        .spawn(move || {
            let mut themes = theme::Resolver::from_environment();
            let decoder = decode::Decoder::new();
            while let Ok((id, icon)) = receiver.recv() {
                if stopping.load(Ordering::Acquire) {
                    break;
                }
                let image = themes
                    .lookup(&icon, state_size(pixels))
                    .and_then(|path| decoder.load(&path, pixels));
                if results.send((id, image)).is_err() {
                    break;
                }
            }
        })
        .context("starting application icon worker")?;
    state.icons = Some(Server {
        requests: Some(requests),
        stop,
        worker: Some(worker),
    });
    Ok(())
}

fn state_size(pixels: u32) -> (u32, u32) {
    // The artwork's logical target is fixed; retain the output scale in lookup.
    let size = tarot_core::design::LAUNCHER_ICON as u32;
    (size, (pixels / size).clamp(1, 4))
}

/// Follow theme symlinks, but reject FIFOs/devices before any read and cap growth.
fn read(path: &std::path::Path, limit: usize) -> Option<Vec<u8>> {
    use std::{fs::OpenOptions, io::Read, os::unix::fs::OpenOptionsExt};
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(path)
        .ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > limit as u64 {
        return None;
    }
    let mut data = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut data).ok()?;
    (data.len() <= limit).then_some(data)
}

#[cfg(test)]
mod tests {
    #[test]
    fn file_guard_accepts_theme_symlinks_and_rejects_fifo_and_oversize() {
        let temp = tempfile::tempdir().unwrap();
        let file = temp.path().join("icon.svg");
        std::fs::write(&file, b"fixture").unwrap();
        let link = temp.path().join("link.svg");
        std::os::unix::fs::symlink(&file, &link).unwrap();
        assert_eq!(super::read(&link, 7), Some(b"fixture".to_vec()));
        assert!(super::read(&link, 6).is_none());
        assert!(super::read(temp.path(), 7).is_none());
        let fifo = temp.path().join("pipe.svg");
        rustix::fs::mkfifoat(rustix::fs::CWD, &fifo, rustix::fs::Mode::RUSR).unwrap();
        assert!(super::read(&fifo, 7).is_none());
    }
}
