#!/usr/bin/env bash
# Headless screenshot of the desktop app for the build story.
# usage: scripts/screenshot.sh <issue-label> [seconds-to-wait]   (writes docs/build-story/<UTC>-<label>.png)
set -euo pipefail
label="${1:?issue label, e.g. issue-1}"
wait_s="${2:-6}"
cd "$(dirname "$0")/.."
stamp="$(date -u +%Y%m%d-%H%M)"
out="docs/build-story/${stamp}-${label}.png"
cargo build --release -p yarnfoundry
xvfb-run -a -s "-screen 0 1100x750x24" bash -c "
  LIBGL_ALWAYS_SOFTWARE=1 ./target/release/yarnfoundry \"\$@\" &
  pid=\$!
  sleep ${wait_s}
  import -window root '${out}'
  kill \$pid || true
" _ "${@:3}"
echo "$out"
