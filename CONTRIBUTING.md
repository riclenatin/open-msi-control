# Contributing

Thanks for helping! Two kinds of contribution are especially valuable: **support for more laptop
models** and **UI polish**.

## Getting set up

1. Windows 10/11 x64, an MSI laptop for anything that touches hardware.
2. Rust stable (pinned via `rust-toolchain.toml`), Visual Studio Build Tools with the
   "Desktop development with C++" workload, Windows SDK (for `fxc.exe`).
3. `cargo run` builds and starts the app. Windows asks for elevation because `MSI_ACPI` is
   restricted to Administrators.

Without MSI hardware you can still work on the library (`cargo test -p msi-acpi` uses an
emulated EC) and on theme/motion code.

## Reporting a new model

Open a *Support a new laptop model* issue with the full output of `msi-probe.exe` (run from an
elevated terminal; it only reads). Mention which features you tried and what happened.

## Making changes

- Keep commits focused; write messages in the imperative (`Add GPU fan curve labels`).
- Before pushing:

  ```sh
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

- **EC registers:** add them to `crates/msi-acpi/src/ec.rs`, document them in
  `docs/PROTOCOL.md`, write through read-modify-write helpers, and cover the encoding with a
  test in `crates/msi-acpi/tests/laptop.rs`. Say in the PR on which model and firmware you
  verified the write and how you read it back.
- **UI:** use theme tokens and existing components; check contrast tests still pass; make new
  controls keyboard-reachable. See the UI conventions in [`AGENTS.md`](AGENTS.md).
- Update `CHANGELOG.md` for anything a user would notice.

## Releasing

1. Set `version` in `[workspace.package]` of `Cargo.toml` (then `cargo check` to update
   `Cargo.lock`) and move the `[Unreleased]` entries of `CHANGELOG.md` under
   `## [x.y.z] - YYYY-MM-DD`.
2. Commit, then tag and push. The tag is the version with a `v` prefix and must match
   `Cargo.toml`:

   ```sh
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. [`release.yml`](.github/workflows/release.yml) runs the tests, builds the release and
   publishes a GitHub Release with `open-msi-control-x.y.z-setup.exe` and
   `open-msi-control-x.y.z-portable.zip`. The notes are the `CHANGELOG.md` section for the version
   followed by GitHub's generated notes.

The in-app update checker looks for these exact asset names; do not rename them.

To build the same artifacts locally, install [Inno Setup 6](https://jrsoftware.org/isdl.php) and
run `packaging/build-release.ps1` from PowerShell; it writes both files to `dist/`. It finds
`ISCC.exe` via `-Iscc <path>`, `$env:ISCC` or the default install folder.

## Licensing

By contributing you agree that your work is licensed under GPL-2.0-or-later, like the
rest of the project. Do not paste code from MSI software or other proprietary sources.
