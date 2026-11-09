//! Picture to chart: decode, aspect-correct area-averaged resample, CIELAB palette mapping (fixed or k-means),
//! lonely-stitch cleanup. Deterministic: the same input always gives the same chart.

use image::ImageReader;

use crate::chart::{Chart, ChartError, Construction, Craft, MAX_PALETTE, PaletteColor, Technique};
use crate::color::{Lab, delta_e_sq, generic_name, lab_to_srgb, linear_to_lab, srgb_to_lab, srgb_to_linear};
use crate::gauge::{Gauge, MAX_SIDE};

/// Largest picture side we decode (pixels).
pub const MAX_PICTURE_SIDE: u32 = 20_000;
const MAX_DECODE_BYTES: u64 = 1 << 30;
const KMEANS_ITERATIONS: usize = 20;
const MAX_CLEANUP: u8 = 3;

/// Why a picture could not be turned into a chart.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PictureError {
    #[error("the picture is empty or larger than {MAX_PICTURE_SIDE} pixels on a side")]
    BadSize,
    #[error("could not read the picture: {0}")]
    Decode(String),
    #[error("chart size {0} x {1} is not between 1 and {MAX_SIDE}")]
    BadGrid(u32, u32),
    #[error("the palette must have between 1 and {MAX_PALETTE} colours")]
    BadPalette,
    #[error("chart: {0}")]
    Chart(#[from] ChartError),
}

/// An RGB picture (alpha is flattened onto white when decoding).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub rgb: Vec<[u8; 3]>,
}

impl Picture {
    /// A validated picture.
    pub fn new(width: u32, height: u32, rgb: Vec<[u8; 3]>) -> Result<Self, PictureError> {
        let ok = (1..=MAX_PICTURE_SIDE).contains(&width) && (1..=MAX_PICTURE_SIDE).contains(&height);
        if !ok || rgb.len() != (width as usize) * (height as usize) {
            return Err(PictureError::BadSize);
        }
        Ok(Self { width, height, rgb })
    }

    /// Decodes PNG, JPEG or WebP bytes. Untrusted input: sizes are capped and every failure is an error.
    pub fn decode(bytes: &[u8]) -> Result<Self, PictureError> {
        let mut reader = ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().map_err(|e| PictureError::Decode(e.to_string()))?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(MAX_PICTURE_SIDE);
        limits.max_image_height = Some(MAX_PICTURE_SIDE);
        limits.max_alloc = Some(MAX_DECODE_BYTES);
        reader.limits(limits);
        let img = reader.decode().map_err(|e| PictureError::Decode(e.to_string()))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        let rgb = rgba
            .pixels()
            .map(|p| {
                let a = u32::from(p.0[3]);
                let blend = |c: u8| ((u32::from(c) * a + 255 * (255 - a) + 127) / 255) as u8;
                [blend(p.0[0]), blend(p.0[1]), blend(p.0[2])]
            })
            .collect();
        Self::new(width, height, rgb)
    }
}

/// Where the chart's colours come from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaletteSpec {
    /// Map each stitch to the nearest of these colours.
    Fixed(Vec<PaletteColor>),
    /// Pick this many colours from the picture (k-means in CIELAB).
    Auto(usize),
}

/// What to make from a picture.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PictureOptions {
    /// Chart size in stitches and rows.
    pub width: u32,
    pub height: u32,
    pub palette: PaletteSpec,
    /// Lonely-stitch cleanup strength, 0 (off) to 3 (three passes).
    pub cleanup: u8,
}

/// Rows for a chart `stitches` wide so the finished piece keeps the picture's shape, given the gauge
/// (stitches are not square, so rows = stitches x (rows per inch / stitches per inch) x picture height / width).
pub fn rows_for_width(gauge: &Gauge, picture_width: u32, picture_height: u32, stitches: u32) -> Result<u32, PictureError> {
    gauge.validate().map_err(ChartError::from)?;
    let rows = f64::from(stitches) * gauge.stitch_aspect() * f64::from(picture_height) / f64::from(picture_width.max(1));
    let rows = rows.round().max(1.0);
    if !(1..=MAX_SIDE).contains(&stitches) || rows > f64::from(MAX_SIDE) {
        return Err(PictureError::BadGrid(stitches, if rows.is_finite() { rows as u32 } else { u32::MAX }));
    }
    Ok(rows as u32)
}

