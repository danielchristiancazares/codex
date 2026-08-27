use std::io::Cursor;
use std::num::NonZeroUsize;
use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_utils_cache::BlockingLruCache;
use codex_utils_cache::blake3_digest;
use codex_utils_cache::sha1_digest;
use codex_utils_image::PromptImageMode;
use codex_utils_image::data_url_from_bytes;
use codex_utils_image::dimensions_from_base64;
use codex_utils_image::load_for_prompt_bytes;
use divan::Bencher;
use image::DynamicImage;
use image::ImageFormat;
use image::Rgb;
use image::RgbImage;
use image::Rgba;
use image::RgbaImage;

const CACHE_MISS_VARIANT_COUNT: usize = 48;
const DATA_URL_INPUT_BYTES: usize = 1024 * 1024;

const TINY_IMAGE: ImageSize = ImageSize {
    width: 1,
    height: 1,
};
const SMALL_SCREENSHOT: ImageSize = ImageSize {
    width: 1_536,
    height: 864,
};
const LARGE_SCREENSHOT: ImageSize = ImageSize {
    width: 2_560,
    height: 1_440,
};
const LARGE_PHOTO: ImageSize = ImageSize {
    width: 3_264,
    height: 2_448,
};

#[derive(Clone, Copy)]
struct ImageSize {
    width: u32,
    height: u32,
}

fn main() {
    divan::main();
}

#[divan::bench]
fn small_png_screenshot_fresh_attachment(bencher: Bencher) {
    bench_fresh_attachment(
        bencher,
        "small-screenshot.png",
        cache_miss_variants(screenshot_png(SMALL_SCREENSHOT)),
    );
}

#[divan::bench]
fn large_png_screenshot_fresh_attachment(bencher: Bencher) {
    bench_fresh_attachment(
        bencher,
        "large-screenshot.png",
        cache_miss_variants(screenshot_png(LARGE_SCREENSHOT)),
    );
}

#[divan::bench]
fn large_jpeg_photo_fresh_attachment(bencher: Bencher) {
    bench_fresh_attachment(
        bencher,
        "large-photo.jpg",
        cache_miss_variants(photo_jpeg(LARGE_PHOTO)),
    );
}

#[divan::bench]
fn small_png_screenshot_repeated_attachment(bencher: Bencher) {
    bench_repeated_attachment(
        bencher,
        "small-screenshot.png",
        screenshot_png(SMALL_SCREENSHOT),
    );
}

#[divan::bench]
fn small_png_screenshot_repeated_load(bencher: Bencher) {
    bench_repeated_load(
        bencher,
        "small-screenshot.png",
        screenshot_png(SMALL_SCREENSHOT),
    );
}

#[divan::bench]
fn large_jpeg_photo_repeated_load(bencher: Bencher) {
    bench_repeated_load(bencher, "large-photo.jpg", photo_jpeg(LARGE_PHOTO));
}

#[divan::bench]
fn tiny_png_repeated_load(bencher: Bencher) {
    bench_repeated_load(bencher, "tiny.png", screenshot_png(TINY_IMAGE));
}

#[divan::bench]
fn one_megabyte_data_url(bencher: Bencher) {
    let bytes = vec![0xa5; DATA_URL_INPUT_BYTES];
    bencher.bench_local(move || data_url_from_bytes("image/png", &bytes));
}

#[divan::bench]
fn large_png_screenshot_base64_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(screenshot_png(LARGE_SCREENSHOT));
    bencher.bench_local(move || dimensions_from_base64(&encoded));
}

#[divan::bench]
fn large_jpeg_photo_base64_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(photo_jpeg(LARGE_PHOTO));
    bencher.bench_local(move || dimensions_from_base64(&encoded));
}

#[divan::bench]
fn tiny_png_base64_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(screenshot_png(TINY_IMAGE));
    bencher.bench_local(move || dimensions_from_base64(&encoded));
}

#[divan::bench]
fn small_png_screenshot_base64_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(screenshot_png(SMALL_SCREENSHOT));
    bencher.bench_local(move || dimensions_from_base64(&encoded));
}

