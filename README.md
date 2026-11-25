# Pageframe

Pageframe renders HTML and CSS natively for Rust desktop apps. Stylo styles the page, Taffy and Parley lay it out, and Vello draws it. You can drive the page from Rust, or load a page that runs its own JavaScript.

Pageframe started as a fork of [Blitz](https://github.com/DioxusLabs/blitz) at commit [b6bb45c](https://github.com/DioxusLabs/blitz/commit/b6bb45c550a9490afa6c43468c77535c6d76856d). The history before that commit is in the Blitz repository.

```rust
use pageframe::{App, Error, EventKind};

const PAGE: &str = r#"
<button id="add">Add one</button>
<span id="count">0</span>
"#;

fn main() -> Result<(), Error> {
    App::new(0_u32, PAGE)
        .stylesheet("button { padding: 0.5rem 1rem; }")
        .on("#add", EventKind::Click, |count, ui, _| {
            *count += 1;
            ui.select("#count").set_text(&count.to_string());
        })
        .run()
}
```

Run it with `cargo run -p pageframe --example counter`. `cargo run -p pageframe --example todo` opens a bigger example, the to-do list in `crates/pageframe/examples/todo`.

## What it does

- HTML and CSS: Stylo handles selectors, the cascade and computed styles, and Taffy and Parley handle layout and text.
- JavaScript and the DOM through the [Boa](https://github.com/boa-dev/boa) engine, including ES modules.
- `fetch()` and `EventSource` over plain HTTP. The built-in client doesn't speak HTTPS, so an `https:` URL fails with a clear error. To load HTTPS pages, pass your own `NetProvider`.
- Cookies, and an `ipc` object that lets page scripts call into your Rust code.
- Canvas 2D and `OffscreenCanvas`, modal `<dialog>`, form controls, and clipboard access through the host.
- Audio, video and Web Audio through Windows Media Foundation. On other platforms, media elements load but don't play.
- An opt-in Chrome DevTools server for inspecting a running page.

Pages loaded over the network can't read local `file:` URLs. Pages loaded from disk, and pages built into an `App`, can.

## Two ways to use it

`App` is for apps written in Rust. Your Rust handlers react to events and edit the DOM, and the page runs no scripts.

- `App::new(state, html)` takes your starting state and the page. `App::from_file(state, path)` reads the page from disk and resolves relative links from its folder.
- `.stylesheet(css)` adds CSS after the page's own styles.
- `.asset(path, bytes)` serves a file the page links to, such as `app.css` or an image, from memory. Pair it with `include_bytes!` to build everything into the binary.
- `.on(selector, kind, handler)` calls `handler(&mut state, &mut ui, &mut event)` when an event of that kind reaches an element matching `selector`. Events bubble, so clicking a `<span>` inside `#add` still runs the `#add` handler.
- `.on_start(f)` runs once after the page loads. `.title(..)` and `.size(..)` set up the window, and `.run()` opens it.

In a handler, `ui.select(selector)` returns the matching elements. Read them with `text`, `value`, `attr`, `checked` and `has_class`, or change them with `set_text`, `set_html`, `set_attr`, `set_value`, `add_class`, `set_style`, `append` and `remove`. `ui.instantiate("#row-template")` clones a `<template>`'s contents, and `find`, `closest`, `parent` and `children` walk the tree. `ui.select("#chart").draw(|canvas| ..)` draws on a `<canvas>` from Rust.

`pageframe::host` is for pages that run their own JavaScript. `HostSession` loads a URL or an HTML string, runs its scripts, and takes input events from your window loop. `PageframeHost` wraps a session around a window and draws it.

- `on_ipc` answers `ipc.invoke(channel, payload)` calls from the page, and `emit` sends events that the page hears with `ipc.on(channel, listener)`.
- `on_navigation` decides which links the page may follow. Without it, a session only follows links on the same origin.
- `take_events` reports title, cursor, clipboard and navigation changes for your window to act on.

The IPC handler answers whichever page is loaded. If you let a session navigate to a remote site, that site's scripts can call your handlers too, so check `session.url()` before acting on a request.

## Features

`gpu`, `svg` and `accessibility` are on by default.

| Feature         | What it does                                                                                      |
| --------------- | ------------------------------------------------------------------------------------------------- |
| `gpu`           | Draws windows with Vello on the GPU.                                                              |
| `svg`           | Renders inline `<svg>` and SVG images.                                                            |
| `accessibility` | Exposes the page to screen readers through AccessKit.                                             |
| `software`      | Draws windows on the CPU. It's used when `gpu` is off, so set `default-features = false` with it. |

With neither `gpu` nor `software` there's no `.run()`, but `app.page(..)` still builds the page.

DevTools is a feature of `pageframe-shell`. Turn on `devtools` to start a Chrome DevTools Protocol server on localhost, and `embedded-devtools-frontend` to serve the DevTools page from that server, so any Chromium browser can open it.

## Testing your app

`app.page(width, height)` builds the page without opening a window. Wrap it in `pageframe-test-harness` to click, type and check layout:

```rust
let mut harness = Harness::wrap(app.page(400, 300).expect("selectors are valid"));
harness.click("#add");
assert_eq!(harness.text_content("#count"), "1");
```

## Crates

| Crate                    | What it holds                                |
| ------------------------ | -------------------------------------------- |
| `pageframe`              | The `App` and `host` APIs that apps use.     |
| `pageframe-dom`          | The DOM, styling and layout.                 |
| `pageframe-html`         | The HTML parser.                             |
| `pageframe-paint`        | Paints a document through anyrender.         |
| `pageframe-script`       | JavaScript, `fetch`, IPC and media, on Boa.  |
| `pageframe-shell`        | The winit window shell.                      |
| `pageframe-devtools`     | The Chrome DevTools Protocol server.         |
| `pageframe-traits`       | Types shared between the crates.             |
| `pageframe-test-harness` | Headless clicking, typing and layout checks. |
| `stylo_taffy`            | Converts Stylo styles to Taffy styles.       |

## Adding it to your project

The crates aren't published to crates.io. Depend on the repository, and add a `[patch]` section so Cargo uses the same vello/glifo, Parley and anyrender copies the workspace does:

```toml
[dependencies]
pageframe = { git = "https://github.com/RealWhyKnot/Pageframe" }
```

Copy the `[patch.crates-io]` and `[patch."https://github.com/DioxusLabs/anyrender"]` sections from this repository's `Cargo.toml`, pointing each `path` at a checkout of Pageframe.

You need Rust 1.99 or newer and Python 3, which Stylo's build script uses. On Linux you also need `pkg-config` and `libfontconfig-dev`. [CONTRIBUTING.md](CONTRIBUTING.md) has the full list.

## Status

Pageframe is early, and the API will change. The version number tells you when.

## Versioning

Pageframe follows [Semantic Versioning](https://semver.org) (`MAJOR.MINOR.PATCH`), starting at `0.1.0`.

Before `1.0.0`, a minor bump (`0.1.0` to `0.2.0`) means something in the API or its behavior broke, and a patch bump (`0.1.0` to `0.1.1`) is safe to take without changes on your side.

From `1.0.0` on, a major bump breaks things, a minor bump adds features that keep working with your code, and a patch bump fixes bugs.

Nightly prereleases are tagged like `v0.2.0-beta.20261124.0`: the version the next release will get, the date in `YYYYMMDD` form, and a counter for that day. The part before `-beta` shows whether the next release breaks anything. If a commit since the last release is marked as breaking, with a `!` after its type (`feat!:`) or a `BREAKING CHANGE:` footer, it jumps to the next breaking version.

## Links

- [CONTRIBUTING.md](CONTRIBUTING.md) explains how to build, test and report bugs.
- [.github/SECURITY.md](.github/SECURITY.md) explains how to report a security problem.
- [CHANGELOG.md](CHANGELOG.md) lists changes by release.
- [docs/licensing.md](docs/licensing.md) covers dependency licenses.

## License

MIT or Apache-2.0, at your option ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). The vendored Stylo in `vendor/stylo` and some dependencies are MPL-2.0. [docs/licensing.md](docs/licensing.md) explains what that means for an app you ship, and [NOTICE](NOTICE) credits the Blitz authors and the other code included here.
