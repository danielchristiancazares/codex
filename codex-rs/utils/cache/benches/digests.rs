use codex_utils_cache::blake3_digest;
use codex_utils_cache::sha1_digest;
use divan::Bencher;

const INPUT_SIZES: [usize; 2] = [1024 * 1024, 5 * 1024 * 1024];

fn main() {
    divan::main();
}

#[divan::bench(args = INPUT_SIZES)]
fn sha1(bencher: Bencher, input_size: usize) {
    let bytes = vec![0xa5; input_size];
    bencher.bench_local(move || sha1_digest(&bytes));
}

#[divan::bench(args = INPUT_SIZES)]
fn blake3(bencher: Bencher, input_size: usize) {
    let bytes = vec![0xa5; input_size];
    bencher.bench_local(move || blake3_digest(&bytes));
}
