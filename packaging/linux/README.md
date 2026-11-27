# Linux packages

`build.sh` builds `yarnfoundry` in release mode and writes `dist/yarnfoundry_<version>_amd64.deb`. If
`appimagetool` is on PATH (or `APPIMAGETOOL` names it) it also writes `dist/YarnFoundry-<version>-x86_64.AppImage`.
Both are unsigned. There is no icon asset yet, so the launcher uses the default icon. Wiring this into the
release workflow belongs to issue #47, slice 4.
