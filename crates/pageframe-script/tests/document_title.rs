use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;

fn run(html: &str) -> (String, String, Vec<String>) {
    let mut doc = ScriptDocument::from_html(html, DocumentConfig::default());
    doc.execute_scripts();
    let errors = doc.take_js_errors();
    let inner = doc.inner();
    let out = inner.query_selector("#out").unwrap().unwrap();
    let title = inner
        .find_title_node()
        .map(|node| node.text_content())
        .unwrap_or_default();
    (inner.get_node(out).unwrap().text_content(), title, errors)
}

#[test]
fn module_scripts_can_set_document_title() {
    let (out, title, errors) = run(
        r#"<html><head><title>Old</title></head><body><div id="out"></div>
        <script type="module">
            document.title = "Library · Backtrack";
            document.getElementById("out").textContent =
                document.title + "|" + document.querySelectorAll("title").length;
        </script></body></html>"#,
    );
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(out, "Library · Backtrack|1");
    assert_eq!(title, "Library · Backtrack");
}

#[test]
fn setting_title_creates_a_title_element_in_head() {
    let (out, title, errors) = run(r#"<html><head></head><body><div id="out"></div>
        <script type="module">
            document.title = "Fresh";
            const created = document.head.querySelector("title");
            document.getElementById("out").textContent = document.title + "|" + (created !== null);
        </script></body></html>"#);
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(out, "Fresh|true");
    assert_eq!(title, "Fresh");
}
