use std::collections::BTreeSet;

use slotmap::{KeyData, SlotMap};

use crate::{BeatMap, ItemId, RealTime, Time};

slotmap::new_key_type! {
    pub struct TrackId;

    pub struct TrackItemId;
}

#[derive(Debug, Clone)]
pub struct Track {
    beat_map: BeatMap,
    items: SlotMap<TrackItemId, TrackItem>,
    item_starts: BTreeSet<(RealTime, TrackItemId)>,
    item_ends: BTreeSet<(RealTime, TrackItemId)>,
}

impl Track {
    pub fn new(beat_map: BeatMap) -> Track {
        Track {
            beat_map,
            items: SlotMap::default(),
            item_starts: BTreeSet::default(),
            item_ends: BTreeSet::default(),
        }
    }

    pub fn insert(&mut self, item_id: ItemId, position: Time, duration: Time) -> TrackItemId {
        let real_start = position.to_real(&self.beat_map);
        let real_end = real_start + duration.to_real(&self.beat_map);

        let item = TrackItem {
            inner: item_id,
            position,
            duration,
            real_start,
            real_end,
        };

        let id = self.items.insert(item);

        self.item_starts.insert((real_start, id));
        self.item_ends.insert((real_end, id));

        id
    }

    pub fn remove(&mut self, id: TrackItemId) {
        if let Some(item) = self.items.remove(id) {
            let real_time = item.position.to_real(&self.beat_map);
            self.item_starts.remove(&(real_time, id));
        }
    }

    pub fn get(&self, id: TrackItemId) -> Option<&TrackItem> {
        self.items.get(id)
    }

    pub fn get_mut(&mut self, id: TrackItemId) -> Option<&mut TrackItem> {
        self.items.get_mut(id)
    }

    pub fn iter(&self) -> impl Iterator<Item = &TrackItem> + '_ {
        self.item_starts.iter().map(|&(_, id)| &self.items[id])
    }

    pub fn range(
        &self,
        start: Option<Time>,
        end: Option<Time>,
    ) -> impl Iterator<Item = &TrackItem> + '_ {
        let min_id = TrackItemId(KeyData::from_ffi(u64::MIN));
        let start = start.map_or((RealTime::MIN, min_id), |t| {
            (t.to_real(&self.beat_map), min_id)
        });

        let max_id = TrackItemId(KeyData::from_ffi(u64::MAX));
        let end = end.map_or((RealTime::MAX, max_id), |t| {
            (t.to_real(&self.beat_map), min_id)
        });

        let mut starts = self.item_starts.range(start..=end).peekable();
        let mut ends = self.item_ends.range(start..=end).peekable();

        std::iter::from_fn(move || loop {
            let from_starts = match (starts.peek(), ends.peek()) {
                (Some((a, _)), Some((b, _))) if a <= b => true,
                (Some(_), Some(_)) => false,
                (Some(_), None) => true,
                (None, Some(_)) => false,
                _ => return None,
            };

            let (_, id) = if from_starts { &mut starts } else { &mut ends }.next()?;
            let item = &self.items[*id];

            if !from_starts && item.real_start >= start.0 {
                continue;
            }

            return Some(item);
        })
    }

    pub fn move_item(&mut self, id: TrackItemId, new_pos: Time) {
        let item = &mut self.items[id];

        let duration = item.real_duration();
        let old_start = item.real_start;
        let old_end = item.real_end;

        item.position = new_pos;
        item.real_start = new_pos.to_real(&self.beat_map);
        item.real_end = item.real_start + duration;

        let new_start = item.real_start;
        let new_end = item.real_end;

        self.item_starts.remove(&(old_start, id));
        self.item_starts.insert((new_start, id));

        self.item_ends.remove(&(old_end, id));
        self.item_ends.insert((new_end, id));
    }

    pub fn resize_item(&mut self, id: TrackItemId, new_duration: Time) {
        self.items[id].duration = new_duration;
    }
}

#[derive(Debug, Clone)]
pub struct TrackItem {
    inner: ItemId,
    position: Time,
    duration: Time,
    real_start: RealTime,
    real_end: RealTime,
}

impl TrackItem {
    pub fn inner_id(&self) -> ItemId {
        self.inner
    }

    pub fn position(&self) -> Time {
        self.position
    }

    pub fn duration(&self) -> Time {
        self.duration
    }

    pub fn real_start(&self) -> RealTime {
        self.real_start
    }

    pub fn real_end(&self) -> RealTime {
        self.real_end
    }

    pub fn real_duration(&self) -> RealTime {
        self.real_end - self.real_start
    }
}
