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
