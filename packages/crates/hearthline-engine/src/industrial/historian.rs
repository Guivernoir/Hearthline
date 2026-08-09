use heapless::Vec as FixedList;

#[derive(Clone, Debug)]
struct HistorianEntry<T> {
    value: T,
    replicated: bool,
}

#[derive(Clone, Debug)]
pub struct HistorianBuffer<T, const CAPACITY: usize> {
    records: FixedList<HistorianEntry<T>, CAPACITY>,
    dropped_unreplicated: u64,
}

impl<T, const CAPACITY: usize> HistorianBuffer<T, CAPACITY> {
    pub const fn new() -> Self {
        assert!(CAPACITY > 0, "historian capacity must be positive");
        Self {
            records: FixedList::new(),
            dropped_unreplicated: 0,
        }
    }

    pub fn push(&mut self, value: T, replicated: bool) {
        if self.records.is_full() {
            let removed = self.records.remove(0);
            if !removed.replicated {
                self.dropped_unreplicated = self.dropped_unreplicated.saturating_add(1);
            }
        }
        self.records
            .push(HistorianEntry { value, replicated })
            .unwrap_or_else(|_| unreachable!("historian capacity checked before insertion"));
    }

    pub fn mark_replicated(&mut self, index: usize) -> bool {
        let Some(record) = self.records.get_mut(index) else {
            return false;
        };
        record.replicated = true;
        true
    }

    pub fn oldest_pending(&self) -> Option<(usize, &T)> {
        self.records
            .iter()
            .enumerate()
            .find(|(_, record)| !record.replicated)
            .map(|(index, record)| (index, &record.value))
    }

    pub fn latest(&self) -> Option<&T> {
        self.records.last().map(|record| &record.value)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&T, bool)> {
        self.records
            .iter()
            .map(|record| (&record.value, record.replicated))
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub const fn capacity(&self) -> usize {
        CAPACITY
    }

    pub fn pending_count(&self) -> usize {
        self.records
            .iter()
            .filter(|record| !record.replicated)
            .count()
    }

    pub const fn dropped_unreplicated(&self) -> u64 {
        self.dropped_unreplicated
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

impl<T, const CAPACITY: usize> Default for HistorianBuffer<T, CAPACITY> {
    fn default() -> Self {
        Self::new()
    }
}
