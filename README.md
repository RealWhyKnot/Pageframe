# Pageframe

Build desktop app UIs in Rust with HTML and CSS. Pageframe styles your page with Stylo, lays it out with Taffy and Parley, and draws it with Vello. Your Rust code handles the events and edits the DOM; there's no browser or JavaScript engine in the app.

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

Run it with `cargo run -p pageframe --example counter`. For something bigger, `cargo run -p pageframe --example todo` opens the to-do list in `crates/pageframe/examples/todo`.

## Building

Rust 1.99 or newer, and Python 3 for Stylo's build script. On Linux you also need `pkg-config` and `libfontconfig-dev`; [CONTRIBUTING.MD](CONTRIBUTING.MD) has the full package list.

## Usage

`App::new(state, html)` takes your starting state and the page. `App::from_file(state, path)` reads the page from disk and resolves relative links from its folder.

- `.stylesheet(css)` adds CSS after the page's own styles.
- `.asset(path, bytes)` serves a file the page links to (`<link href="app.css">`, images) from memory. Pair it with `include_bytes!` to build everything into the binary.
- `.on(selector, kind, handler)` calls `handler(&mut state, &mut ui, &mut event)` when an event of that kind reaches an element matching `selector`. Events bubble, so clicking a `<span>` inside `#add` still runs the `#add` handler.
- `.on_start(f)` runs once after the page loads.
- `.title(..)` and `.size(..)` set up the window, and `.run()` opens it.

In a handler, `ui.select(selector)` returns the matching elements. Read them with `text`, `value`, `attr`, `checked` and `has_class`, or change them with `set_text`, `set_html`, `set_attr`, `set_value`, `add_class`, `set_style`, `append` and `remove`. `ui.instantiate("#row-template")` clones a `<template>`'s contents, and `find`, `closest`, `parent` and `children` walk the tree.

`event.target()` is the element that was hit and `event.current_target()` the one your selector matched. `value()` has the text for input events and `key()` the key for keyboard events. `prevent_default()` and `stop_propagation()` work like they do in a browser.

## Features

`gpu`, `svg` and `accessibility` are on by default.

- `gpu` draws windows with Vello on the GPU.
- `svg` renders inline `<svg>` and SVG images.
- `accessibility` exposes the page to screen readers through AccessKit.
- `software` draws windows on the CPU. It's used when `gpu` is off, so set `default-features = false` with it.

With neither `gpu` nor `software` there's no `.run()`. `app.page(..)` still builds the page.

## Testing

`app.page(width, height)` builds the page without opening a window. Wrap it in `pageframe-test-harness` to click, type and check layout:

```rust
let mut harness = Harness::wrap(app.page(400, 300).expect("selectors are valid"));
harness.click("#add");
assert_eq!(harness.text_content("#count"), "1");
```

`cargo test -p pageframe` runs the crate's tests, including pixel checks rendered on the CPU.

## Status

Early, and the API will change. CSS support comes from Blitz. Flexbox, CSS variables, rounded corners, box and text shadows, gradients, `mix-blend-mode`, 2D and 3D transforms including `perspective()`, `text-overflow: ellipsis` and `line-clamp` work. The prefixed `-webkit-line-clamp` isn't parsed yet, and text shadows skip underlines.

Draw on a `<canvas>` from Rust:

```rust
ui.select("#chart").draw(|canvas| {
    canvas.set_fill(Color::rgb(59, 130, 246)).fill_rect(10.0, 10.0, 80.0, 40.0);
    canvas.set_font("Arial", 12.0).fill_text("March", 10.0, 64.0);
});
```

Rectangles, paths, arcs, strokes, transforms, clipping, alpha and text work. Images, gradients and dashed lines don't yet.

`crates/pageframe` is the API apps use. The engine under it is in the other crates in `crates/`, which started as Blitz's packages.

## License

MIT or Apache-2.0, at your option, like Blitz ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Some dependencies, Stylo among them, are MPL-2.0; [docs/licensing.md](docs/licensing.md) is Blitz's review of what that means for your app. [NOTICE](NOTICE) credits the Blitz authors.
