//! The written amigurumi pattern: round by round, with increases and decreases spread evenly and staggered.
//!
//! Format: `Rnd 1: 6 sc in magic ring (6)`, `Rnd 2: inc x6 (12)`, `Rnd 3: (sc, inc) x6 (18)`, `Rnd 8: sc 36 (36)`.
//! An `inc` is two stitches in one; a `dec` joins two stitches into one. US terms (sc) or UK terms (dc).

use crate::rounds::{RING, Round, Stitch};
use crate::setup::YarnWeight;

/// US or UK crochet terms.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Terms {
    #[default]
    Us,
    Uk,
}

impl Terms {
    /// The plain stitch: single crochet (US) or double crochet (UK).
    pub fn plain(self) -> &'static str {
        match self {
            Terms::Us => "sc",
            Terms::Uk => "dc",
        }
    }
}

/// One written round.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PatternRound {
    pub index: usize,
    pub stitches: u32,
    /// The whole line, e.g. `Rnd 3: (sc, inc) x6 (18)`.
    pub text: String,
}

/// Where the safety eyes go on a part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EyeMark {
    /// Between this round and the next.
    pub round: usize,
    /// Stitches between the two eyes (0 or 1 means one eye, centred).
    pub apart: u32,
}

/// The written pattern for one part, with notes and totals.
#[derive(Clone, Debug, PartialEq)]
pub struct PartPattern {
    pub name: String,
    pub rounds: Vec<PatternRound>,
    /// Lines after the last round (fasten off, close, sew).
    pub finishing: Vec<String>,
    /// Where to start stuffing and where the eyes go.
    pub notes: Vec<String>,
    pub total_stitches: u32,
    /// Estimated yarn for this part in yards (an estimate, not a promise).
    pub yards: f64,
}

/// Yarn for a number of stitches, in yards. A simple factor per yarn weight, plus 10% for ends and mistakes.
/// It is an estimate: a swatch and a weighed ball are better.
pub fn yardage(stitches: u32, weight: YarnWeight) -> f64 {
    f64::from(stitches) * weight.inches_per_stitch() * 1.10 / 36.0
}

/// How many stitches of the previous round to work plain between each change, spread evenly (Bresenham style).
fn gaps(plain: u32, changes: u32) -> Vec<u32> {
    (0..changes).map(|j| ((j + 1) * plain) / changes - (j * plain) / changes).collect()
}

/// The shortest run of gaps that, repeated, makes the whole list.
fn period_of(g: &[u32]) -> usize {
    (1..=g.len()).find(|p| g.len().is_multiple_of(*p) && g.iter().enumerate().all(|(i, x)| *x == g[i % p])).unwrap_or(g.len())
}

/// One group of the repeat: `sc 2, inc` (the change is `inc` or `dec`).
fn group(gap: u32, change: &str, terms: Terms) -> String {
    match gap {
        0 => change.to_string(),
        1 => format!("{}, {change}", terms.plain()),
        g => format!("{} {g}, {change}", terms.plain()),
    }
}

/// `(a) x3` style: a group repeated `n` times (no brackets for a single bare token).
fn repeat(text: String, n: usize) -> String {
    if n <= 1 {
        text
    } else if text.contains(", ") || text.contains(' ') && !text.starts_with("inc") && !text.starts_with("dec") {
        format!("({text}) x{n}")
    } else {
        format!("{text} x{n}")
    }
}

