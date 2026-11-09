//! Parts: split a figure into pieces (body, head, arms, legs, ears) that are crocheted separately and sewn together.
//!
//! Our own plain-geometry method: slice the figure at fine heights, follow how the loops of each slice join and
//! separate from one height to the next (a limb appears as its own loop, then joins the body), and cut there. A
//! single chain of loops is also cut where it narrows sharply (a neck). Each part then gets its own rounds,
//! starting at a closed tip where there is one.

use std::collections::{HashMap, HashSet};

use crate::gauge::Gauge;
use crate::mesh::Mesh;
use crate::pattern::{EyeMark, PartPattern, Terms, eyes_at, part_pattern};
use crate::rounds::{ProfilePoint, Round, RoundsError, Stitch, make_rounds};
use crate::setup::YarnWeight;
use crate::slice::slices;

/// A neck is a narrowing to less than this fraction of the smaller bulge either side of it.
pub const NECK_RATIO: f64 = 0.6;

/// The user's changes to the automatic split.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ManualSplits {
    /// Extra split planes, as heights in inches.
    pub add: Vec<f64>,
    /// Automatic neck splits to skip, by position in [`Figure::auto_splits`] (lowest first).
    pub remove: Vec<usize>,
}

/// Where one part is sewn to another.
#[derive(Clone, Debug, PartialEq)]
pub struct Attach {
    /// Index of the part it is sewn to.
    pub host: usize,
    /// The two neighbouring rounds of the host the join falls between (lower number first).
    pub host_rounds: (usize, usize),
    /// Height of the join in the model.
    pub z: f64,
}

/// One crocheted part.
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    pub name: String,
    pub rounds: Vec<Round>,
    /// The last round closes to 6 (a whole closed shape); otherwise the end is open and sewn on.
    pub closed_last: bool,
    /// Worked from the top of the model downwards (started at its tip, which is at the top).
    pub from_top: bool,
    /// Where this part is sewn on (empty for the main body and for loose pieces).
    pub attach: Vec<Attach>,
    /// The widest the part gets, in inches around.
    pub peak_perimeter: f64,
}

/// The split figure.
#[derive(Clone, Debug, PartialEq)]
pub struct Figure {
    pub parts: Vec<Part>,
    /// Heights (inches, lowest first) where the shape narrows like a neck; manual changes refer to these.
    pub auto_splits: Vec<f64>,
    pub notes: Vec<String>,
}

#[derive(Clone)]
struct FChain {
    nodes: Vec<usize>,
    start_closed: bool,
    end_closed: bool,
    /// Pieces made by cutting one chain share this.
    dropped: bool,
}

