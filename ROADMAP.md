# YarnFoundry roadmap

## Honest assessment
Pre-alpha (2026-11-09). A window opens with an empty chart canvas. There is no chart model, no importer and no output yet.
Self-assessed, measured by behaviour: everything below is NOT done.

## Gap list (ranked; each is a GitHub issue)
1. YF-1 Chart model, gauge math, `.yarnfoundry` file format
2. YF-2 Picture to chart
3. YF-3 Words to chart
4. YF-4 Technique rules (tapestry, intarsia, fair isle, filet)
5. YF-5 Written row-by-row instructions
6. YF-6 Chart view and printable chart (PDF/SVG/PNG)
7. YF-7 Guided 5-step setup and generic yarn palettes
8. YF-8 Web build on GitHub Pages, `--screenshot`, v0.1.0 notes

## Shipped so far
- 2026-11-09 YF-4: technique rules (core).
- 2026-11-09 YF-3: words to chart.
- 2026-11-09 YF-2: picture to chart.
- 2026-11-09 YF-1: chart model, gauge math, file format.
- 2026-11-09 YF-0: Cargo workspace, `cargo xtask ci` gate, CI workflow, agent files, empty-canvas app window.

## Progress log
- 2026-11-09 YF-4 technique rules in core: fair isle (max 2 colours a row, floats over the limit flagged with catch points), intarsia (bobbins per row, small areas merged), tapestry (carried colours per row, warn over 3), filet (filled/open by threshold, 3 x squares + 1 stitches). Checked by unit and property tests and `cargo xtask ci`. Not done: choosing rules in the UI, in-the-round float wrap-around, showing floats on the chart.
- 2026-11-09 YF-3 words to chart: one bundled OFL font (Liberation Sans Bold) via ab_glyph, stitch-aspect-correct scaling, bold, outline, letter spacing, centring with margins, `yarnfoundry-cli text`. Checked by a golden picture of KING CASHEW (readable at 18 rows), option/error tests, proptest, `cargo xtask ci`. Not done: choice of fonts, kerning beyond the font's own, multi-line text.
- 2026-11-09 YF-2 picture to chart: PNG/JPEG/WebP decode (size-capped), aspect-correct area-averaged resample, CIELAB nearest-colour or deterministic k-means palette, lonely-stitch cleanup, `yarnfoundry-cli picture`, chart drawn in the app. Checked by unit/golden/property tests (20x10 exact, N colours exact, byte-identical, 12 MP in the time limit) and `cargo xtask ci`. Not done: dithering, technique rules, instructions.
- 2026-11-09 YF-1 chart model, gauge math, `.yarnfoundry` JSON file (versioned, validated, size-capped). Checked by 19 core tests incl. property tests on hostile files, `cargo xtask ci`. Not done: any importer or output.
- 2026-11-09 YF-0 scaffold. Checked by `cargo xtask ci` locally and in CI. Not done: any chart feature.