#[divan::bench]
fn large_png_screenshot_full_decode_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(screenshot_png(LARGE_SCREENSHOT));
    bencher.bench_local(move || {
        #[allow(clippy::expect_used)]
        let bytes = BASE64_STANDARD
            .decode(&encoded)
            .expect("benchmark fixture should decode");
        #[allow(clippy::expect_used)]
        let image = image::load_from_memory(&bytes).expect("benchmark fixture should load");
        (image.width(), image.height())
    });
}

#[divan::bench]
fn large_jpeg_photo_full_decode_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(photo_jpeg(LARGE_PHOTO));
    bencher.bench_local(move || {
        #[allow(clippy::expect_used)]
        let bytes = BASE64_STANDARD
            .decode(&encoded)
            .expect("benchmark fixture should decode");
        #[allow(clippy::expect_used)]
        let image = image::load_from_memory(&bytes).expect("benchmark fixture should load");
        (image.width(), image.height())
    });
}

#[divan::bench]
fn tiny_png_full_decode_dimensions(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(screenshot_png(TINY_IMAGE));
    bencher.bench_local(move || {
        #[allow(clippy::expect_used)]
        let bytes = BASE64_STANDARD
            .decode(&encoded)
            .expect("benchmark fixture should decode");
        #[allow(clippy::expect_used)]
        let image = image::load_from_memory(&bytes).expect("benchmark fixture should load");
        (image.width(), image.height())
    });
}

#[divan::bench]
fn large_png_screenshot_sha1_cache_key(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(screenshot_png(LARGE_SCREENSHOT));
    bencher.bench_local(move || sha1_digest(encoded.as_bytes()));
}

#[divan::bench]
fn large_jpeg_photo_sha1_cache_key(bencher: Bencher) {
    let encoded = BASE64_STANDARD.encode(photo_jpeg(LARGE_PHOTO));
    bencher.bench_local(move || sha1_digest(encoded.as_bytes()));
}

#[divan::bench]
fn tiny_png_sha1_cache_hit(bencher: Bencher) {
    bench_sha1_cache_hit(bencher, screenshot_png(TINY_IMAGE));
}

#[divan::bench]
fn small_png_screenshot_sha1_cache_hit(bencher: Bencher) {
    bench_sha1_cache_hit(bencher, screenshot_png(SMALL_SCREENSHOT));
}

#[divan::bench]
fn tiny_png_blake3_cache_hit(bencher: Bencher) {
    bench_blake3_cache_hit(bencher, screenshot_png(TINY_IMAGE));
}

#[divan::bench]
fn small_png_screenshot_blake3_cache_hit(bencher: Bencher) {
    bench_blake3_cache_hit(bencher, screenshot_png(SMALL_SCREENSHOT));
}

fn bench_sha1_cache_hit(bencher: Bencher, image: Vec<u8>) {
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let encoded = BASE64_STANDARD.encode(image);
    let cache = BlockingLruCache::new(NonZeroUsize::MIN);
    let key = sha1_digest(encoded.as_bytes());
    let _ = cache.get_or_insert_with(key, || (1u32, 1u32));

    bencher.bench_local(move || {
        cache.get_or_insert_with(sha1_digest(encoded.as_bytes()), || (1u32, 1u32))
    });
}

fn bench_blake3_cache_hit(bencher: Bencher, image: Vec<u8>) {
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let encoded = BASE64_STANDARD.encode(image);
    let cache = BlockingLruCache::new(NonZeroUsize::MIN);
    let key = blake3_digest(encoded.as_bytes());
    let _ = cache.get_or_insert_with(key, || (1u32, 1u32));

    bencher.bench_local(move || {
        cache.get_or_insert_with(blake3_digest(encoded.as_bytes()), || (1u32, 1u32))
    });
}

fn bench_fresh_attachment(bencher: Bencher, path: &'static str, images: Vec<Vec<u8>>) {
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let mut image_index = 0;

    bencher
        // Divan excludes `with_inputs` from the measured benchmark timing.
        .with_inputs(move || {
            let image = images[image_index].clone();
            image_index = (image_index + 1) % images.len();
            image
        })
        .bench_local_values(move |image| prepare_prompt_data_url(path, image));
}