/// Area-averages the picture onto a `grid_w` x `grid_h` grid. Returns linear-light RGB per cell, top row first.
pub fn resample(pic: &Picture, grid_w: u32, grid_h: u32) -> Result<Vec<[f64; 3]>, PictureError> {
    if !(1..=MAX_SIDE).contains(&grid_w) || !(1..=MAX_SIDE).contains(&grid_h) {
        return Err(PictureError::BadGrid(grid_w, grid_h));
    }
    let lut: Vec<f32> = (0..=255u8).map(|v| srgb_to_linear(v) as f32).collect();
    let cols = spans(pic.width, grid_w);
    let rows = spans_by_source(pic.height, grid_h);
    let (gw, gh) = (grid_w as usize, grid_h as usize);
    let mut out = vec![[0f32; 3]; gw * gh];
    let mut row_acc = vec![[0f32; 3]; gw];
    for (sy, row_spans) in rows.iter().enumerate() {
        row_acc.iter_mut().for_each(|a| *a = [0.0; 3]);
        let Some(src) = pic.rgb.get(sy * pic.width as usize..(sy + 1) * pic.width as usize) else { continue };
        for (dx, col_spans) in cols.iter().enumerate() {
            let mut acc = [0f32; 3];
            for &(sx, w) in col_spans {
                if let Some(px) = src.get(sx) {
                    for c in 0..3 {
                        acc[c] += w * lut[usize::from(px[c])];
                    }
                }
            }
            row_acc[dx] = acc;
        }
        for &(dy, wy) in row_spans {
            let base = dy * gw;
            for dx in 0..gw {
                for c in 0..3 {
                    out[base + dx][c] += wy * row_acc[dx][c];
                }
            }
        }
    }
    Ok(out.into_iter().map(|p| [f64::from(p[0]), f64::from(p[1]), f64::from(p[2])]).collect())
}

/// Area-averaging weights: for each of `n_dst` destination cells, the (source index, weight) pairs; weights sum to 1.
fn spans(n_src: u32, n_dst: u32) -> Vec<Vec<(usize, f32)>> {
    let (s, n) = (f64::from(n_src), f64::from(n_dst));
    (0..n_dst)
        .map(|d| {
            let (start, end) = (f64::from(d) * s / n, f64::from(d + 1) * s / n);
            let first = start.floor() as usize;
            let last = (end.ceil() as usize).min(n_src as usize);
            (first..last)
                .map(|p| {
                    let w = ((p as f64 + 1.0).min(end) - (p as f64).max(start)).max(0.0) / (end - start);
                    (p, w as f32)
                })
                .collect()
        })
        .collect()
}

/// The same weights regrouped by source index: for each source cell, the (destination index, weight) pairs.
fn spans_by_source(n_src: u32, n_dst: u32) -> Vec<Vec<(usize, f32)>> {
    let mut by_src = vec![Vec::new(); n_src as usize];
    for (d, list) in spans(n_src, n_dst).into_iter().enumerate() {
        for (p, w) in list {
            if let Some(v) = by_src.get_mut(p) {
                v.push((d, w));
            }
        }
    }
    by_src
}

