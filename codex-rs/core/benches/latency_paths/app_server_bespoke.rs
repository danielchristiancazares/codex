use divan::Bencher;
use divan::counter::ItemsCount;
use std::sync::Arc;

#[divan::bench(sample_count = 100, sample_size = 1000)]
fn app_server_bespoke_dependency_handoff(bencher: Bencher) {
    let shared = std::array::from_fn::<_, 7, _>(|_| Arc::new(()));
    let fallback_model_provider = "benchmark-provider".to_string();

    bencher
        .counter(ItemsCount::new(/*count*/ 1usize))
        .bench_local(move || {
            divan::black_box((
                divan::black_box(&shared[0]),
                divan::black_box(&shared[1]),
                divan::black_box(&shared[2]),
                divan::black_box(&shared[3]),
                divan::black_box(&shared[6]),
                divan::black_box(fallback_model_provider.as_str()),
            ));
        });
}
