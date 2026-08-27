//! Generation-based lifecycle for the periodic streaming commit ticker.

use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

#[derive(Debug, Default)]
pub(super) struct CommitAnimationTicker {
    generation: Arc<AtomicU64>,
}

#[derive(Clone, Debug)]
pub(super) struct CommitAnimationGeneration {
    current_generation: Arc<AtomicU64>,
    generation: u64,
}

impl CommitAnimationTicker {
    pub(super) fn start(&self) -> Option<CommitAnimationGeneration> {
        let mut current_generation = self.generation.load(Ordering::Acquire);
        loop {
            if current_generation % 2 == 1 {
                return None;
            }

            let next_generation = current_generation.wrapping_add(1);
            match self.generation.compare_exchange_weak(
                current_generation,
                next_generation,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(CommitAnimationGeneration {
                        current_generation: Arc::clone(&self.generation),
                        generation: next_generation,
                    });
                }
                Err(actual_generation) => current_generation = actual_generation,
            }
        }
    }

    pub(super) fn stop(&self) {
        let _ = self.generation.fetch_update(
            Ordering::AcqRel,
            Ordering::Acquire,
            |current_generation| {
                (current_generation % 2 == 1).then(|| current_generation.wrapping_add(1))
            },
        );
    }

    pub(super) fn accepts_tick(&self, generation: u64) -> bool {
        generation % 2 == 1 && self.generation.load(Ordering::Acquire) == generation
    }
}

impl CommitAnimationGeneration {
    fn is_current(&self) -> bool {
        self.current_generation.load(Ordering::Acquire) == self.generation
    }

    fn value(&self) -> u64 {
        self.generation
    }
}

pub(super) fn run_commit_animation_ticks(
    generation: CommitAnimationGeneration,
    mut wait_for_tick: impl FnMut(),
    mut emit_tick: impl FnMut(u64),
) {
    loop {
        wait_for_tick();
        if !generation.is_current() {
            break;
        }
        emit_tick(generation.value());
    }
}

#[cfg(test)]
#[path = "commit_animation_ticker_tests.rs"]
mod tests;
