//! Resize a written round-by-round pattern: same design at a new size, yarn or hook.
//!
//! Our own geometry: stitch counts scale with circumference, round counts with height. Resizing is an estimate:
//! make a test piece.

use crate::gauge::Gauge;
use crate::pattern::{PatternRound, Terms, written_rounds};
use crate::rounds::{MAX_ROUNDS, RING, Round, smooth};

/// Longest pattern text we read, in bytes.
pub const MAX_TEXT: usize = 1_000_000;
/// Deepest bracket nesting we read.
const MAX_DEPTH: usize = 8;
/// Largest scale we accept, as a factor.
pub const MAX_SCALE: f64 = 20.0;

/// Why a pattern could not be read or resized.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResizeError {
    #[error("the pattern is empty")]
    Empty,
    #[error("the pattern is too long")]
    TooLong,
    #[error("line {line}: {why}")]
    Line { line: usize, why: String },
    #[error("the scale must be a number above 0 and at most {MAX_SCALE} times")]
    BadScale,
    #[error("the pattern would need more than {MAX_ROUNDS} rounds")]
    TooManyRounds,
    #[error("gauge: {0}")]
    Gauge(#[from] crate::gauge::GaugeError),
}

fn bad(line: usize, why: &str) -> ResizeError {
    ResizeError::Line { line, why: why.to_string() }
}

/// What one round used from the round below, and made.
type Used = (u32, u32);

/// Reads a comma-separated list of stitch groups, returning (used, made) and the rest of the text.
fn list(s: &str, depth: usize) -> Result<(Used, &str), String> {
    if depth > MAX_DEPTH {
        return Err("brackets are nested too deeply".into());
    }
    let (mut used, mut made) = (0u32, 0u32);
    let mut rest = s.trim_start();
    loop {
        let ((u, m), r) = item(rest, depth)?;
        used = used.checked_add(u).ok_or("too many stitches")?;
        made = made.checked_add(m).ok_or("too many stitches")?;
        rest = r.trim_start();
        match rest.strip_prefix(',') {
            Some(r) => rest = r.trim_start(),
            None => return Ok(((used, made), rest)),
        }
    }
}

fn number(s: &str) -> Option<(u32, &str)> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let n = s.get(..end)?.parse::<u32>().ok().filter(|n| *n <= 100_000)?;
    Some((n, s.get(end..)?))
}

/// An optional `xN` repeat.
fn times(s: &str) -> Result<(u32, &str), String> {
    let t = s.trim_start();
    match t.strip_prefix('x').or_else(|| t.strip_prefix('X')) {
        Some(r) => number(r.trim_start()).filter(|(n, _)| *n > 0).ok_or_else(|| "expected a number after x".to_string()),
        None => Ok((1, s)),
    }
}

fn scale_used((u, m): Used, n: u32) -> Result<Used, String> {
    Ok((u.checked_mul(n).ok_or("too many stitches")?, m.checked_mul(n).ok_or("too many stitches")?))
}

fn item(s: &str, depth: usize) -> Result<(Used, &str), String> {
    if let Some(inner) = s.strip_prefix('(') {
        let (u, r) = list(inner, depth + 1)?;
        let r = r.strip_prefix(')').ok_or("missing )")?;
        let (n, r) = times(r)?;
        return Ok((scale_used(u, n)?, r));
    }
    let word_end = s.find(|c: char| !c.is_ascii_alphabetic()).unwrap_or(s.len());
    let word = s.get(..word_end).unwrap_or("").to_ascii_lowercase();
    let r = s.get(word_end..).unwrap_or("");
    match word.as_str() {
        "inc" => {
            let (n, r) = times(r)?;
            Ok((scale_used((1, 2), n)?, r))
        }
        "dec" => {
            let (n, r) = times(r)?;
            Ok((scale_used((2, 1), n)?, r))
        }
        "sc" | "dc" => Ok(match number(r.trim_start()) {
            Some((n, r2)) => ((n, n), r2),
            None => ((1, 1), r),
        }),
        "" => {
            // `6 sc in magic ring`: only the first round starts this way.
            let (n, r) = number(s).ok_or("cannot read this")?;
            let r = r.trim_start();
            let r = r.strip_prefix("sc").or_else(|| r.strip_prefix("dc")).ok_or("expected sc after the number")?;
            let r = r.trim_start().strip_prefix("in magic ring").ok_or("expected 'in magic ring'")?;
            Ok(((0, n), r))
        }
        other => Err(format!("do not know '{other}'")),
    }
}

