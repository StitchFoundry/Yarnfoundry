# YarnFoundry 0.1.0 (draft release notes)

DRAFT: this version is not tagged or published. Bruce decides when.

YarnFoundry is a free, open-source app for knitters and crocheters. You tell it what you want to make, and it gives you
a colorwork chart, written instructions row by row, and a chart you can print at its real size. No account, no sign-in,
nothing leaves your computer.

## What you can do

- **Follow five friendly steps.** 1) Knit or crochet, the colorwork technique, flat or in the round. 2) Type some words
  or choose a picture. 3) Say how big it should be, and give your gauge, or just say what yarn weight you use if you
  don't know it. 4) Pick 2 to 8 colours from a built-in palette, or let the app pick them from your picture.
  5) Look at the chart, print it, save it.
- **Words to chart.** A name or a saying, in bold letters at least 20 rows tall, so it reads clearly on a scarf or blanket.
- **Picture to chart.** Your picture is fitted to the stitch grid in your gauge (stitches are not square, and the chart knows).
- **Four techniques.** Intarsia and fair isle for knitting; tapestry and filet for crochet. The app tells you when a
  row needs more colours than the technique handles well, or a float is long.
- **Written instructions.** One line per row, which side to start from, stitch counts, and a total per colour.
- **A printable chart at true size.** Letter or A4, in colour or in black and white with a different symbol for each
  colour. Big charts are split over several pages with a page map. A 1-inch bar on each page lets you check your printer.
- **Saves your work** as a `.yarnfoundry` file you can open again.
- **Use it in the browser** (when the project's web page is switched on) or as a desktop app.

## What it does not do yet

- No 3D models or amigurumi patterns, no yardage estimate, no pattern booklet. Those are for later.
- Colour names are plain words (red, dark blue); there are no yarn-brand colour cards.
- The PDF uses standard fonts, so titles with accents or non-Latin letters show a question mark.
- In the browser the printed chart and project are downloaded as files; opening a saved project in the browser is not built yet.
- Very large charts (more than a few hundred pages at true size) are refused with a message.
- It has been tested by its own automatic tests, not yet by many knitters. Please swatch and check the 1-inch bar
  before trusting a printed size, and tell us what is wrong.

## Licence

MIT today. A change to "MIT or Apache-2.0" is proposed in issue #10 and is Bruce's call.

## For testers

Desktop: `cargo run --release -p yarnfoundry`. Command line: `yarnfoundry-cli text "KING CASHEW" scarf.yarnfoundry --width 180`,
then `yarnfoundry-cli print scarf.yarnfoundry scarf.pdf --paper a4`.
