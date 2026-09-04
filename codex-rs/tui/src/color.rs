pub(crate) fn is_light(bg: (u8, u8, u8)) -> bool {
    let (r, g, b) = bg;
    let y = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
    y > 128.0
}

pub(crate) fn blend(fg: (u8, u8, u8), bg: (u8, u8, u8), alpha: f32) -> (u8, u8, u8) {
    let r = (fg.0 as f32 * alpha + bg.0 as f32 * (1.0 - alpha)) as u8;
    let g = (fg.1 as f32 * alpha + bg.1 as f32 * (1.0 - alpha)) as u8;
    let b = (fg.2 as f32 * alpha + bg.2 as f32 * (1.0 - alpha)) as u8;
    (r, g, b)
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn relative_luminance((r, g, b): (u8, u8, u8)) -> f32 {
    0.2126 * srgb_to_linear(r) + 0.7152 * srgb_to_linear(g) + 0.0722 * srgb_to_linear(b)
}

pub(crate) fn contrast_ratio(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    let a = relative_luminance(a);
    let b = relative_luminance(b);
    let (lighter, darker) = if a >= b { (a, b) } else { (b, a) };
    (lighter + 0.05) / (darker + 0.05)
}

/// Moves a foreground toward whichever neutral endpoint improves contrast most.
pub(crate) fn strengthen_contrast(
    foreground: (u8, u8, u8),
    background: (u8, u8, u8),
    amount: f32,
) -> (u8, u8, u8) {
    let amount = amount.clamp(0.0, 1.0);
    let candidates = [
        foreground,
        blend((0, 0, 0), foreground, amount),
        blend((255, 255, 255), foreground, amount),
    ];
    let mut best = foreground;
    let mut best_ratio = contrast_ratio(foreground, background);
    for candidate in candidates.into_iter().skip(1) {
        let candidate_ratio = contrast_ratio(candidate, background);
        if candidate_ratio > best_ratio {
            best = candidate;
            best_ratio = candidate_ratio;
        }
    }
    best
}

/// Returns the perceptual color distance between two RGB colors.
/// Uses the CIE76 formula (Euclidean distance in Lab space approximation).
pub(crate) fn perceptual_distance(a: (u8, u8, u8), b: (u8, u8, u8)) -> f32 {
    // Convert RGB to XYZ
    fn rgb_to_xyz(r: u8, g: u8, b: u8) -> (f32, f32, f32) {
        let r = srgb_to_linear(r);
        let g = srgb_to_linear(g);
        let b = srgb_to_linear(b);

        let x = r * 0.4124 + g * 0.3576 + b * 0.1805;
        let y = r * 0.2126 + g * 0.7152 + b * 0.0722;
        let z = r * 0.0193 + g * 0.1192 + b * 0.9505;
        (x, y, z)
    }

    // Convert XYZ to Lab
    fn xyz_to_lab(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
        // D65 reference white
        let xr = x / 0.95047;
        let yr = y / 1.00000;
        let zr = z / 1.08883;

        fn f(t: f32) -> f32 {
            if t > 0.008856 {
                t.powf(1.0 / 3.0)
            } else {
                7.787 * t + 16.0 / 116.0
            }
        }

        let fx = f(xr);
        let fy = f(yr);
        let fz = f(zr);

        let l = 116.0 * fy - 16.0;
        let a = 500.0 * (fx - fy);
        let b = 200.0 * (fy - fz);
        (l, a, b)
    }

    let (x1, y1, z1) = rgb_to_xyz(a.0, a.1, a.2);
    let (x2, y2, z2) = rgb_to_xyz(b.0, b.1, b.2);

    let (l1, a1, b1) = xyz_to_lab(x1, y1, z1);
    let (l2, a2, b2) = xyz_to_lab(x2, y2, z2);

    let dl = l1 - l2;
    let da = a1 - a2;
    let db = b1 - b2;

    (dl * dl + da * da + db * db).sqrt()
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn strengthen_contrast_chooses_the_safer_neutral_direction() {
        assert_eq!(
            [
                strengthen_contrast((180, 180, 180), (0, 0, 0), 0.25),
                strengthen_contrast((80, 80, 80), (255, 255, 255), 0.25),
                strengthen_contrast((255, 255, 255), (0, 0, 0), 0.25),
            ],
            [(198, 198, 198), (60, 60, 60), (255, 255, 255)]
        );
    }

    #[test]
    fn strengthen_contrast_never_reduces_the_original_ratio() {
        for (foreground, background) in [
            ((210, 210, 210), (16, 18, 20)),
            ((40, 45, 50), (245, 245, 240)),
            ((0, 175, 175), (0, 0, 0)),
            ((0, 95, 135), (255, 255, 255)),
        ] {
            let strengthened = strengthen_contrast(foreground, background, 0.35);
            assert!(
                contrast_ratio(strengthened, background) >= contrast_ratio(foreground, background)
            );
        }
    }
}
