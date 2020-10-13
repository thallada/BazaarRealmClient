/// Thin wrapper around HashMap that automatically assigns new entries with an incrementing key (like a database)
use std::collections::HashMap;

pub struct Cache<T> {
    next_key: usize,
    cache: HashMap<usize, T>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Cache {
            next_key: 0,
            cache: HashMap::new(),
        }
    }
}

impl<T> Cache<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, value: T) -> usize {
        let new_key = self.next_key;
        self.cache.insert(new_key, value);
        self.next_key += 1;
        new_key
    }

    pub fn get(&self, key: &usize) -> Option<&T> {
        self.cache.get(key)
    }

    pub fn remove(&mut self, key: &usize) {
        self.cache.remove(key);
    }
}