fn bench_repeated_attachment(bencher: Bencher, path: &'static str, image: Vec<u8>) {
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let _ = prepare_prompt_data_url(path, image.clone());

    bencher
        // Divan excludes the per-iteration input clone from measured timing.
        .with_inputs(move || image.clone())
        .bench_local_values(move |image| prepare_prompt_data_url(path, image));
}

fn bench_repeated_load(bencher: Bencher, path: &'static str, image: Vec<u8>) {
    #[allow(clippy::expect_used)]
    let runtime = tokio::runtime::Runtime::new().expect("benchmark runtime should start");
    let _runtime_guard = runtime.enter();
    let _ = load_prompt_image(path, image.clone());

    bencher
        // Divan excludes the per-iteration input clone from measured timing.
        .with_inputs(move || image.clone())
        .bench_local_values(move |image| load_prompt_image(path, image));
}

fn prepare_prompt_data_url(path: &str, image: Vec<u8>) -> String {
    load_prompt_image(path, image).into_data_url()
}

fn load_prompt_image(path: &str, image: Vec<u8>) -> codex_utils_image::EncodedImage {
    #[allow(clippy::expect_used)]
    load_for_prompt_bytes(Path::new(path), image, PromptImageMode::ResizeToFit)
        .expect("benchmark fixture should load")
}

fn cache_miss_variants(image: Vec<u8>) -> Vec<Vec<u8>> {
    // The loader caches by content digest. Suffixes keep this workload on the miss path.
    (0..CACHE_MISS_VARIANT_COUNT)
        .map(|variant| {
            let mut image = image.clone();
            image.extend_from_slice(&variant.to_le_bytes());
            image
        })
        .collect()
}

/// Encodes a synthetic UI screenshot fixture for prompt image benchmarks.
fn screenshot_png(size: ImageSize) -> Vec<u8> {
    let image = RgbaImage::from_fn(size.width, size.height, |x, y| {
        let toolbar = y < 52;
        let sidebar = x < 240;
        let panel_border = x % 320 < 2 || y % 216 < 2;
        let text_row = x > 270 && y > 88 && x % 19 < 13 && y % 31 < 3;

        if toolbar {
            Rgba([33, 40, 52, 255])
        } else if sidebar {
            let selection = y / 68 % 5 == 2;
            if selection {
                Rgba([65, 106, 171, 255])
            } else {
                Rgba([44, 54, 67, 255])
            }
        } else if panel_border {
            Rgba([198, 205, 216, 255])
        } else if text_row {
            Rgba([72, 82, 96, 255])
        } else {
            let panel = ((x / 320) + (y / 216) * 3) % 4;
            match panel {
                0 => Rgba([246, 248, 252, 255]),
                1 => Rgba([234, 241, 250, 255]),
                2 => Rgba([240, 247, 236, 255]),
                _ => Rgba([250, 240, 235, 255]),
            }
        }
    });

    encode_fixture(DynamicImage::ImageRgba8(image), ImageFormat::Png)
}

/// Encodes a synthetic textured photo fixture for prompt image benchmarks.
fn photo_jpeg(size: ImageSize) -> Vec<u8> {
    let image = RgbImage::from_fn(size.width, size.height, |x, y| {
        let x_gradient = x * 255 / size.width;
        let y_gradient = y * 255 / size.height;
        let texture = ((x.wrapping_mul(17) ^ y.wrapping_mul(31) ^ (x / 7) ^ (y / 11)) & 0xff) as u8;

        Rgb([
            blend_channel(x_gradient, texture, 3),
            blend_channel((x_gradient + y_gradient) / 2, texture, 5),
            blend_channel(255 - y_gradient, texture, 4),
        ])
    });

    encode_fixture(DynamicImage::ImageRgb8(image), ImageFormat::Jpeg)
}

fn blend_channel(gradient: u32, texture: u8, divisor: u32) -> u8 {
    ((gradient + u32::from(texture) / divisor) % 256) as u8
}

fn encode_fixture(image: DynamicImage, format: ImageFormat) -> Vec<u8> {
    let mut encoded = Cursor::new(Vec::new());
    #[allow(clippy::expect_used)]
    image
        .write_to(&mut encoded, format)
        .expect("benchmark fixture should encode");
    encoded.into_inner()
}
