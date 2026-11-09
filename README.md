# Pageframe

Build desktop app UIs in Rust with HTML and CSS. Pageframe styles your page with Stylo, lays it out with Taffy and Parley, and draws it with Vello. Your Rust code handles the events and edits the DOM; there's no browser or JavaScript engine in the app.

Pageframe is a fork of [Blitz](https://github.com/DioxusLabs/blitz).

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

## Testing

`app.page(width, height)` builds the page without opening a window. Wrap it in `blitz-test-harness` to click, type and check layout:

```rust
let mut harness = Harness::wrap(app.page(400, 300).expect("selectors are valid"));
harness.click("#add");
assert_eq!(harness.text_content("#count"), "1");
```

`cargo test -p pageframe` runs the crate's tests, including pixel checks rendered on the CPU.

## Status

Early, and the API will change. CSS support comes from Blitz. Flexbox, CSS variables, rounded corners, box and text shadows, gradients, `mix-blend-mode`, 2D and 3D transforms including `perspective()`, `text-overflow: ellipsis` and `line-clamp` work. The prefixed `-webkit-line-clamp` isn't parsed yet, and text shadows skip underlines. In my tests multi-column layout and `<select>` didn't render.

`crates/pageframe` is Pageframe itself. `packages/`, `examples/`, `tests/` and `wpt/` come from Blitz and stay close to upstream so its fixes can be merged in.

## License

MIT or Apache-2.0, at your option, like Blitz ([LICENSE-MIT](LICENSE-MIT), [LICENSE-APACHE](LICENSE-APACHE)). Some dependencies, Stylo among them, are MPL-2.0; [docs/licensing.md](docs/licensing.md) is Blitz's review of what that means for your app. [NOTICE](NOTICE) credits the Blitz authors.
