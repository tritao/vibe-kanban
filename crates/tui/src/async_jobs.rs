use std::collections::HashMap;

use uuid::Uuid;

pub(crate) fn next_generation(counter: &mut u64) -> u64 {
    *counter = counter.wrapping_add(1);
    *counter
}

pub(crate) fn next_generation_for(map: &mut HashMap<Uuid, u64>, key: Uuid) -> u64 {
    let next = map.get(&key).copied().unwrap_or(0).wrapping_add(1);
    map.insert(key, next);
    next
}

pub(crate) fn is_latest_for(map: &HashMap<Uuid, u64>, key: Uuid, generation: u64) -> bool {
    map.get(&key).copied().unwrap_or(0) == generation
}
