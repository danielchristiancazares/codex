use super::*;
use std::cell::Cell;
use std::cell::RefCell;

#[test]
fn rapid_restart_keeps_only_the_new_commit_tick_generation_active() {
    let ticker = CommitAnimationTicker::default();
    let first_generation = ticker.start().expect("first generation should start");
    let first_generation_value = first_generation.value();
    assert!(ticker.start().is_none());

    let restarted_generation = RefCell::new(None);
    let first_generation_waits = Cell::new(0);
    let emitted_generations = RefCell::new(Vec::new());
    run_commit_animation_ticks(
        first_generation,
        || {
            first_generation_waits.set(first_generation_waits.get() + 1);
            ticker.stop();
            restarted_generation.replace(ticker.start());
        },
        |generation| emitted_generations.borrow_mut().push(generation),
    );

    assert_eq!(first_generation_waits.get(), 1);
    assert!(emitted_generations.borrow().is_empty());
    assert!(!ticker.accepts_tick(first_generation_value));

    let second_generation = restarted_generation
        .take()
        .expect("restart should create a new generation");
    let second_generation_value = second_generation.value();
    assert!(ticker.accepts_tick(second_generation_value));
    assert!(ticker.start().is_none());

    let second_generation_waits = Cell::new(0);
    run_commit_animation_ticks(
        second_generation,
        || {
            let waits = second_generation_waits.get() + 1;
            second_generation_waits.set(waits);
            if waits == 2 {
                ticker.stop();
            }
        },
        |generation| emitted_generations.borrow_mut().push(generation),
    );

    assert_eq!(second_generation_waits.get(), 2);
    assert_eq!(
        emitted_generations.into_inner(),
        vec![second_generation_value]
    );
    assert!(!ticker.accepts_tick(second_generation_value));
}
