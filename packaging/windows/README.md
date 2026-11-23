# Windows installer

`build.ps1` builds `yarnfoundry.exe` in release mode and wraps it with Inno Setup 6 (`yarnfoundry.iss`) into
`dist\YarnFoundry-<version>-windows-setup.exe`. The output is unsigned. Wiring this into CI and signing it
belong to the release workflow (issue #47, slice 4).
