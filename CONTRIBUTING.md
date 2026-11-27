# Contributing to Pageframe

I maintain Pageframe on my own. Before writing a pull request, [open an issue](https://github.com/RealWhyKnot/Pageframe/issues/new) and describe the bug or what you're trying to build. For a bigger change, let's agree on the approach before you spend time on the code.

Pageframe is early and changes faster than the docs. If the docs and the code disagree, go by the code.

## Bugs and security

For a rendering bug, include the smallest page that shows it, what Chrome draws, and what Pageframe draws. For a crash, paste the panic message and backtrace exactly as they appeared.

Use the private advisory form in the [security policy](.github/SECURITY.md) for security reports.

## Building

You need Rust 1.99 or newer from [rustup](https://rustup.rs), Python 3 for Stylo's build script, and a C toolchain. Rustup reads `rust-toolchain.toml` and selects the version Pageframe tests with.

- Windows: install the Visual Studio C++ build tools and any Python 3.
- macOS: Python comes with the Xcode Command Line Tools, which also provide the C toolchain.
- Linux: install `build-essential`, `pkg-config`, `python3` and `libfontconfig-dev`. To open windows you also need X11 or Wayland libraries, for example `libx11-dev`, `libxcb1-dev` and `libwayland-dev` on Ubuntu.

The file dialog in `pageframe-shell` uses `rfd` with its `xdg-portal` backend on Linux. A dialog only opens if `xdg-desktop-portal` and a backend for your desktop are running, or if `zenity` is installed.

Audio and video playback uses Windows Media Foundation. The playback tests run on Windows. Media elements can load on other platforms, where playback is unavailable.

## Checks

Turn on the Git hooks after cloning.

```sh
git config core.hooksPath .githooks
```

The `commit-msg` hook checks the subject format described under [Commit messages](#commit-messages).

The `pre-push` hook runs the following checks on Windows, macOS and Linux.

- `cargo fmt --check`
- `cargo clippy --workspace --all-targets --locked -- -D warnings`
- `cargo test --workspace --all-targets --locked`
- the style, shell, PowerShell, Prettier and Stylelint checks in `.github/lint`

The lint checks need Node (`npm ci` runs on first use), PowerShell 7, `shellcheck`, `shfmt`, `typos`, `gitleaks`, `zizmor` and `cargo-deny` on your `PATH`.

## If we agree on a PR

Contributions are licensed under MIT OR Apache-2.0, with edits to vendored MPL-2.0 files licensed under MPL-2.0.

- Link the issue where we settled the approach.
- Keep the diff small. A "while I'm here" cleanup is its own PR.
- Follow the surrounding code and `rustfmt.toml`. Don't add code comments unless we agreed on one.
- If behavior changed, add or update a test. Rendering changes need a test that compares against Chrome's output.

## Commit messages

Subjects use [Conventional Commits](https://www.conventionalcommits.org): `type(scope?): short summary`.

- `type` is one of `feat`, `fix`, `chore`, `ci`, `docs`, `refactor`, `revert`, `test`, `perf`, `diag` or `style`. The `commit-msg` hook checks this.
- If the change breaks the public API or changes behavior people rely on, mark it: put `!` after the type or scope (`feat(dom)!: drop Document::old_call`), or add a `BREAKING CHANGE:` footer that says what broke and how to update. The nightly beta and the changelog both read these markers.
- Use the body to explain why you made the change.

## Releases

Versions follow [Semantic Versioning](https://semver.org), as described in the [README](README.md#versioning). Pageframe crates inherit their version from `[workspace.package]` in the root `Cargo.toml`.

- A nightly job tags a prerelease like `v0.2.0-beta.20261124.0` when `main` has shippable changes. It picks the version from the commit markers above.
- A stable release is tagged `vX.Y.Z`. Bump `Cargo.toml` to `X.Y.Z` first. The release workflow fails if the tag and `Cargo.toml` disagree.
