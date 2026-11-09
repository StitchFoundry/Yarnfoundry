# YarnFoundry roadmap

## Honest assessment
Pre-alpha (2026-11-09). The core works headlessly: gauge math, `.yarnfoundry` files, picture to chart, words to chart, technique rules and written instructions, all tested. The app can open a chart and draw it, but there is printable charts (PDF/SVG/PNG) work, with a guided five-step setup, but there is no web build yet.
Self-assessed, measured by behaviour: the gaps below are NOT done.

## Gap list (ranked; each is a GitHub issue)
1. YF-8 Web build on GitHub Pages, `--screenshot`, v0.1.0 notes

## Shipped so far
- 2026-11-09 YF-7: guided five-step setup, generic palettes, yarn-weight gauges.
- 2026-11-09 YF-6: chart view with row numbers and legend, printable chart (PDF/SVG/PNG, true size, Letter and A4, tiling), bold text weight.
- 2026-11-09 YF-5: written instructions (core, CLI).
- 2026-11-09 YF-4: technique rules (core).
- 2026-11-09 YF-3: words to chart.
- 2026-11-09 YF-2: picture to chart.
- 2026-11-09 YF-1: chart model, gauge math, file format.
- 2026-11-09 YF-0: Cargo workspace, `cargo xtask ci` gate, CI workflow, agent files, empty-canvas app window.

## Progress log
- 2026-11-09 YF-7 guided setup: `core::setup` holds the choices and builds the chart (craft/technique pairing, words or picture, size by width or stitches, gauge known or by yarn weight lace to super bulky, colours 2-8 from four generic palettes or from the picture, words widen the chart rather than fail, technique rules applied). The app walks steps 1-5 (Back/Next, a message when something is missing) ending in the preview with legend, written rows, PDF print and Save project; step 2 offers a bundled sample picture so no file is needed. `yarnfoundry --step N` starts at a step with demo words (for screenshots). Checked by 13 setup tests (no brand words, hostile sizes and gauges), 5 UI flow tests, `cargo xtask ci`; screenshots of every step in docs/build-story. Not done: loading pictures in the browser, per-colour yarn names (A, B, C), tweak options (cleanup, dither) in the UI, fonts choice.
- 2026-11-09 YF-6 printable chart: new render crate draws one display list to SVG, PDF (hand-written, standard fonts, no new dependency) and PNG (own rasteriser, `image` already used). Cells are 72/gauge points so a 100% print matches the gauge (tested within 1% for Letter and A4, inch and cm gauges); big charts tile on 10-stitch lines (stitch 1 at the right, row 1 at the bottom) with a cover page holding the legend, counts and a page map; row numbers on the right for RS rows and rounds, left for WS rows; colour, colour with symbols, and black and white (16 drawn shapes, then numbers: 64 distinct symbols). Also: text weight (regular/bold/heavy, default bold), minimum letter height (default 20 rows, now the capital height) so a name on a scarf reads clearly; golden updated. `yarnfoundry-cli print`, app side panel with legend and Save PDF buttons. Checked by 18 render tests, text tests, `cargo xtask ci`. Not done: choosing the technique/gauge in the UI, on-screen zoom and scroll, embedded fonts in the PDF, non-ASCII titles in the PDF (shown as ?).
- 2026-11-09 YF-5 written instructions: run-length rows for knit (k/p) and crochet (sc; filet dc/ch), RS right-to-left, WS left-to-right, rounds right-to-left, stitch counts, per-colour totals, `yarnfoundry-cli instructions`. Checked by example, direction and property tests (every row's counts add up to the width) and `cargo xtask ci`. Not done: shown in the UI, abbreviations key, UK terms, colour-change notes.
- 2026-11-09 YF-4 technique rules in core: fair isle (max 2 colours a row, floats over the limit flagged with catch points), intarsia (bobbins per row, small areas merged), tapestry (carried colours per row, warn over 3), filet (filled/open by threshold, 3 x squares + 1 stitches). Checked by unit and property tests and `cargo xtask ci`. Not done: choosing rules in the UI, in-the-round float wrap-around, showing floats on the chart.
- 2026-11-09 YF-3 words to chart: one bundled OFL font (Liberation Sans Bold) via ab_glyph, stitch-aspect-correct scaling, bold, outline, letter spacing, centring with margins, `yarnfoundry-cli text`. Checked by a golden picture of KING CASHEW (readable at 18 rows), option/error tests, proptest, `cargo xtask ci`. Not done: choice of fonts, kerning beyond the font's own, multi-line text.
- 2026-11-09 YF-2 picture to chart: PNG/JPEG/WebP decode (size-capped), aspect-correct area-averaged resample, CIELAB nearest-colour or deterministic k-means palette, lonely-stitch cleanup, `yarnfoundry-cli picture`, chart drawn in the app. Checked by unit/golden/property tests (20x10 exact, N colours exact, byte-identical, 12 MP in the time limit) and `cargo xtask ci`. Not done: dithering, technique rules, instructions.
- 2026-11-09 YF-1 chart model, gauge math, `.yarnfoundry` JSON file (versioned, validated, size-capped). Checked by 19 core tests incl. property tests on hostile files, `cargo xtask ci`. Not done: any importer or output.
- 2026-11-09 YF-0 scaffold. Checked by `cargo xtask ci` locally and in CI. Not done: any chart feature.
