# Contributing to Pageframe

Pageframe renders HTML and CSS natively for Rust desktop apps. It's a fork of [Blitz](https://github.com/DioxusLabs/blitz).

## Dependency licenses

CI checks dependency licenses with `cargo deny --workspace --all-features --locked
check licenses`. See [the licensing review](docs/licensing.md) for the allowlist
policy and commercial-distribution obligations, including MPL-2.0 dependencies.

## Prerequisites

All platforms require Rust, Python 3 and a C toolchain.

Rust can be installed via https://rustup.rs

### macOS

Python is pre-installed. We recommend using the C toolchaing from the Xcode Command Line tools.

### Windows

Usually requires installing Python. We recommend installing using https://docs.astral.sh/uv/, but any Python 3 should work.

### Linux

To **build** Pageframe you need a C toolchain plus a few dependencies:

- `pkg-config` and a C compiler (`build-essential`)
- `python3` — used to generate code while building `stylo`
- `libfontconfig-dev` (fontconfig) — required by `parley`/`fontique` for system-font enumeration

For example on Ubuntu:

```sh
sudo apt-get update
sudo apt-get install build-essential pkg-config python3 libfontconfig-dev
```

To **run** Pageframe as windowed application you additionally need X11 and/or Wayland. These are usually already
present on a desktop Linux install, but may need to be installed in server setups or if using a system like
Nix that requires explicit dependencies. needed there.

For example on Ubuntu:

```sh
sudo apt-get update
sudo apt-get install libfontconfig-dev libx11-dev libxcb1-dev libwayland-dev
```

The optional **file dialog feature** uses [`rfd`](https://crates.io/crates/rfd)
with its `xdg-portal` backend. So one of the following must be available for
a dialog to actually open:

- a running `xdg-desktop-portal` service **plus** a backend for your desktop
  environment (e.g. `xdg-desktop-portal-gtk`, `-kde`, `-gnome`, or `-wlr`)
- the `zenity` executable, which `rfd` falls back to.

### VSCode

You can add the following JSON to your `.vscode/settings.json` to automically build Pageframe on all supported targets.

```json
{
  "rust-analyzer.check.features": "all",
  "rust-analyzer.cargo.features": "all",
  "rust-analyzer.check.allTargets": true,
  "rust-analyzer.cargo.allTargets": true
}
```
