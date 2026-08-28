//! Lookup strategy retained with each append-only history turn.
//!
//! Replacements preserve IDs and positions; duplicate IDs resolve to their first
//! occurrence. Completed turns retain their index through late updates and rollback.

use super::ThreadItem;
use std::collections::HashMap;

pub(super) const TURN_ITEM_INDEX_THRESHOLD: usize = 32;

#[derive(Default)]
pub(super) enum TurnItemIndex {
    #[default]
    Linear,
    Indexed(HashMap<String, usize>),
}

impl TurnItemIndex {
    pub(super) fn push(&mut self, items: &mut Vec<ThreadItem>, item: ThreadItem) {
        if let Self::Indexed(positions) = self {
            positions
                .entry(item.id().to_string())
                .or_insert(items.len());
        }
        items.push(item);
    }

    pub(super) fn upsert<'a>(
        &mut self,
        items: &'a mut Vec<ThreadItem>,
        item: ThreadItem,
    ) -> &'a ThreadItem {
        if matches!(self, Self::Linear) && items.len() >= TURN_ITEM_INDEX_THRESHOLD {
            let mut positions = HashMap::with_capacity(items.len());
            for (index, existing) in items.iter().enumerate() {
                positions.entry(existing.id().to_string()).or_insert(index);
            }
            *self = Self::Indexed(positions);
        }

        let existing_index = match self {
            Self::Indexed(positions) => positions.get(item.id()).copied(),
            Self::Linear => items.iter().position(|existing| existing.id() == item.id()),
        };
        if let Some(index) = existing_index {
            items[index] = item;
            &items[index]
        } else {
            let index = items.len();
            self.push(items, item);
            &items[index]
        }
    }
}
