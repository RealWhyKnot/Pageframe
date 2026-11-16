use std::collections::HashMap;

pub(crate) struct ImageCache {
    entries: HashMap<u64, (peniko::ImageData, u64)>,
    clock: u64,
    bytes: usize,
    budget: usize,
}

impl ImageCache {
    pub(crate) fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            clock: 0,
            bytes: 0,
            budget,
        }
    }

    pub(crate) fn get(&mut self, key: u64) -> Option<peniko::ImageData> {
        self.clock += 1;
        let (image, last_used) = self.entries.get_mut(&key)?;
        *last_used = self.clock;
        Some(image.clone())
    }

    pub(crate) fn insert(&mut self, key: u64, image: peniko::ImageData) {
        self.bytes += image.data.len();
        if let Some((old, _)) = self.entries.insert(key, (image, self.clock)) {
            self.bytes -= old.data.len();
        }
        if self.bytes <= self.budget {
            return;
        }
        let mut by_age: Vec<(u64, u64)> = self
            .entries
            .iter()
            .map(|(key, (_, last_used))| (*last_used, *key))
            .collect();
        by_age.sort_unstable();
        for (_, key) in by_age {
            if self.bytes <= self.budget / 2 {
                break;
            }
            if let Some((old, _)) = self.entries.remove(&key) {
                self.bytes -= old.data.len();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(bytes: usize) -> peniko::ImageData {
        peniko::ImageData {
            data: peniko::Blob::new(std::sync::Arc::new(vec![0_u8; bytes])),
            format: peniko::ImageFormat::Rgba8,
            alpha_type: peniko::ImageAlphaType::Alpha,
            width: 1,
            height: 1,
        }
    }

    #[test]
    fn evicts_the_least_recently_used_image_past_the_budget() {
        let mut cache = ImageCache::new(100);
        cache.insert(1, image(40));
        cache.insert(2, image(40));
        assert!(cache.get(1).is_some());
        cache.insert(3, image(40));
        assert!(cache.get(2).is_none());
        assert!(cache.get(3).is_some());
    }

    #[test]
    fn a_hit_returns_the_same_blob() {
        let mut cache = ImageCache::new(100);
        let stored = image(4);
        cache.insert(7, stored.clone());
        let found = cache.get(7).expect("just inserted");
        assert_eq!(found.data.id(), stored.data.id());
    }
}