/// Splits `mesh` (upright, in inches) into parts and gives each its rounds.
pub fn split_figure(mesh: &Mesh, gauge: &Gauge, manual: &ManualSplits) -> Result<Figure, RoundsError> {
    let stitch = Stitch::from_gauge(gauge)?;
    let (lo, hi) = mesh.bounds();
    let height = hi[2] - lo[2];
    if !(height.is_finite() && height > 0.0) {
        return Err(RoundsError::Flat);
    }
    let n = ((height / (stitch.height / 6.0)).ceil() as usize).clamp(48, 3000);
    let dz = height / n as f64;
    let sl = slices(mesh, n);

    // Nodes: one per loop per slice.
    let mut node_of: Vec<Vec<usize>> = Vec::new();
    let (mut zs, mut per, mut cen) = (Vec::new(), Vec::new(), Vec::new());
    for s in &sl {
        let mut ids = Vec::new();
        for l in &s.loops {
            ids.push(zs.len());
            zs.push(s.z);
            per.push(l.perimeter);
            cen.push(l.centroid);
        }
        node_of.push(ids);
    }
    let total = zs.len();
    let mut up = vec![Vec::new(); total];
    let mut down = vec![Vec::new(); total];
    for i in 0..sl.len().saturating_sub(1) {
        for (ja, &a) in node_of[i].iter().enumerate() {
            for (jb, &b) in node_of[i + 1].iter().enumerate() {
                if sl[i].loops[ja].overlaps(&sl[i + 1].loops[jb]) {
                    up[a].push(b);
                    down[b].push(a);
                }
            }
        }
    }
    if total == 0 {
        return Err(RoundsError::Flat);
    }

    // Chains: follow loops upwards; a chain continues through a plain one-to-one step, through a split into the
    // largest child (if it is not much smaller), and through a merge from the largest incoming loop (if the
    // merged loop is not much bigger). Anything else starts a new chain.
    let mut chain_of = vec![usize::MAX; total];
    let mut chains: Vec<FChain> = Vec::new();
    let largest = |ids: &[usize]| ids.iter().copied().max_by(|a, b| per[*a].total_cmp(&per[*b]));
    for ids in &node_of {
        for &nd in ids {
            let continues = match down[nd].as_slice() {
                [] => None,
                [p] => {
                    let p = *p;
                    if up[p].len() == 1 || (largest(&up[p]) == Some(nd) && per[nd] >= 0.5 * per[p]) { Some(p) } else { None }
                }
                many => largest(many).filter(|&m| up[m].len() == 1 && per[nd] <= 1.6 * per[m]),
            };
            match continues.map(|p| chain_of[p]).filter(|&c| chains[c].nodes.last().is_some_and(|&l| continues == Some(l))) {
                Some(c) => {
                    chain_of[nd] = c;
                    chains[c].nodes.push(nd);
                }
                None => {
                    chain_of[nd] = chains.len();
                    chains.push(FChain { nodes: vec![nd], start_closed: down[nd].is_empty(), end_closed: false, dropped: false });
                }
            }
        }
    }
    for c in &mut chains {
        c.end_closed = c.nodes.last().is_some_and(|&l| up[l].is_empty());
    }

    // Necks: where one chain's perimeter dips well below the bulges on both sides.
    let mut necks: Vec<(usize, usize, f64)> = Vec::new(); // (chain, position of last node below the cut, z)
    for (ci, c) in chains.iter().enumerate() {
        let p: Vec<f64> = c.nodes.iter().map(|&x| per[x]).collect();
        let m = p.len();
        if m < 6 {
            continue;
        }
        let mut left = vec![0.0_f64; m];
        let mut right = vec![0.0_f64; m];
        for i in 1..m {
            left[i] = left[i - 1].max(p[i - 1]);
        }
        for i in (0..m - 1).rev() {
            right[i] = right[i + 1].max(p[i + 1]);
        }
        let neck = |i: usize| p[i] < NECK_RATIO * left[i].min(right[i]);
        let mut i = 0;
        while i < m {
            if neck(i) {
                let mut j = i;
                while j + 1 < m && neck(j + 1) {
                    j += 1;
                }
                let best = (i..=j).min_by(|a, b| p[*a].total_cmp(&p[*b])).unwrap_or(i);
                if best >= 1 && best + 2 < m {
                    necks.push((ci, best, zs[c.nodes[best]]));
                }
                i = j + 1;
            } else {
                i += 1;
            }
        }
    }
    necks.sort_by(|a, b| a.2.total_cmp(&b.2));
    let auto_splits: Vec<f64> = necks.iter().map(|x| x.2).collect();
    let mut cuts: Vec<(usize, usize)> = necks.iter().enumerate().filter(|(i, _)| !manual.remove.contains(i)).map(|(_, x)| (x.0, x.1)).collect();
    for &z in &manual.add {
        if !z.is_finite() {
            continue;
        }
        for (ci, c) in chains.iter().enumerate() {
            let k = c.nodes.iter().rposition(|&x| zs[x] <= z).unwrap_or(0);
            if k >= 1 && k + 2 < c.nodes.len() && zs[c.nodes[k + 1]] > z {
                cuts.push((ci, k));
            }
        }
    }
    cuts.sort_unstable();
    cuts.dedup();

    // Cut chains into final pieces; remember the plain junctions between pieces of one chain.
    let mut finals: Vec<FChain> = Vec::new();
    let mut piece_links: Vec<(usize, usize, f64)> = Vec::new(); // (lower piece, upper piece, z of cut)
    let mut first_final = vec![0usize; chains.len()];
    for (ci, c) in chains.iter().enumerate() {
        first_final[ci] = finals.len();
        let mut at: Vec<usize> = cuts.iter().filter(|x| x.0 == ci).map(|x| x.1).collect();
        at.sort_unstable();
        let mut start = 0;
        let mut bounds: Vec<usize> = at.iter().map(|k| k + 1).collect();
        bounds.push(c.nodes.len());
        for (pi, &end) in bounds.iter().enumerate() {
            if end <= start {
                continue;
            }
            let last = pi + 1 == bounds.len();
            let idx = finals.len();
            finals.push(FChain {
                nodes: c.nodes[start..end].to_vec(),
                start_closed: start == 0 && c.start_closed,
                end_closed: last && c.end_closed,
                dropped: false,
            });
            if start > 0 {
                let z = (zs[c.nodes[start - 1]] + zs[c.nodes[start]]) / 2.0;
                piece_links.push((idx - 1, idx, z));
            }
            start = end;
        }
    }
    let mut final_of = vec![usize::MAX; total];
    for (fi, c) in finals.iter().enumerate() {
        for &x in &c.nodes {
            final_of[x] = fi;
        }
    }
    let mut notes = Vec::new();
    for c in &mut finals {
        if (c.nodes.len() as f64) * dz < 2.0 * stitch.height {
            c.dropped = true;
        }
    }
    let dropped_n = finals.iter().filter(|c| c.dropped).count();
    if dropped_n > 0 {
        notes.push(format!("{dropped_n} tiny piece(s) were too small to crochet and were left out."));
    }
    let peak = |c: &FChain| c.nodes.iter().map(|&x| per[x]).fold(0.0, f64::max);

    // Junctions: sets of chains that meet at one plane; the one with the widest girth is the host.
    struct Junction {
        members: Vec<usize>,
        z: f64,
    }
    let mut junctions: Vec<Junction> = Vec::new();
    for (fi, c) in finals.iter().enumerate() {
        let (Some(&first), Some(&last)) = (c.nodes.first(), c.nodes.last()) else { continue };
        if !c.start_closed && !down[first].is_empty() {
            let mut m: Vec<usize> = down[first].iter().map(|&d| final_of[d]).collect();
            m.push(fi);
            let z = down[first].iter().map(|&d| (zs[d] + zs[first]) / 2.0).sum::<f64>() / down[first].len() as f64;
            junctions.push(Junction { members: m, z });
        }
        if !c.end_closed && !up[last].is_empty() {
            let mut m: Vec<usize> = up[last].iter().map(|&d| final_of[d]).collect();
            m.push(fi);
            let z = up[last].iter().map(|&d| (zs[d] + zs[last]) / 2.0).sum::<f64>() / up[last].len() as f64;
            junctions.push(Junction { members: m, z });
        }
    }
    for &(a, b, z) in &piece_links {
        junctions.push(Junction { members: vec![a, b], z });
    }

    // Rounds for every kept piece.
    let mut parts: Vec<Part> = Vec::new();
    let mut part_of_final = vec![usize::MAX; finals.len()];
    let mut reversed_of = vec![false; finals.len()];
    for (fi, c) in finals.iter().enumerate() {
        if c.dropped {
            continue;
        }
        let mut profile: Vec<ProfilePoint> = c.nodes.iter().map(|&x| ProfilePoint { z: zs[x], perimeter: per[x], center: cen[x] }).collect();
        let reversed = !c.start_closed && c.end_closed;
        if reversed {
            profile.reverse();
        }
        let (cf, cl) = if reversed { (c.end_closed, c.start_closed) } else { (c.start_closed, c.end_closed) };
        let rounds = make_rounds(&profile, cf, cl, stitch)?;
        part_of_final[fi] = parts.len();
        reversed_of[fi] = reversed;
        parts.push(Part { name: String::new(), rounds, closed_last: cl, from_top: reversed, attach: Vec::new(), peak_perimeter: peak(c) });
    }
    if parts.is_empty() {
        return Err(RoundsError::Flat);
    }

    // Attachments.
    let mut seen: HashSet<(usize, usize)> = HashSet::new();
    for j in &junctions {
        let kept: Vec<usize> = j.members.iter().copied().filter(|&f| part_of_final[f] != usize::MAX).collect::<HashSet<_>>().into_iter().collect();
        let Some(&host_f) = kept.iter().max_by(|a, b| peak(&finals[**a]).total_cmp(&peak(&finals[**b])).then(b.cmp(a))) else { continue };
        for &f in &kept {
            if f == host_f {
                continue;
            }
            let (pi, hi_) = (part_of_final[f], part_of_final[host_f]);
            if !seen.insert((pi, hi_)) {
                continue;
            }
            let rounds = &parts[hi_].rounds;
            let host_rounds = between(rounds, j.z);
            parts[pi].attach.push(Attach { host: hi_, host_rounds, z: j.z });
        }
    }
    name_parts(&mut parts);
    Ok(Figure { parts, auto_splits, notes })
}