/// Reads rounds typed or pasted in the notation `Rnd 3: (sc, inc) x6 (18)`. Blank lines are skipped.
/// Reports the exact line it cannot read; nothing is guessed. Returns the stitch count of each round.
pub fn parse(text: &str) -> Result<Vec<u32>, ResizeError> {
    if text.len() > MAX_TEXT {
        return Err(ResizeError::TooLong);
    }
    let mut counts: Vec<u32> = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        let line = i + 1;
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }
        if counts.len() >= MAX_ROUNDS {
            return Err(ResizeError::TooManyRounds);
        }
        let rest =
            t.get(..3).filter(|p| p.eq_ignore_ascii_case("rnd")).and_then(|_| t.get(3..)).ok_or_else(|| bad(line, "a round starts with 'Rnd'"))?;
        let (num, rest) = number(rest.trim_start()).ok_or_else(|| bad(line, "expected the round number after 'Rnd'"))?;
        if num as usize != counts.len() + 1 {
            return Err(bad(line, &format!("expected round {}, found round {num}", counts.len() + 1)));
        }
        let body = rest.trim_start().strip_prefix(':').ok_or_else(|| bad(line, "expected ':' after the round number"))?;
        let missing = "the stitch count in (brackets) is missing at the end";
        let body = body.trim_end().strip_suffix(')').ok_or_else(|| bad(line, missing))?;
        let open = body.rfind('(').ok_or_else(|| bad(line, missing))?;
        let stated = body
            .get(open + 1..)
            .and_then(|c| c.trim().parse::<u32>().ok())
            .ok_or_else(|| bad(line, "the stitch count at the end is not a number"))?;
        let ((used, made), left) = list(body.get(..open).unwrap_or(""), 0).map_err(|e| bad(line, &e))?;
        if !left.trim().is_empty() {
            return Err(bad(line, &format!("cannot read '{}'", left.trim())));
        }
        if made != stated {
            return Err(bad(line, &format!("the stitches add up to {made} but the end says ({stated})")));
        }
        match counts.last() {
            None if used != 0 || made != RING => return Err(bad(line, "the first round must be 6 in a magic ring")),
            Some(&prev) if used != prev => return Err(bad(line, &format!("this round works {used} stitches but the round before has {prev}"))),
            _ => {}
        }
        counts.push(made);
    }
    if counts.is_empty() { Err(ResizeError::Empty) } else { Ok(counts) }
}

/// How to resize.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResizeOptions {
    /// Finished size as a factor of the original (1.0 = same size, 2.0 = 200%).
    pub scale: f64,
    /// The gauge the pattern was written for and the new one (a new yarn or hook); `None` keeps it.
    pub gauges: Option<(Gauge, Gauge)>,
    pub terms: Terms,
}

impl Default for ResizeOptions {
    fn default() -> Self {
        Self { scale: 1.0, gauges: None, terms: Terms::Us }
    }
}

/// An old and a new round side by side (either side is `None` where one pattern is longer).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub old: Option<u32>,
    pub new: Option<u32>,
}

/// The resized pattern.
#[derive(Clone, Debug, PartialEq)]
pub struct Resized {
    pub rounds: Vec<PatternRound>,
    pub table: Vec<Row>,
}

/// Resizes a list of round counts: stitch counts scale with circumference, round counts with height.
/// The first round stays at 6 and the changes stay buildable (see `rounds::smooth`).
pub fn resize_counts(old: &[u32], o: &ResizeOptions) -> Result<Vec<u32>, ResizeError> {
    if !o.scale.is_finite() || o.scale <= 0.0 || o.scale > MAX_SCALE {
        return Err(ResizeError::BadScale);
    }
    let (wide, tall) = match o.gauges {
        Some((a, b)) => {
            a.validate()?;
            b.validate()?;
            (b.stitches_per_inch() / a.stitches_per_inch(), b.rows_per_inch() / a.rows_per_inch())
        }
        None => (1.0, 1.0),
    };
    let (cs, hs) = (o.scale * wide, o.scale * tall);
    let n = old.len();
    let m = (n as f64 * hs).round();
    if !m.is_finite() || m > MAX_ROUNDS as f64 {
        return Err(ResizeError::TooManyRounds);
    }
    let m = (m as usize).clamp(2, MAX_ROUNDS);
    if n == m && (cs - 1.0).abs() < 1e-9 {
        return Ok(old.to_vec());
    }
    let last = n.saturating_sub(1);
    let targets: Vec<u32> = (0..m)
        .map(|i| {
            let t = i as f64 * last as f64 / (m - 1) as f64;
            let lo = (t.floor() as usize).min(last);
            let hi = (lo + 1).min(last);
            let (a, b) = (f64::from(old.get(lo).copied().unwrap_or(RING)), f64::from(old.get(hi).copied().unwrap_or(RING)));
            let v = (a + (b - a) * (t - lo as f64)) * cs;
            v.round().clamp(f64::from(RING), 1.0e6) as u32
        })
        .collect();
    Ok(smooth(&targets, old.last() == Some(&RING)))
}

