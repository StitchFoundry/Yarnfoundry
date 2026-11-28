//! Follow-along: which written row or round the maker is on, remembered per pattern (no I/O; the host stores the text).

use std::collections::BTreeMap;

use crate::chart::Chart;

/// Most patterns remembered; older entries beyond this are ignored when reading.
const MAX_ENTRIES: usize = 1000;

/// A stable identity for a pattern, from its size, kind and stitches (not its gauge or colour names).
pub fn pattern_key(chart: &Chart) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut feed = |b: u8| {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for b in chart.width.to_le_bytes().into_iter().chain(chart.height.to_le_bytes()) {
        feed(b);
    }
    feed(chart.craft as u8);
    feed(chart.technique as u8);
    feed(chart.construction as u8);
    for &c in &chart.cells {
        feed(c);
    }
    h
}

/// Moves `current` (0-based) by `delta` rows within `rows` rows, stopping at the first and last.
pub fn step(current: usize, delta: i32, rows: usize) -> usize {
    let last = rows.saturating_sub(1);
    let moved = i64::try_from(current).unwrap_or(i64::MAX).saturating_add(i64::from(delta));
    usize::try_from(moved.max(0)).unwrap_or(last).min(last)
}

/// The saved position (0-based row) for each pattern.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Progress {
    entries: BTreeMap<u64, usize>,
}

impl Progress {
    /// The saved row for `chart`, kept inside the chart (0 when none is saved).
    pub fn position(&self, chart: &Chart) -> usize {
        self.entries.get(&pattern_key(chart)).copied().unwrap_or(0).min((chart.height as usize).saturating_sub(1))
    }

    /// Remembers `row` for `chart`.
    pub fn set(&mut self, chart: &Chart, row: usize) {
        let key = pattern_key(chart);
        if self.entries.len() < MAX_ENTRIES || self.entries.contains_key(&key) {
            self.entries.insert(key, row);
        }
    }

    /// One `key row` line per pattern, the key in hex.
    pub fn to_text(&self) -> String {
        self.entries.iter().map(|(k, v)| format!("{k:016x} {v}\n")).collect()
    }

    /// Reads [`Self::to_text`] output; lines that do not parse are skipped.
    pub fn from_text(text: &str) -> Self {
        let entries = text
            .lines()
            .filter_map(|l| {
                let (k, v) = l.split_once(' ')?;
                Some((u64::from_str_radix(k, 16).ok()?, v.trim().parse::<usize>().ok()?))
            })
            .take(MAX_ENTRIES)
            .collect();
        Self { entries }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chart::{Construction, Craft, PaletteColor, Technique};
    use crate::gauge::{Gauge, Unit};
    use crate::instructions::instructions;

    fn chart(cells: Vec<u8>, width: u32, height: u32) -> Chart {
        let palette = vec![PaletteColor { name: "cream".into(), rgb: [255, 255, 240] }, PaletteColor { name: "plum".into(), rgb: [90, 30, 80] }];
        let gauge = Gauge::new(20.0, 28.0, 4.0, Unit::Inches).unwrap();
        let mut c = Chart::blank(width, height, palette, Craft::Knit, Technique::Intarsia, Construction::Flat, gauge).unwrap();
        c.cells = cells;
        c
    }

    #[test]
    fn stepping_stops_at_both_ends() {
        assert_eq!(step(0, -1, 5), 0);
        assert_eq!(step(0, 1, 5), 1);
        assert_eq!(step(4, 1, 5), 4);
        assert_eq!(step(2, i32::MIN, 5), 0);
        assert_eq!(step(0, 1, 0), 0);
    }

    #[test]
    fn current_row_has_its_stitch_total() {
        let c = chart(vec![0, 1, 1, 0, 0, 0], 3, 2);
        let rows = instructions(&c).rows;
        let at = step(0, 1, rows.len());
        assert_eq!(rows[at].stitches, 3);
        assert!(rows[at].text.starts_with("Row 2"));
    }

    #[test]
    fn position_is_saved_per_pattern() {
        let a = chart(vec![0, 1, 1, 0, 0, 0], 3, 2);
        let b = chart(vec![1, 1, 1, 0, 0, 0], 3, 2);
        let mut p = Progress::default();
        p.set(&a, 1);
        let back = Progress::from_text(&p.to_text());
        assert_eq!(back, p);
        assert_eq!(back.position(&a), 1);
        assert_eq!(back.position(&b), 0);
    }

    #[test]
    fn hostile_text_is_ignored_and_positions_are_clamped() {
        let a = chart(vec![0, 1, 1, 0, 0, 0], 3, 2);
        let mut p = Progress::from_text("garbage\n\nzz 1\n 5\n");
        assert_eq!(p, Progress::default());
        p.set(&a, 99);
        assert_eq!(p.position(&a), 1);
        assert_eq!(Progress::from_text(&"1 99999999999999999999999\n".repeat(3)), Progress::default());
    }
}