/// The two neighbouring rounds (lower number first) whose heights bracket `z`.
fn between(rounds: &[Round], z: f64) -> (usize, usize) {
    let Some(near) = rounds.iter().min_by(|a, b| (a.z - z).abs().total_cmp(&(b.z - z).abs())) else { return (1, 1) };
    let pos = near.index - 1;
    let other = if z > near.z {
        rounds.get(pos + 1).filter(|r| r.z > near.z).or_else(|| pos.checked_sub(1).and_then(|p| rounds.get(p)).filter(|r| r.z > near.z))
    } else {
        rounds.get(pos + 1).filter(|r| r.z < near.z).or_else(|| pos.checked_sub(1).and_then(|p| rounds.get(p)).filter(|r| r.z < near.z))
    };
    match other {
        Some(o) => (near.index.min(o.index), near.index.max(o.index)),
        None => (near.index, near.index),
    }
}

fn name_parts(parts: &mut [Part]) {
    let Some(main) = (0..parts.len()).max_by(|a, b| parts[*a].peak_perimeter.total_cmp(&parts[*b].peak_perimeter).then(b.cmp(a))) else { return };
    let (mut zlo, mut zhi, mut rmax, mut cx) = (f64::MAX, f64::MIN, 0.0_f64, 0.0);
    for r in &parts[main].rounds {
        zlo = zlo.min(r.z);
        zhi = zhi.max(r.z);
        rmax = rmax.max(r.radius);
        cx += r.center[0] / parts[main].rounds.len() as f64;
    }
    let body_h = (zhi - zlo).max(1e-9);
    let mut names: Vec<String> = vec![String::new(); parts.len()];
    names[main] = "Body".to_string();
    let mut head_taken = false;
    let mut order: Vec<usize> = (0..parts.len()).filter(|&i| i != main).collect();
    order.sort_by(|a, b| parts[*b].peak_perimeter.total_cmp(&parts[*a].peak_perimeter));
    for i in order {
        let rs = &parts[i].rounds;
        let zmid = rs.iter().map(|r| r.z).sum::<f64>() / rs.len() as f64;
        let x = rs.iter().map(|r| r.center[0]).sum::<f64>() / rs.len() as f64 - cx;
        let on_body = parts[i].attach.iter().all(|a| a.host == main);
        let side = if x.abs() < 0.15 * rmax.max(1e-9) {
            ""
        } else if x < 0.0 {
            "Left "
        } else {
            "Right "
        };
        let above = zmid > zhi - 0.3 * body_h;
        let below = zmid < zlo + 0.3 * body_h;
        let kind = if above && x.abs() <= 0.35 * rmax && on_body && !head_taken {
            head_taken = true;
            "Head"
        } else if above && !on_body {
            "ear"
        } else if above && x.abs() <= 0.35 * rmax {
            "top piece"
        } else if above {
            "arm"
        } else if below {
            "leg"
        } else {
            "arm"
        };
        names[i] = if kind == "Head" { "Head".to_string() } else { format!("{side}{kind}").trim().to_string() };
        if let Some(c) = names[i].chars().next() {
            names[i] = c.to_uppercase().collect::<String>() + &names[i][c.len_utf8()..];
        }
    }
    let mut count: HashMap<String, usize> = HashMap::new();
    for (i, n) in names.iter().enumerate() {
        let k = count.entry(n.clone()).or_default();
        *k += 1;
        parts[i].name = if *k > 1 { format!("{n} {k}") } else { n.clone() };
    }
}

