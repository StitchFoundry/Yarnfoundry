# YarnFoundry: instructions for agents

YarnFoundry is a free, open-source app for knitters and crocheters: type a word or drop in a picture and get a
colorwork chart (tapestry, intarsia, fair isle, filet) sized to your gauge, with row-by-row written instructions and
a true-size printable chart. Rust, egui, native desktop plus browser (wasm). Product family: StitchFoundry,
LaserFoundry, PatternFoundry, MakerFoundry (shared standards, never shared code).
Spec: `bwattendorf/Crew-Chief` `plan/yarnfoundry.md` (MVP spec, algorithm sketch, acceptance tests).

## Session start
1. `ROADMAP.md` (honest assessment, gap list) 2. `plan/STATUS.md` 3. the task (GitHub issue; with none given, the oldest open
`claude-queue` issue) 4. only the docs that task needs.

## The loop
Orient, plan ONE task (one issue = one branch `claude/issue-N` = one PR), implement with tests, run the gate, open a PR
"Fixes #N" (commit messages start with the task ID, e.g. `YF-1`), wait for CI, fix up to 3 times. Never skip or delete
a failing test to get green. Pull main and re-run the gate after a merge. Merge only per the repo rules below.

## Quality gate
`cargo xtask ci` (fmt, clippy -D warnings, tests, layers, cleanroom, assets, wasm). Run it before every push. CI runs the same command.

## Non-negotiables
- **Clean room.** Never read, run or copy code from CrochetPARADE (GPLv3) or any competitor. No-read list: CrochetPARADE
  (crochetparade.org, codeberg.org/crochetparade), crochet3d.com, weavelcrochet.com, amigurumi-designer.vercel.app, plushify.net,
  and any GPL/AGPL/LGPL knitting or crochet tool. Features come from public descriptions and published papers (cited by URL).
  Competitor names do not appear in code, UI text, fixtures or marketing; never call YarnFoundry a clone or alternative.
- **Original designs only.** No licensed characters, no yarn brand or colour-card names (generic colour names only).
- **Licenses.** MIT OR Apache-2.0 (dual, from 2026-11-09). Dependencies must be MIT or Apache-2.0 (or compatible permissive). Never change `LICENSE-MIT` or `LICENSE-APACHE`. A non-permissive dependency or any CI workflow change after YF-0 gets the `needs-review` label and is not self-merged.
- **Assets.** Anything file-based (fonts, images) must be original or openly licensed with a row in `ASSETS.md` (`cargo xtask assets` enforces it).
- **Never crash.** No `unwrap`/`expect`/`panic!` in shipped code (clippy denies them; tests may). Files and pasted data are untrusted: cap sizes, use `.get()`, check floats are finite, never divide by zero. Every parser ships with hostile-input tests.
- **Layering** (`cargo xtask layers`): `yarnfoundry-core` has no UI, no I/O, no workspace deps; nothing below `ui-egui` uses egui.
- **Never break wasm** (`cargo xtask wasm`). New crates must be classified in `xtask/src/wasm.rs`.
- **Rust only.** No handwritten JS/TS.
- **Never** push to `main`, force-push, tag or publish a release, or change repo settings or secrets.

## Build story
At every visible milestone render a screenshot headlessly, commit it as `docs/build-story/<UTC yyyymmdd-hhmm>-<issue>.png`, and add a line to `docs/build-story/LOG.md` (UTC time, milestone, PR link, path). Note PR open and merge times there too. See `scripts/screenshot.sh`.

## Project map
- `crates/core` chart model, gauge, palette, techniques, instructions, file format (no UI, no I/O)
- `crates/render` chart to SVG/PNG/PDF
- `crates/ui-egui` thin egui front end (`theme` holds colour tokens)
- `apps/yarnfoundry` desktop app; `apps/yarnfoundry-cli` command line
- `xtask` workspace tooling (`cargo xtask <cmd>`)
- `plan/STATUS.md` current status; `ROADMAP.md` gaps; `docs/build-story/` making-of screenshots and log

## Pitfalls
- Headless screenshots: `scripts/screenshot.sh` (Xvfb + software GL + ImageMagick `import`).
- Parallel work: use a separate `CARGO_TARGET_DIR`; `target/` grows large.