/// Maps resampled cells to palette indices and builds the chart cells (row 0 at the bottom) and palette.
pub fn picture_to_cells(pic: &Picture, opts: &PictureOptions) -> Result<(Vec<u8>, Vec<PaletteColor>), PictureError> {
    let cells = resample(pic, opts.width, opts.height)?;
    let labs: Vec<Lab> = cells.iter().map(|&c| linear_to_lab(c)).collect();
    let palette = match &opts.palette {
        PaletteSpec::Fixed(p) => {
            if p.is_empty() || p.len() > MAX_PALETTE {
                return Err(PictureError::BadPalette);
            }
            p.clone()
        }
        PaletteSpec::Auto(n) => {
            if !(1..=MAX_PALETTE).contains(n) {
                return Err(PictureError::BadPalette);
            }
            auto_palette(&labs, *n)
        }
    };
    let palette_lab: Vec<Lab> = palette.iter().map(|c| srgb_to_lab(c.rgb)).collect();
    let mut idx: Vec<u8> = labs.iter().map(|&l| nearest(l, &palette_lab) as u8).collect();
    for _ in 0..opts.cleanup.min(MAX_CLEANUP) {
        idx = majority_pass(&idx, opts.width as usize, opts.height as usize);
    }
    // The chart's row 0 is the first row worked, at the bottom of the picture.
    let (w, h) = (opts.width as usize, opts.height as usize);
    let mut flipped = vec![0u8; idx.len()];
    for y in 0..h {
        flipped[(h - 1 - y) * w..(h - y) * w].copy_from_slice(&idx[y * w..(y + 1) * w]);
    }
    Ok((flipped, palette))
}

/// A whole chart from a picture.
pub fn chart_from_picture(
    pic: &Picture,
    opts: &PictureOptions,
    craft: Craft,
    technique: Technique,
    construction: Construction,
    gauge: Gauge,
) -> Result<Chart, PictureError> {
    let (cells, palette) = picture_to_cells(pic, opts)?;
    Ok(Chart::new(opts.width, opts.height, cells, palette, craft, technique, construction, gauge)?)
}

fn nearest(l: Lab, palette: &[Lab]) -> usize {
    let mut best = (0, f64::INFINITY);
    for (i, p) in palette.iter().enumerate() {
        let d = delta_e_sq(l, *p);
        if d < best.1 {
            best = (i, d);
        }
    }
    best.0
}

/// k-means in CIELAB with farthest-first seeding, so the result is deterministic. Palette sorted lightest first.
fn auto_palette(labs: &[Lab], k: usize) -> Vec<PaletteColor> {
    let n = labs.len().max(1) as f64;
    let mean = labs.iter().fold([0.0; 3], |a, l| [a[0] + l[0], a[1] + l[1], a[2] + l[2]]).map(|v| v / n);
    let mut centres: Vec<Lab> = Vec::with_capacity(k);
    centres.push(labs.iter().copied().min_by(|a, b| delta_e_sq(*a, mean).total_cmp(&delta_e_sq(*b, mean))).unwrap_or(mean));
    let mut min_d: Vec<f64> = labs.iter().map(|&l| delta_e_sq(l, centres[0])).collect();
    while centres.len() < k {
        let far = min_d.iter().enumerate().fold((0usize, -1.0f64), |b, (i, &d)| if d > b.1 { (i, d) } else { b }).0;
        let c = labs.get(far).copied().unwrap_or(mean);
        for (d, &l) in min_d.iter_mut().zip(labs) {
            *d = d.min(delta_e_sq(l, c));
        }
        centres.push(c);
    }
    let mut assign = vec![usize::MAX; labs.len()];
    for _ in 0..KMEANS_ITERATIONS {
        let mut changed = false;
        let mut sums = vec![[0.0f64; 3]; k];
        let mut counts = vec![0usize; k];
        for (a, &l) in assign.iter_mut().zip(labs) {
            let best = nearest(l, &centres);
            if *a != best {
                *a = best;
                changed = true;
            }
            for c in 0..3 {
                sums[best][c] += l[c];
            }
            counts[best] += 1;
        }
        for i in 0..k {
            if counts[i] > 0 {
                let m = counts[i] as f64;
                centres[i] = [sums[i][0] / m, sums[i][1] / m, sums[i][2] / m];
            }
        }
        if !changed {
            break;
        }
    }
    centres.sort_by(|a, b| b[0].total_cmp(&a[0]).then(a[1].total_cmp(&b[1])).then(a[2].total_cmp(&b[2])));
    let mut names: Vec<String> = Vec::new();
    centres
        .iter()
        .map(|&c| {
            let rgb = lab_to_srgb(c);
            let base = generic_name(rgb);
            let mut name = base.clone();
            let mut n = 2;
            while names.contains(&name) {
                name = format!("{base} {n}");
                n += 1;
            }
            names.push(name.clone());
            PaletteColor { name, rgb }
        })
        .collect()
}

