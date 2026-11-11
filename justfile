
## Build/lint commands

check:
  cargo check --workspace

clippy:
  cargo clippy --workspace

fmt:
  cargo fmt --all

## WPT test runner

wpt *ARGS:
  cargo run --release --package wpt {{ARGS}}

