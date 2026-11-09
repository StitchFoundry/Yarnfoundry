# YarnFoundry

A free, open-source app for knitters and crocheters. Type a word or drop in a picture and get a colorwork chart
(tapestry, intarsia, fair isle, filet) sized to your gauge, with row-by-row written instructions and a chart that
prints at true size.

Status: pre-alpha. See [ROADMAP.md](ROADMAP.md).

```
cargo run --release -p yarnfoundry     # desktop app
cargo xtask ci                         # the quality gate
sh apps/yarnfoundry-web/build.sh       # browser build into apps/yarnfoundry-web/pkg (needs wasm-bindgen-cli)
yarnfoundry --screenshot out.png [--project f.yarnfoundry]   # draw the window, save a PNG, exit (use xvfb-run without a display)
```

The browser version is built and published by the Pages workflow. Draft notes for the first release: [RELEASE-NOTES.md](RELEASE-NOTES.md).

MIT licensed. Agents: see [AGENTS.md](AGENTS.md).