/// One cleanup pass: a stitch with no same-colour neighbour above, below, left or right takes the most common colour
/// around it when at least four of its neighbours share that colour.
fn majority_pass(cells: &[u8], w: usize, h: usize) -> Vec<u8> {
    let mut out = cells.to_vec();
    for y in 0..h {
        for x in 0..w {
            let me = cells[y * w + x];
            let at = |dx: isize, dy: isize| -> Option<u8> {
                let (nx, ny) = (x as isize + dx, y as isize + dy);
                if nx < 0 || ny < 0 || nx >= w as isize || ny >= h as isize { None } else { cells.get(ny as usize * w + nx as usize).copied() }
            };
            if [(0, -1), (0, 1), (-1, 0), (1, 0)].iter().any(|&(dx, dy)| at(dx, dy) == Some(me)) {
                continue;
            }
            let mut counts = [0u8; 256];
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if (dx, dy) != (0, 0)
                        && let Some(c) = at(dx, dy)
                    {
                        counts[usize::from(c)] += 1;
                    }
                }
            }
            let (best, count) = counts.iter().enumerate().fold((0usize, 0u8), |b, (i, &c)| if c > b.1 { (i, c) } else { b });
            if count >= 4 {
                out[y * w + x] = best as u8;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::test_support::gauge;
    use crate::gauge::Unit;
    use proptest::prelude::*;
    use std::time::Instant;

    fn colour(name: &str, rgb: [u8; 3]) -> PaletteColor {
        PaletteColor { name: name.into(), rgb }
    }

    fn red_cream() -> Vec<PaletteColor> {
        vec![colour("cream", [250, 244, 230]), colour("red", [200, 30, 40])]
    }

    fn encode_png(pic: &Picture) -> Vec<u8> {
        let mut raw = Vec::new();
        for p in &pic.rgb {
            raw.extend_from_slice(p);
        }
        let img = image::RgbImage::from_raw(pic.width, pic.height, raw).unwrap();
        let mut out = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    /// A deterministic "photo": smooth colour fields plus pseudo-random noise, so it has hundreds of distinct colours.
    fn photo(w: u32, h: u32) -> Picture {
        let mut seed = 12345u32;
        let mut rgb = Vec::new();
        for y in 0..h {
            for x in 0..w {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                let n = (seed >> 24) as i32 % 40 - 20;
                let v = |base: u32| (base as i32 + n).clamp(0, 255) as u8;
                rgb.push([v(x * 255 / w), v(y * 255 / h), v(((x + y) * 255 / (w + h)) ^ 0x55)]);
            }
        }
        Picture::new(w, h, rgb).unwrap()
    }

    fn two_colour_20_by_10() -> Picture {
        // columns alternate in blocks of 2; the top 5 rows are inverted
        let mut rgb = Vec::new();
        for y in 0..10 {
            for x in 0..20 {
                let red = (x / 2 % 2 == 0) ^ (y < 5);
                rgb.push(if red { [200, 30, 40] } else { [250, 244, 230] });
            }
        }
        Picture::new(20, 10, rgb).unwrap()
    }

    fn opts(w: u32, h: u32, palette: PaletteSpec, cleanup: u8) -> PictureOptions {
        PictureOptions { width: w, height: h, palette, cleanup }
    }

    mod golden {
        use super::*;

        #[test]
        fn a_20_by_10_two_colour_png_becomes_exactly_20_by_10_with_the_expected_cells() {
            let png = encode_png(&two_colour_20_by_10());
            let pic = Picture::decode(&png).unwrap();
            let rows = rows_for_width(&Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap(), pic.width, pic.height, 20).unwrap();
            assert_eq!(rows, 10);
            let (cells, palette) = picture_to_cells(&pic, &opts(20, rows, PaletteSpec::Fixed(red_cream()), 0)).unwrap();
            assert_eq!((cells.len(), palette.len()), (200, 2));
            for y in 0..10usize {
                for x in 0..20usize {
                    let src_y = 9 - y; // chart row 0 is the bottom of the picture
                    let red = (x / 2 % 2 == 0) ^ (src_y < 5);
                    assert_eq!(cells[y * 20 + x], u8::from(red), "x={x} y={y}");
                }
            }
        }
    }

    mod sizing {
        use super::*;

        #[test]
        fn rows_follow_the_gauge_so_the_piece_is_not_stretched() {
            let square = Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap();
            assert_eq!(rows_for_width(&square, 200, 100, 40), Ok(20));
            // 20 sts and 28 rows per 4 in: stitches are 1.4x shorter, so a square picture needs 1.4x the rows
            assert_eq!(rows_for_width(&gauge(), 100, 100, 40), Ok(56));
            assert!(matches!(rows_for_width(&gauge(), 100, 100, 0), Err(PictureError::BadGrid(..))));
            assert!(matches!(rows_for_width(&gauge(), 1, 1_000_000, 100), Err(PictureError::BadGrid(..))));
        }

        #[test]
        fn area_averaging_gives_the_mean_colour_of_a_block() {
            // a 2x2 picture, one white and three black pixels, averaged to one cell: 1/4 of the light
            let pic = Picture::new(2, 2, vec![[255, 255, 255], [0, 0, 0], [0, 0, 0], [0, 0, 0]]).unwrap();
            let cell = resample(&pic, 1, 1).unwrap()[0];
            for c in cell {
                assert!((c - 0.25).abs() < 1e-6, "{cell:?}");
            }
        }

        #[test]
        fn upscaling_repeats_pixels_and_every_cell_is_covered() {
            let pic = Picture::new(1, 1, vec![[255, 0, 0]]).unwrap();
            let cells = resample(&pic, 3, 2).unwrap();
            assert_eq!(cells.len(), 6);
            assert!(cells.iter().all(|c| (c[0] - 1.0).abs() < 1e-6 && c[1].abs() < 1e-6));
        }
    }

    mod palette {
        use super::*;

        #[test]
        fn a_200_colour_photo_reduces_to_exactly_the_chosen_number_of_colours() {
            let pic = photo(200, 200);
            for n in [2usize, 3, 5, 8] {
                let (cells, palette) = picture_to_cells(&pic, &opts(60, 60, PaletteSpec::Auto(n), 0)).unwrap();
                assert_eq!(palette.len(), n);
                let used: std::collections::BTreeSet<u8> = cells.iter().copied().collect();
                assert_eq!(used.len(), n, "n={n}");
            }
        }

        #[test]
        fn auto_palette_is_lightest_first_with_unique_generic_names() {
            let (_, palette) = picture_to_cells(&photo(100, 100), &opts(30, 30, PaletteSpec::Auto(6), 0)).unwrap();
            let lightness: Vec<f64> = palette.iter().map(|c| srgb_to_lab(c.rgb)[0]).collect();
            assert!(lightness.windows(2).all(|w| w[0] >= w[1]), "{lightness:?}");
            let names: std::collections::BTreeSet<_> = palette.iter().map(|c| c.name.clone()).collect();
            assert_eq!(names.len(), 6);
        }

        #[test]
        fn fixed_palettes_pick_the_nearest_colour_in_lab() {
            let pic = Picture::new(2, 1, vec![[230, 60, 60], [240, 235, 220]]).unwrap();
            let (cells, _) = picture_to_cells(&pic, &opts(2, 1, PaletteSpec::Fixed(red_cream()), 0)).unwrap();
            assert_eq!(cells, [1, 0]);
        }

        #[test]
        fn bad_palettes_are_errors() {
            let pic = photo(10, 10);
            assert_eq!(picture_to_cells(&pic, &opts(5, 5, PaletteSpec::Fixed(vec![]), 0)).unwrap_err(), PictureError::BadPalette);
            assert_eq!(picture_to_cells(&pic, &opts(5, 5, PaletteSpec::Auto(0), 0)).unwrap_err(), PictureError::BadPalette);
            assert_eq!(picture_to_cells(&pic, &opts(5, 5, PaletteSpec::Auto(65), 0)).unwrap_err(), PictureError::BadPalette);
        }
    }

    mod cleanup {
        use super::*;

        #[test]
        fn a_lonely_stitch_is_removed_only_when_cleanup_is_on() {
            let mut rgb = vec![[250, 244, 230]; 25];
            rgb[12] = [200, 30, 40];
            let pic = Picture::new(5, 5, rgb).unwrap();
            let (off, _) = picture_to_cells(&pic, &opts(5, 5, PaletteSpec::Fixed(red_cream()), 0)).unwrap();
            assert_eq!(off.iter().filter(|&&c| c == 1).count(), 1);
            let (on, _) = picture_to_cells(&pic, &opts(5, 5, PaletteSpec::Fixed(red_cream()), 1)).unwrap();
            assert_eq!(on.iter().filter(|&&c| c == 1).count(), 0);
        }

        #[test]
        fn pairs_and_edges_of_real_shapes_survive() {
            let mut rgb = vec![[250, 244, 230]; 25];
            rgb[11] = [200, 30, 40];
            rgb[12] = [200, 30, 40];
            let pic = Picture::new(5, 5, rgb).unwrap();
            let (on, _) = picture_to_cells(&pic, &opts(5, 5, PaletteSpec::Fixed(red_cream()), 3)).unwrap();
            assert_eq!(on.iter().filter(|&&c| c == 1).count(), 2);
        }
    }

    mod determinism_and_speed {
        use super::*;

        #[test]
        fn the_same_input_gives_byte_identical_output() {
            let pic = photo(120, 90);
            let make = || {
                let o = opts(40, 30, PaletteSpec::Auto(5), 1);
                let c = chart_from_picture(&pic, &o, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge()).unwrap();
                crate::file::to_json(&c).unwrap()
            };
            assert_eq!(make(), make());
        }

        #[test]
        fn a_12_megapixel_photo_to_200_by_200_is_fast() {
            let pic = Picture::new(4000, 3000, vec![[120, 80, 200]; 12_000_000]).unwrap();
            let start = Instant::now();
            let (cells, _) = picture_to_cells(&pic, &opts(200, 200, PaletteSpec::Auto(4), 1)).unwrap();
            let took = start.elapsed().as_secs_f64();
            assert_eq!(cells.len(), 40_000);
            // 2 s is the release-build target; unoptimised test builds get headroom.
            assert!(took < if cfg!(debug_assertions) { 20.0 } else { 2.0 }, "took {took:.2}s");
        }
    }

    mod hostile_input {
        use super::*;

        #[test]
        fn garbage_and_truncated_files_are_errors() {
            assert!(Picture::decode(b"").is_err());
            assert!(Picture::decode(b"not a picture at all").is_err());
            let png = encode_png(&two_colour_20_by_10());
            assert!(Picture::decode(&png[..png.len() / 2]).is_err());
            assert!(Picture::decode(&png[..8]).is_err());
        }

        #[test]
        fn bad_grids_and_pictures_are_errors() {
            let pic = photo(4, 4);
            assert!(matches!(resample(&pic, 0, 5), Err(PictureError::BadGrid(..))));
            assert!(matches!(resample(&pic, 5, MAX_SIDE + 1), Err(PictureError::BadGrid(..))));
            assert_eq!(Picture::new(0, 1, vec![]).unwrap_err(), PictureError::BadSize);
            assert_eq!(Picture::new(2, 2, vec![[0; 3]; 3]).unwrap_err(), PictureError::BadSize);
        }

        proptest! {
            #[test]
            fn random_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..300)) {
                let _ = Picture::decode(&bytes);
            }

            #[test]
            fn mutated_pngs_never_panic(pos in 0usize..400, val in any::<u8>()) {
                let mut png = encode_png(&two_colour_20_by_10());
                let p = pos % png.len();
                png[p] = val;
                let _ = Picture::decode(&png);
            }
        }
    }
}
