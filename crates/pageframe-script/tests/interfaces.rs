use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;

fn run(html: &str) -> (String, Vec<String>) {
    let mut doc = ScriptDocument::from_html(html, DocumentConfig::default());
    doc.execute_scripts();
    let errors = doc.take_js_errors();
    let inner = doc.inner();
    let id = inner.query_selector("#out").unwrap().unwrap();
    (inner.get_node(id).unwrap().text_content(), errors)
}

#[test]
fn tag_interfaces_match_their_elements() {
    let (text, errors) = run(
        r#"<button id="save">Save</button><div id="box"></div><svg><circle id="dot"></circle></svg>
        <div id="out"></div><script>
            const save = document.getElementById("save");
            const box = document.getElementById("box");
            const dot = document.getElementById("dot");
            const made = (tag) => document.createElement(tag);
            let constructed = "constructed";
            try { new HTMLDivElement(); } catch (e) { constructed = e.constructor.name; }
            document.getElementById("out").textContent = [
                save instanceof HTMLButtonElement,
                save instanceof HTMLInputElement,
                box instanceof HTMLDivElement,
                box instanceof HTMLButtonElement,
                made("input") instanceof HTMLInputElement,
                made("dialog") instanceof HTMLDialogElement,
                made("video") instanceof HTMLMediaElement,
                made("h3") instanceof HTMLHeadingElement,
                made("canvas") instanceof HTMLCanvasElement,
                dot instanceof SVGElement,
                dot instanceof HTMLDivElement,
                box instanceof SVGElement,
                document.createTextNode("x") instanceof HTMLSpanElement,
                ({ localName: "div" }) instanceof HTMLDivElement,
                HTMLButtonElement.name,
                constructed,
            ].join(",");
        </script>"#,
    );
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        text,
        "true,false,true,false,true,true,true,true,true,true,false,false,false,false,HTMLButtonElement,TypeError"
    );
}