impl Figure {
    /// Assembly notes: which part is sewn to which, and between which rounds.
    pub fn assembly_notes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for p in &self.parts {
            for a in &p.attach {
                let host_part = self.parts.get(a.host);
                let host = host_part.map_or("body", |h| h.name.as_str());
                let at_open_end =
                    host_part.is_some_and(|h| !h.closed_last && a.host_rounds.0 == a.host_rounds.1 && a.host_rounds.0 == h.rounds.len());
                let place = if at_open_end {
                    format!("(after Rnd {})", a.host_rounds.0)
                } else if a.host_rounds.0 == a.host_rounds.1 {
                    format!("at Rnd {}", a.host_rounds.0)
                } else {
                    format!("between Rnd {} and Rnd {}", a.host_rounds.0, a.host_rounds.1)
                };
                let target = if at_open_end { "open end of the" } else { "" };
                out.push(
                    format!("Sew the open end of the {} to the {target} {} {place}.", p.name.to_lowercase(), host.to_lowercase()).replace("  ", " "),
                );
            }
        }
        out
    }

    /// The written pattern of every part. `eyes` is a marked point on the model; the part with the nearest round gets
    /// the safety-eye note.
    pub fn patterns(&self, terms: Terms, weight: YarnWeight, gauge: &Gauge, eyes: Option<[f64; 3]>) -> Vec<PartPattern> {
        let stitch = Stitch::from_gauge(gauge).ok();
        let mark: Option<(usize, EyeMark)> = match (eyes, stitch) {
            (Some(p), Some(st)) => self
                .parts
                .iter()
                .enumerate()
                .filter_map(|(i, part)| eyes_at(&part.rounds, p, st).map(|m| (i, m, part)))
                .min_by(|a, b| dist_to_rounds(&a.2.rounds, a.1.round, p).total_cmp(&dist_to_rounds(&b.2.rounds, b.1.round, p)))
                .map(|(i, m, _)| (i, m)),
            _ => None,
        };
        self.parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let e = mark.as_ref().filter(|m| m.0 == i).map(|m| &m.1);
                part_pattern(&p.name, &p.rounds, p.closed_last, terms, weight, e)
            })
            .collect()
    }
}

