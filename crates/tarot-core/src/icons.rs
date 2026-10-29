//! Session-only, bounded application icon cache. Rasters never enter preferences.
use std::{
    collections::{BTreeMap, VecDeque},
    sync::Arc,
};

pub const MAX_PENDING: usize = 32;
pub const MAX_ENTRIES: usize = 256;
pub const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub struct Raster {
    size: [usize; 2],
    rgba: Vec<u8>,
}
impl Raster {
    /// Straight alpha, sRGB. Reject malformed worker output before texture upload.
    pub fn new(size: [usize; 2], rgba: Vec<u8>) -> Option<Self> {
        (size.iter().all(|s| (1..=256).contains(s)) && rgba.len() == size[0] * size[1] * 4)
            .then_some(Self { size, rgba })
    }
    pub fn size(&self) -> [usize; 2] {
        self.size
    }
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }
}

#[derive(Clone, Debug)]
enum Entry {
    Pending,
    Complete(Option<Arc<Raster>>),
}

#[derive(Clone, Debug, Default)]
pub struct State {
    entries: BTreeMap<String, Entry>,
    completed: VecDeque<String>,
    bytes: usize,
    pending: usize,
}
#[derive(Clone, Copy, Debug, serde::Serialize)]
pub struct Stats {
    pub ready: usize,
    pub missing: usize,
    pub pending: usize,
    pub raster_bytes: usize,
}
impl State {
    pub fn stats(&self) -> Stats {
        let ready = self
            .entries
            .values()
            .filter(|entry| matches!(entry, Entry::Complete(Some(_))))
            .count();
        Stats {
            ready,
            missing: self.entries.len() - ready - self.pending,
            pending: self.pending,
            raster_bytes: self.bytes,
        }
    }
    pub fn image(&self, id: &str) -> Option<&Arc<Raster>> {
        match self.entries.get(id) {
            Some(Entry::Complete(image)) => image.as_ref(),
            _ => None,
        }
    }
    pub fn needs_load(&self, id: &str) -> bool {
        !self.entries.contains_key(id) && self.pending < MAX_PENDING
    }
    pub fn begin(&mut self, id: &str) -> bool {
        if !self.needs_load(id) {
            return false;
        }
        while self.entries.len() >= MAX_ENTRIES {
            self.evict();
        }
        self.entries.insert(id.to_owned(), Entry::Pending);
        self.pending += 1;
        true
    }
    /// Only the matching outstanding request may install a result.
    pub fn complete(&mut self, id: String, image: Option<Raster>) {
        if !matches!(self.entries.get(&id), Some(Entry::Pending)) {
            return;
        }
        self.pending -= 1;
        let image = image.map(Arc::new);
        let bytes = image.as_ref().map_or(0, |r| r.rgba.len());
        while self.bytes + bytes > MAX_BYTES {
            self.evict();
        }
        self.bytes += bytes;
        self.entries.insert(id.clone(), Entry::Complete(image));
        self.completed.push_back(id);
    }
    fn evict(&mut self) {
        if let Some(id) = self.completed.pop_front()
            && let Some(Entry::Complete(image)) = self.entries.remove(&id)
        {
            self.bytes -= image.map_or(0, |r| r.rgba.len());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_requests_negative_cache_and_eviction() {
        let mut s = State::default();
        for n in 0..MAX_PENDING {
            assert!(s.begin(&n.to_string()));
        }
        assert!(!s.begin("overflow"));
        assert!(!s.begin("0"));
        for n in 0..MAX_PENDING {
            s.complete(n.to_string(), None);
        }
        assert!(!s.needs_load("0"));
        for n in MAX_PENDING..MAX_ENTRIES + 1 {
            assert!(s.begin(&n.to_string()));
            s.complete(n.to_string(), None);
        }
        assert!(s.needs_load("0"));
        assert_eq!(s.entries.len(), MAX_ENTRIES);
        for n in 0..100 {
            let id = format!("r{n}");
            assert!(s.begin(&id));
            s.complete(id, Raster::new([256, 256], vec![255; 256 * 256 * 4]));
            assert!(s.bytes <= MAX_BYTES);
        }
        assert!(s.image("r0").is_none());
        assert!(s.image("r99").is_some());
        s.complete("unsolicited".into(), Raster::new([1, 1], vec![0; 4]));
        assert!(s.image("unsolicited").is_none());
        assert!(Raster::new([257, 1], vec![0; 1028]).is_none());
        assert!(Raster::new([1, 1], vec![0; 3]).is_none());
    }
}
