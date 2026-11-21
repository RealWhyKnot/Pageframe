use pageframe_dom::DocumentConfig;
use pageframe_script::ScriptDocument;
use pageframe_traits::shell::{ColorScheme, Viewport};

fn run(body: &str, script: &str) -> String {
    let config = DocumentConfig {
        viewport: Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
        ..Default::default()
    };
    let html = format!("<!doctype html><html><body>{body}</body></html>");
    let mut doc = ScriptDocument::from_html(&html, config);
    doc.eval(&format!(
        "__pageframe_send_message((() => {{\n{script}\n}})().map(String).join(\"|\"));"
    ));
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    doc.take_messages().join("\n")
}

#[test]
fn svg_attributes_keep_their_case_and_html_attributes_lowercase() {
    let script = r##"
const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
svg.setAttribute("viewBox", "0 0 24 24");
const out = [svg.getAttributeNames().join(","), svg.getAttribute("viewBox"), svg.getAttribute("viewbox"),
    svg.hasAttribute("viewBox"), svg.hasAttribute("viewbox")];
svg.removeAttribute("viewbox");
out.push(svg.hasAttribute("viewBox"));
svg.removeAttribute("viewBox");
out.push(svg.hasAttribute("viewBox"));
const div = document.createElement("div");
div.setAttribute("DATA-Foo", "1");
out.push(div.getAttributeNames().join(","), div.getAttribute("data-FOO"), div.hasAttribute("Data-Foo"));
div.removeAttribute("DATA-FOO");
out.push(div.hasAttribute("data-foo"));
document.getElementById("host").innerHTML = '<svg viewBox="0 0 10 10"></svg>';
out.push(document.querySelector("#host svg").getAttribute("viewBox"));
return out;
"##;
    assert_eq!(
        run("<div id=host></div>", script),
        "viewBox|0 0 24 24|null|true|false|true|false|data-foo|1|true|false|0 0 10 10"
    );
}

#[test]
fn select_options_and_selected_index_follow_the_selection() {
    let body = r##"<select id=s><option value=a>A</option><optgroup label=g><option>B  two</option></optgroup>
<option value=c selected>C</option></select><div id=d></div>"##;
    let script = r##"
const s = document.getElementById("s");
const out = [s.options.length, s.options[1].value, s.options[2] === s.lastElementChild, s.selectedIndex, s.value];
s.selectedIndex = 0;
out.push(s.selectedIndex, s.value);
s.selectedIndex = 10;
out.push(s.selectedIndex, JSON.stringify(s.value));
s.value = "c";
out.push(s.selectedIndex);
s.value = "B two";
out.push(s.selectedIndex);
s.value = "missing";
out.push(s.selectedIndex);
out.push(Array.from(s.options).map((o) => o.textContent).join(","));
const d = document.getElementById("d");
out.push(d.options, d.selectedIndex);
return out;
"##;
    assert_eq!(
        run(body, script),
        "3|B two|true|2|c|0|a|-1|\"\"|2|1|-1|A,B  two,C|undefined|undefined"
    );
}

#[test]
fn scroll_into_view_scrolls_nested_scrollers_like_chrome() {
    let body = r##"<style>body{margin:0}#outer{height:300px;overflow:auto}
#inner{height:100px;overflow:auto;border:2px solid}.pad{height:500px}#t{height:20px}</style>
<div id=outer><div class=pad></div><div id=inner><div class=pad></div><div id=t></div>
<div class=pad></div></div><div class=pad></div></div>"##;
    let script = r##"
const outer = document.getElementById("outer");
const inner = document.getElementById("inner");
const t = document.getElementById("t");
const out = [];
for (const block of ["center", "start", "end"]) {
    outer.scrollTop = 0;
    inner.scrollTop = 0;
    t.scrollIntoView({ block });
    out.push(block + ":" + inner.scrollTop + "," + outer.scrollTop + "," + window.scrollY);
}
t.scrollIntoView({ block: "nearest" });
out.push("nearest:" + inner.scrollTop + "," + outer.scrollTop);
return out;
"##;
    assert_eq!(
        run(body, script),
        "center:460,402,0|start:500,502,0|end:420,302,0|nearest:420,302"
    );
}