/// The instruction for one round worked over `prev` stitches, making `n`. `change_no` counts the earlier rounds
/// that had increases or decreases, so every other one starts half a gap later (staggered).
pub fn round_text(prev: u32, n: u32, change_no: usize, terms: Terms) -> String {
    let plain_st = terms.plain();
    if n == prev {
        return format!("{plain_st} {n}");
    }
    let inc = n > prev;
    let changes = if inc { n - prev } else { prev - n };
    let used = if inc { changes } else { 2 * changes };
    let plain = prev.saturating_sub(used);
    let change = if inc { "inc" } else { "dec" };
    let mut g = gaps(plain, changes);
    // Stagger: on alternate changing rounds, start `o` plain stitches earlier by moving them from the last gap.
    let o = match (change_no % 2, g.last()) {
        (1, Some(&last)) => (last / 2).min(g.iter().copied().max().unwrap_or(0) / 2),
        _ => 0,
    };
    if let Some(last) = g.last_mut() {
        *last -= o.min(*last);
    }
    let mut parts: Vec<String> = Vec::new();
    // Whole thing one repeating group?
    let period = period_of(&g);
    if o > 0 {
        // The stitches moved to the front join the first group when it is a single one, so "sc 12, sc 24" reads "sc 36".
        let first_is_single = g.len() == 1 || (period == g.len() && g.get(1) != g.first());
        if first_is_single {
            g[0] += o;
        } else {
            parts.push(if o == 1 { plain_st.to_string() } else { format!("{plain_st} {o}") });
        }
    }
    let period = period_of(&g);
    if period < g.len() {
        let inner = g[..period].iter().map(|&x| group(x, change, terms)).collect::<Vec<_>>().join(", ");
        parts.push(repeat(inner, g.len() / period));
    } else {
        // Runs of identical groups.
        let mut i = 0;
        while i < g.len() {
            let mut j = i;
            while j + 1 < g.len() && g[j + 1] == g[i] {
                j += 1;
            }
            parts.push(repeat(group(g[i], change, terms), j - i + 1));
            i = j + 1;
        }
    }
    parts.join(", ")
}

/// The written lines for a list of rounds.
pub fn written_rounds(rounds: &[Round], terms: Terms) -> Vec<PatternRound> {
    let mut out = Vec::with_capacity(rounds.len());
    let mut changes = 0usize;
    let mut prev = RING;
    for (i, r) in rounds.iter().enumerate() {
        let body = if i == 0 {
            format!("{} {} in magic ring", r.stitches, terms.plain())
        } else {
            let t = round_text(prev, r.stitches, changes, terms);
            if r.stitches != prev {
                changes += 1;
            }
            t
        };
        out.push(PatternRound { index: r.index, stitches: r.stitches, text: format!("Rnd {}: {body} ({})", r.index, r.stitches) });
        prev = r.stitches;
    }
    out
}

