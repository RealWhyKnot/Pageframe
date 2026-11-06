
## Build/lint commands

check:
  cargo check --workspace

clippy:
  cargo clippy --workspace

fmt:
  cargo fmt --all

small:
  cargo build --profile small -p counter --no-default-features --features cpu,system-fonts

## WPT test runner

wpt *ARGS:
  cargo run --release --package wpt {{ARGS}}

## Rendering

screenshot *ARGS:
  cargo run --release --example screenshot {{ARGS}}

## 7GUIs

seven_guis *ARGS:
  cargo run --release --package seven_guis --bin seven_guis_native {{ARGS}}

## TodoMVC commands

todomvc *ARGS:
  cargo run --release --package todomvc --bin todomvc_native {{ARGS}}

todoskia *ARGS:
  cargo run --release --package todomvc --bin todomvc_native {{ARGS}} --no-default-features --features skia

todoandroid *ARGS:
  export CARGO_APK_RELEASE_KEYSTORE="$HOME/.android/debug.keystore"
  export CARGO_APK_RELEASE_KEYSTORE_PASSWORD="android"
  cargo apk run --lib --no-default-features --features skia -p todomvc

counterandroid *ARGS:
  export CARGO_APK_RELEASE_KEYSTORE="$HOME/.android/debug.keystore"
  export CARGO_APK_RELEASE_KEYSTORE_PASSWORD="android"
  cargo apk run --lib --no-default-features --features skia -p counter

## WASM

wasm-build APP *ARGS:
  cd examples/{{APP}} && trunk build --release --public-url ./ {{ARGS}}

wasm-serve APP *ARGS:
  cd examples/{{APP}} && trunk serve --release --public-url ./ {{ARGS}}