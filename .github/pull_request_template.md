## What and why

<!-- One paragraph. Link the issue. -->

## How it was verified

<!-- Hardware writes must be tested on a real laptop: model, EC firmware, what you clicked, what you read back. -->

- [ ] `cargo fmt --all --check`, `cargo clippy --workspace --all-targets`, `cargo test --workspace` pass
- [ ] Touches EC registers → `docs/PROTOCOL.md` updated and verified on hardware
- [ ] User-visible change → `CHANGELOG.md` updated