/// Nearest round to a marked point on the model (distance to the ring), if there are any rounds.
pub fn round_near(rounds: &[Round], point: [f64; 3]) -> Option<usize> {
    rounds
        .iter()
        .map(|r| {
            let rho = ((point[0] - r.center[0]).powi(2) + (point[1] - r.center[1]).powi(2)).sqrt();
            (r.index, (rho - r.radius).powi(2) + (point[2] - r.z).powi(2))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|x| x.0)
}

/// Safety eyes for a marked point (one eye; the other mirrors it across the front): which round and how far apart.
pub fn eyes_at(rounds: &[Round], marker: [f64; 3], stitch: Stitch) -> Option<EyeMark> {
    let round = round_near(rounds, marker)?;
    let r = rounds.iter().find(|r| r.index == round)?;
    let apart = (2.0 * (marker[0] - r.center[0]).abs() / stitch.width).round() as u32;
    Some(EyeMark { round, apart })
}

/// A default eye position for a head: about 60% of the way through the rounds, a quarter of the way round apart.
pub fn default_eyes(rounds: &[Round]) -> Option<EyeMark> {
    let r = rounds.get((rounds.len() * 3 / 5).min(rounds.len().saturating_sub(1)))?;
    Some(EyeMark { round: r.index, apart: (r.stitches / 4).max(2) })
}

/// The full written pattern for one part.
/// `closed_end`: the last round closes to 6 (otherwise the end is sewn on). `eyes`: where the safety eyes go, if any.
pub fn part_pattern(name: &str, rounds: &[Round], closed_end: bool, terms: Terms, weight: YarnWeight, eyes: Option<&EyeMark>) -> PartPattern {
    let written = written_rounds(rounds, terms);
    let total: u32 = rounds.iter().map(|r| r.stitches).sum();
    let mut finishing = Vec::new();
    let mut notes = Vec::new();
    let max = rounds.iter().map(|r| r.stitches).max().unwrap_or(0);
    let widest_at = rounds.iter().position(|r| r.stitches == max).unwrap_or(0);
    if closed_end {
        finishing.push(format!(
            "Fasten off, leaving a 6 in tail. Thread the tail through the front loops of the last {RING} stitches, pull closed and weave in the end."
        ));
        let stuff =
            rounds.iter().skip(widest_at).find(|r| r.stitches * 10 <= max * 6).map_or(rounds.len().saturating_sub(1), |r| r.index.saturating_sub(1));
        notes.push(format!(
            "Start stuffing after Rnd {}. Stuff firmly, keep adding as the opening closes, and finish stuffing before the last 2 rounds.",
            stuff.max(1)
        ));
    } else {
        finishing.push("Fasten off, leaving a long tail (about 20 in) for sewing this part on.".to_string());
        notes.push("Stuff firmly but leave the open end flat so it sews on neatly.".to_string());
    }
    if let Some(e) = eyes {
        let where_ =
            if e.apart >= 2 { format!("about {} stitches apart, centred on the front", e.apart) } else { "centred on the front".to_string() };
        notes.push(format!(
            "Safety eyes: place them between Rnd {} and Rnd {}, {where_}. Push the posts through before you stuff and fit the backs firmly.",
            e.round,
            e.round + 1
        ));
    }
    PartPattern { name: name.to_string(), rounds: written, finishing, notes, total_stitches: total, yards: yardage(total, weight) }
}

/// Abbreviations used by the written rounds, for the key at the end of the pattern.
pub fn abbreviations(terms: Terms) -> Vec<(&'static str, String)> {
    let plain = match terms {
        Terms::Us => ("sc", "single crochet".to_string()),
        Terms::Uk => ("dc", "double crochet (UK; a US single crochet)".to_string()),
    };
    let dec = match terms {
        Terms::Us => "decrease: single crochet two stitches together (invisible decrease: front loops only)".to_string(),
        Terms::Uk => "decrease: double crochet two stitches together (invisible decrease: front loops only)".to_string(),
    };
    vec![
        plain,
        ("inc", format!("increase: 2 {} in the same stitch", terms.plain())),
        ("dec", dec),
        ("Rnd", "round (work in a spiral; mark the first stitch of each round)".to_string()),
        ("x", "times, e.g. (sc, inc) x6 means work what is in the brackets 6 times".to_string()),
        ("(n)", "the number of stitches you should have at the end of the round".to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauge::{Gauge, Unit};
    use crate::{models, rounds_for_mesh};

    fn r(counts: &[u32]) -> Vec<Round> {
        counts.iter().enumerate().map(|(i, &c)| Round { index: i + 1, stitches: c, z: i as f64, center: [0.0, 0.0], radius: 1.0 }).collect()
    }

    /// Reads a round's instruction back into (stitches used, stitches made) to check the arithmetic.
    fn evaluate(text: &str) -> (u32, u32) {
        fn eval(s: &str) -> (u32, u32) {
            // split on top-level commas
            let (mut depth, mut start, mut parts) = (0, 0, Vec::new());
            for (i, c) in s.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => depth -= 1,
                    ',' if depth == 0 => {
                        parts.push(&s[start..i]);
                        start = i + 1;
                    }
                    _ => {}
                }
            }
            parts.push(&s[start..]);
            let (mut used, mut made) = (0, 0);
            for p in parts {
                let p = p.trim();
                let (body, times) = match p.rsplit_once(" x") {
                    Some((b, t)) if t.chars().all(|c| c.is_ascii_digit()) && !t.is_empty() => (b, t.parse::<u32>().unwrap()),
                    _ => (p, 1),
                };
                let (u, m) = if let Some(inner) = body.strip_prefix('(').and_then(|b| b.strip_suffix(')')) {
                    eval(inner)
                } else if body == "inc" {
                    (1, 2)
                } else if body == "dec" {
                    (2, 1)
                } else if let Some(n) = body.strip_prefix("sc ").or_else(|| body.strip_prefix("dc ")) {
                    let n: u32 = n.parse().unwrap();
                    (n, n)
                } else if body == "sc" || body == "dc" {
                    (1, 1)
                } else {
                    panic!("cannot read {body:?} in {s:?}")
                };
                used += u * times;
                made += m * times;
            }
            (used, made)
        }
        let body = text.split_once(": ").unwrap().1;
        let body = body.rsplit_once(" (").unwrap().0;
        eval(body)
    }

    #[test]
    fn the_classic_ball_reads_like_a_pattern() {
        let lines: Vec<String> = written_rounds(&r(&[6, 12, 18, 24, 30, 36, 36, 36, 30]), Terms::Us).into_iter().map(|x| x.text).collect();
        assert_eq!(lines[0], "Rnd 1: 6 sc in magic ring (6)");
        assert_eq!(lines[1], "Rnd 2: inc x6 (12)");
        assert_eq!(lines[2], "Rnd 3: (sc, inc) x6 (18)");
        assert_eq!(lines[6], "Rnd 7: sc 36 (36)");
        assert!(lines[3].contains("sc 2") && lines[3].ends_with("(24)"), "{}", lines[3]);
        assert!(lines[8].contains("dec") && lines[8].ends_with("(30)"));
    }

    #[test]
    fn every_line_adds_up_for_every_change() {
        for prev in 6..60u32 {
            for n in 6..=(prev + 6).min(60) {
                if n.abs_diff(prev) > 6 || (n < prev && 2 * (prev - n) > prev) {
                    continue;
                }
                for change_no in 0..4 {
                    let t = round_text(prev, n, change_no, Terms::Us);
                    let line = format!("Rnd 9: {t} ({n})");
                    assert_eq!(evaluate(&line), (prev, n), "{prev}->{n} #{change_no}: {t}");
                }
            }
        }
    }

    #[test]
    fn alternate_changing_rounds_start_at_a_different_place() {
        let a = round_text(30, 36, 0, Terms::Us);
        let b = round_text(30, 36, 1, Terms::Us);
        assert_ne!(a, b, "{a} / {b}");
        assert_eq!(evaluate(&format!("Rnd 1: {a} (36)")), (30, 36));
    }

    #[test]
    fn uk_terms_use_dc() {
        let lines = written_rounds(&r(&[6, 12, 18]), Terms::Uk);
        assert_eq!(lines[0].text, "Rnd 1: 6 dc in magic ring (6)");
        assert_eq!(lines[2].text, "Rnd 3: (dc, inc) x6 (18)");
        assert!(abbreviations(Terms::Uk).iter().any(|a| a.0 == "dc"));
    }

    #[test]
    fn stuffing_eyes_totals_and_yardage() {
        let counts = [6, 12, 18, 24, 30, 36, 36, 36, 30, 24, 18, 12, 6];
        let rounds = r(&counts);
        let eyes = EyeMark { round: 6, apart: 9 };
        let p = part_pattern("Head", &rounds, true, Terms::Us, YarnWeight::Worsted, Some(&eyes));
        assert_eq!(p.total_stitches, counts.iter().sum::<u32>());
        assert!(p.notes.iter().any(|n| n.contains("Start stuffing after Rnd")));
        assert!(p.notes.iter().any(|n| n.contains("between Rnd 6 and Rnd 7") && n.contains("9 stitches apart")));
        assert!(p.finishing[0].contains("Fasten off"));
        assert!(p.yards > 0.0 && p.yards < 50.0);
        let open = part_pattern("Arm", &rounds, false, Terms::Us, YarnWeight::Worsted, None);
        assert!(open.finishing[0].contains("sewing"));
        // Heavier yarn needs more for the same stitches.
        assert!(yardage(100, YarnWeight::Bulky) > yardage(100, YarnWeight::Fingering));
    }

    #[test]
    fn eyes_come_from_a_marked_point() {
        let g = Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap();
        let st = Stitch::from_gauge(&g).unwrap();
        let rounds = rounds_for_mesh(&models::sphere(1.0), &g).unwrap();
        let e = eyes_at(&rounds, [0.4, -0.8, 1.4], st).unwrap();
        let near = rounds.iter().find(|x| x.index == e.round).unwrap();
        assert!((near.z - 1.4).abs() < 0.3);
        assert!(e.apart >= 2);
        assert!(default_eyes(&rounds).is_some());
        assert!(eyes_at(&[], [0.0; 3], st).is_none());
    }

    #[test]
    fn a_whole_ball_pattern_adds_up_round_to_round() {
        let g = Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap();
        let rounds = rounds_for_mesh(&models::sphere(1.5), &g).unwrap();
        let lines = written_rounds(&rounds, Terms::Us);
        for (i, l) in lines.iter().enumerate().skip(1) {
            assert_eq!(evaluate(&l.text), (lines[i - 1].stitches, l.stitches), "{}", l.text);
        }
    }
}
