use std::time::Duration;

use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;
use pageframe_traits::shell::{ColorScheme, Viewport};

fn page(html: &str) -> ScriptDocument {
    let mut doc = ScriptDocument::from_html(html, DocumentConfig::default())
        .without_timer_thread()
        .with_virtual_time();
    doc.inner_mut()
        .set_viewport(Viewport::new(800, 600, 1.0, ColorScheme::Light));
    doc.execute_scripts();
    doc
}

fn frames(doc: &mut ScriptDocument, count: usize) {
    for _ in 0..count {
        let now = doc.clock_now();
        doc.advance_clock_to(now + Duration::from_millis(17));
        doc.poll(None);
    }
}

fn out(doc: &ScriptDocument) -> String {
    let inner = doc.inner();
    let id = inner.query_selector("#out").unwrap().unwrap();
    inner.get_node(id).unwrap().text_content()
}

#[test]
fn resize_observer_reports_initial_and_changed_sizes() {
    let mut doc = page(
        r#"<body style="margin:0"><div id="box" style="width:200px;height:50px;padding:10px;border:2px solid"></div>
        <div id="out"></div><script>
            const box = document.getElementById("box");
            const seen = [];
            new ResizeObserver((entries, observer) => {
                const entry = entries[0];
                seen.push([
                    entries.length,
                    entry.target === box,
                    entry.contentRect.width,
                    entry.contentRect.height,
                    entry.borderBoxSize[0].inlineSize,
                    entry.contentBoxSize[0].blockSize,
                    observer instanceof ResizeObserver,
                ].join(" "));
                document.getElementById("out").textContent = seen.join("|");
            }).observe(box);
            setTimeout(() => { box.style.width = "300px"; }, 100);
        </script></body>"#,
    );
    frames(&mut doc, 3);
    assert_eq!(out(&doc), "1 true 200 50 224 50 true");
    frames(&mut doc, 12);
    assert_eq!(
        out(&doc),
        "1 true 200 50 224 50 true|1 true 300 50 324 50 true"
    );
    frames(&mut doc, 12);
    assert_eq!(out(&doc).matches('|').count(), 1);
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn intersection_observer_tracks_root_margin() {
    let mut doc = page(
        r#"<body style="margin:0"><div id="scroller" style="height:100px;overflow:auto">
            <div id="near" style="height:40px;margin-top:150px"></div>
            <div id="far" style="height:40px;margin-top:400px"></div>
        </div><div id="out"></div><script>
            const seen = [];
            const observer = new IntersectionObserver((changes) => {
                for (const change of changes) {
                    seen.push(change.target.id + "=" + change.isIntersecting + ":" + change.intersectionRatio);
                }
                document.getElementById("out").textContent = seen.sort().join(",") + " " + observer.rootMargin;
            }, { root: document.getElementById("scroller"), rootMargin: "100px" });
            observer.observe(document.getElementById("near"));
            observer.observe(document.getElementById("far"));
        </script></body>"#,
    );
    frames(&mut doc, 3);
    assert_eq!(out(&doc), "far=false:0,near=true:1 100px 100px 100px 100px");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}
