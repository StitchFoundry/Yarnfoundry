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
- 2026-11-09 YF-2: picture to chart.
- 2026-11-09 YF-1: chart model, gauge math, file format.
- 2026-11-09 YF-0: Cargo workspace, `cargo xtask ci` gate, CI workflow, agent files, empty-canvas app window.

## Progress log
- 2026-11-09 YF-2 picture to chart: PNG/JPEG/WebP decode (size-capped), aspect-correct area-averaged resample, CIELAB nearest-colour or deterministic k-means palette, lonely-stitch cleanup, `yarnfoundry-cli picture`, chart drawn in the app. Checked by unit/golden/property tests (20x10 exact, N colours exact, byte-identical, 12 MP in the time limit) and `cargo xtask ci`. Not done: dithering, technique rules, instructions.
- 2026-11-09 YF-1 chart model, gauge math, `.yarnfoundry` JSON file (versioned, validated, size-capped). Checked by 19 core tests incl. property tests on hostile files, `cargo xtask ci`. Not done: any importer or output.
- 2026-11-09 YF-0 scaffold. Checked by `cargo xtask ci` locally and in CI. Not done: any chart feature.