fn dist_to_rounds(rounds: &[Round], index: usize, p: [f64; 3]) -> f64 {
    rounds.iter().find(|r| r.index == index).map_or(f64::MAX, |r| {
        let rho = ((p[0] - r.center[0]).powi(2) + (p[1] - r.center[1]).powi(2)).sqrt();
        (rho - r.radius).powi(2) + (p[2] - r.z).powi(2)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gauge::Unit;
    use crate::models;

    fn gauge() -> Gauge {
        Gauge::new(20.0, 20.0, 4.0, Unit::Inches).unwrap()
    }

    fn fit(m: &Mesh, h: f64) -> Mesh {
        m.fitted(h, Unit::Inches, None).unwrap()
    }

    #[test]
    fn a_ball_is_one_part() {
        let f = split_figure(&models::sphere(1.0), &gauge(), &ManualSplits::default()).unwrap();
        assert_eq!(f.parts.len(), 1);
        assert_eq!(f.parts[0].name, "Body");
        assert!(f.parts[0].closed_last && f.parts[0].attach.is_empty());
        assert!(f.assembly_notes().is_empty());
    }

    #[test]
    fn the_snowman_splits_into_two_parts_at_the_neck() {
        let m = fit(&models::snowman(), 6.0);
        let f = split_figure(&m, &gauge(), &ManualSplits::default()).unwrap();
        assert_eq!(f.parts.len(), 2, "{:?}", f.parts.iter().map(|p| &p.name).collect::<Vec<_>>());
        assert_eq!(f.auto_splits.len(), 1);
        let neck_z = f.auto_splits[0];
        // Neck: where the body ball ends (about 1.9/3.15 of the height).
        assert!((neck_z / 6.0 - 1.94 / 3.15).abs() < 0.05, "{neck_z}");
        let body = f.parts.iter().find(|p| p.name == "Body").unwrap();
        let head = f.parts.iter().find(|p| p.name == "Head").unwrap();
        assert!(!body.from_top && !body.closed_last, "the body starts at its closed bottom and ends open at the neck");
        assert!(head.from_top && !head.closed_last, "the head starts at its crown and ends open at the neck");
        assert_eq!(head.attach.len(), 1);
        let notes = f.assembly_notes();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].starts_with("Sew the open end of the head to the open end of the body") && notes[0].contains("Rnd "), "{notes:?}");
    }

    #[test]
    fn manual_split_planes_can_remove_and_add() {
        let m = fit(&models::snowman(), 6.0);
        let none = split_figure(&m, &gauge(), &ManualSplits { add: vec![], remove: vec![0] }).unwrap();
        assert_eq!(none.parts.len(), 1);
        let moved = split_figure(&m, &gauge(), &ManualSplits { add: vec![4.5], remove: vec![0] }).unwrap();
        assert_eq!(moved.parts.len(), 2);
        let extra = split_figure(&m, &gauge(), &ManualSplits { add: vec![1.0], remove: vec![] }).unwrap();
        assert_eq!(extra.parts.len(), 3);
        for bad in [f64::NAN, -5.0, 1e9] {
            assert!(split_figure(&m, &gauge(), &ManualSplits { add: vec![bad], remove: vec![] }).is_ok());
        }
    }

    #[test]
    fn a_body_with_four_limbs_splits_into_body_and_four_limbs() {
        let m = fit(&models::limb_figure(), 8.0);
        let f = split_figure(&m, &gauge(), &ManualSplits::default()).unwrap();
        let names: Vec<&str> = f.parts.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(f.parts.len(), 5, "{names:?}");
        assert_eq!(names.iter().filter(|n| n.contains("leg")).count(), 2, "{names:?}");
        assert_eq!(names.iter().filter(|n| n.contains("arm")).count(), 2, "{names:?}");
        assert!(names.contains(&"Body"));
        let body = f.parts.iter().position(|p| p.name == "Body").unwrap();
        for (i, p) in f.parts.iter().enumerate() {
            if i != body {
                assert!(p.attach.iter().any(|a| a.host == body), "{} is not sewn to the body", p.name);
                assert!(p.rounds.len() >= 3);
            }
        }
        assert_eq!(f.assembly_notes().len(), 4);
        // Limbs start at their closed tips, so the legs go from the bottom and the arms from the top.
        for p in &f.parts {
            if p.name.contains("arm") {
                assert!(p.from_top && !p.closed_last);
            }
            if p.name.contains("leg") {
                assert!(!p.from_top && !p.closed_last);
            }
        }
    }

    #[test]
    fn every_part_is_buildable_and_patterns_exist() {
        let m = fit(&models::limb_figure(), 8.0);
        let f = split_figure(&m, &gauge(), &ManualSplits::default()).unwrap();
        for p in &f.parts {
            assert_eq!(p.rounds[0].stitches, 6);
            for w in p.rounds.windows(2) {
                let (a, b) = (w[0].stitches, w[1].stitches);
                assert!(a.abs_diff(b) <= 6 && (b >= a || 2 * (a - b) <= a));
            }
        }
        let pats = f.patterns(Terms::Us, YarnWeight::Worsted, &gauge(), Some([0.0, -0.5, 6.0]));
        assert_eq!(pats.len(), 5);
        assert!(pats.iter().all(|p| p.rounds[0].text.contains("magic ring") && p.yards > 0.0));
        assert_eq!(pats.iter().filter(|p| p.notes.iter().any(|n| n.contains("Safety eyes"))).count(), 1);
    }

    #[test]
    fn a_capsule_a_cylinder_and_a_cone_stay_whole() {
        for m in [models::capsule(1.0, 2.0), models::cylinder(1.0, 3.0), models::cone(1.0, 2.0)] {
            let f = split_figure(&fit(&m, 5.0), &gauge(), &ManualSplits::default()).unwrap();
            assert_eq!(f.parts.len(), 1);
        }
    }
}