/// Resizes pattern text, giving the new pattern in the same notation and an old-versus-new table.
pub fn resize(text: &str, o: &ResizeOptions) -> Result<Resized, ResizeError> {
    let old = parse(text)?;
    let new = resize_counts(&old, o)?;
    let rounds: Vec<Round> =
        new.iter().enumerate().map(|(i, &c)| Round { index: i + 1, stitches: c, z: 0.0, center: [0.0, 0.0], radius: 0.0 }).collect();
    let table = (0..old.len().max(new.len())).map(|i| Row { old: old.get(i).copied(), new: new.get(i).copied() }).collect();
    Ok(Resized { rounds: written_rounds(&rounds, o.terms), table })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn counts_text(c: &[u32]) -> String {
        let rounds: Vec<Round> =
            c.iter().enumerate().map(|(i, &s)| Round { index: i + 1, stitches: s, z: 0.0, center: [0.0; 2], radius: 0.0 }).collect();
        written_rounds(&rounds, Terms::Us).iter().map(|r| r.text.clone()).collect::<Vec<_>>().join("\n")
    }

    /// A ball: 6 up to 36 and back down.
    fn ball() -> Vec<u32> {
        let mut c: Vec<u32> = (1..=6).map(|i| i * 6).collect();
        c.extend([36, 36, 36]);
        c.extend((1..=5).rev().map(|i| i * 6));
        c
    }

    #[test]
    fn reads_the_notation_it_writes() {
        let c = ball();
        assert_eq!(parse(&counts_text(&c)).unwrap(), c);
        assert_eq!(parse("Rnd 1: 6 sc in magic ring (6)\nRnd 2: inc x6 (12)\n\nRnd 3: (sc, inc) x6 (18)").unwrap(), vec![6, 12, 18]);
    }

    #[test]
    fn full_size_returns_the_identical_pattern() {
        let text = counts_text(&ball());
        let r = resize(&text, &ResizeOptions::default()).unwrap();
        let out = r.rounds.iter().map(|r| r.text.clone()).collect::<Vec<_>>().join("\n");
        assert_eq!(out, text);
    }

    #[test]
    fn double_size_doubles_stitches_and_rounds() {
        let old = ball();
        let new = resize_counts(&old, &ResizeOptions { scale: 2.0, ..Default::default() }).unwrap();
        let (om, nm) = (old.iter().max().unwrap(), new.iter().max().unwrap());
        assert!((f64::from(*nm) / f64::from(*om) - 2.0).abs() < 0.2, "{om} -> {nm}");
        assert!((new.len() as f64 / old.len() as f64 - 2.0).abs() < 0.2);
        assert_eq!(new.first(), Some(&RING));
        assert_eq!(new.last(), Some(&RING));
    }

    #[test]
    fn a_thicker_yarn_means_fewer_stitches() {
        let g = |s| Gauge::new(s, s, 4.0, crate::gauge::Unit::Inches).unwrap();
        let o = ResizeOptions { gauges: Some((g(20.0), g(10.0))), ..Default::default() };
        let new = resize_counts(&ball(), &o).unwrap();
        assert_eq!(new.iter().max(), Some(&18));
    }

    #[test]
    fn a_malformed_line_is_reported_with_its_number() {
        let e = parse("Rnd 1: 6 sc in magic ring (6)\nRnd 2: inc x6 (12)\nRnd 3: (sc, inc) x6 (19)").unwrap_err();
        assert!(matches!(e, ResizeError::Line { line: 3, .. }), "{e}");
        let e = parse("Rnd 1: 6 sc in magic ring (6)\nhello").unwrap_err();
        assert!(matches!(e, ResizeError::Line { line: 2, .. }));
        let e = parse("Rnd 1: 6 sc in magic ring (6)\nRnd 3: sc 6 (6)").unwrap_err();
        assert!(matches!(e, ResizeError::Line { line: 2, .. }));
        assert_eq!(parse("  \n"), Err(ResizeError::Empty));
    }

    #[test]
    fn bad_scales_are_refused() {
        for s in [0.0, -1.0, f64::NAN, f64::INFINITY, 1e9] {
            assert_eq!(resize_counts(&ball(), &ResizeOptions { scale: s, ..Default::default() }), Err(ResizeError::BadScale));
        }
    }

    proptest! {
        #[test]
        fn hostile_text_never_panics(s in "\\PC{0,300}") {
            let _ = parse(&s);
        }

        #[test]
        fn hostile_rounds_never_panic(s in "(Rnd [0-9]{1,3}: [()a-z0-9, x]{0,40}\\([0-9]{0,3}\\)\n){0,6}") {
            let _ = parse(&s);
        }

        #[test]
        fn resized_rounds_are_always_buildable(scale in 0.2f64..4.0) {
            let new = resize_counts(&ball(), &ResizeOptions { scale, ..Default::default() }).unwrap();
            prop_assert_eq!(new[0], RING);
            for w in new.windows(2) {
                prop_assert!(w[1].abs_diff(w[0]) <= 6);
            }
        }
    }
}
