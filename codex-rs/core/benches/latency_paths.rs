use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use divan::Bencher;

#[path = "../src/audio_preparation.rs"]
#[allow(dead_code, unused_imports)]
mod audio_preparation;
#[path = "../src/util.rs"]
#[allow(dead_code, unused_imports)]
mod util;

const PCM_SAMPLE_COUNTS: [usize; 2] = [8_000, 5 * 1024 * 1024];

fn main() {
    divan::main();
}

#[divan::bench(args = PCM_SAMPLE_COUNTS)]
fn pcm_wav_prepare(bencher: Bencher, sample_count: usize) {
    let audio_url = pcm_wav_data_url(sample_count);
    bencher
        .with_inputs(move || audio_url.clone())
        .bench_local_values(|mut audio_url| {
            #[allow(clippy::expect_used)]
            audio_preparation::prepare_audio(&mut audio_url)
                .expect("benchmark fixture should prepare");
            audio_url
        });
}

#[divan::bench(args = PCM_SAMPLE_COUNTS)]
fn pcm_wav_estimate(bencher: Bencher, sample_count: usize) {
    let audio_url = pcm_wav_data_url(sample_count);
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let _ = audio_preparation::estimate_audio_token_count(&audio_url);
    bencher.bench_local(move || audio_preparation::estimate_audio_token_count(&audio_url));
}

#[divan::bench]
fn one_mebibyte_json_counted_len(bencher: Bencher) {
    let value = one_mebibyte_json_value();
    bencher.bench_local(move || util::serialized_json_bytes(&value));
}

fn pcm_wav_data_url(sample_count: usize) -> String {
    let sample_count = u32::try_from(sample_count).unwrap_or(u32::MAX);
    let padding = sample_count % 2;
    let riff_size = 36u32.saturating_add(sample_count).saturating_add(padding);
    let mut bytes = Vec::with_capacity(riff_size as usize + 8);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&riff_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&8_000u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&8u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&sample_count.to_le_bytes());
    bytes.resize(bytes.len() + sample_count as usize + padding as usize, 0);
    format!("data:audio/wav;base64,{}", BASE64_STANDARD.encode(bytes))
}

fn one_mebibyte_json_value() -> serde_json::Value {
    serde_json::json!({
        "call_id": "call-performance",
        "output": "x".repeat(1024 * 1024),
    })
}
