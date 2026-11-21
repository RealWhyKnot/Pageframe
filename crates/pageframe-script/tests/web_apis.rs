use std::time::{Duration, Instant};

use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;
use pageframe_traits::shell::{ColorScheme, Viewport};

fn page(html: &str, viewport: Option<Viewport>) -> ScriptDocument {
    let config = DocumentConfig {
        base_url: Some("http://127.0.0.1:8123/app/index.html".to_string()),
        viewport,
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config).without_timer_thread();
    doc.execute_scripts();
    doc
}

fn text_of(doc: &ScriptDocument) -> String {
    let inner = doc.inner();
    let id = inner.query_selector("#out").unwrap().unwrap();
    inner.get_node(id).unwrap().text_content()
}

fn pump(doc: &mut ScriptDocument) -> String {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        doc.poll(None);
        let text = text_of(doc);
        if !text.is_empty() || Instant::now() > deadline {
            return text;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
}

#[test]
fn performance_now_is_monotonic_and_coarsened() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const a = performance.now();
            const b = performance.now();
            performance.mark("start");
            const measure = performance.measure("span", "start");
            document.getElementById("out").textContent = [
                typeof a, a >= 0, b >= a, Math.abs(a * 10 - Math.round(a * 10)) < 1e-6,
                Math.abs(performance.timeOrigin + a - Date.now()) < 1000,
                performance.getEntriesByName("start").length, measure.entryType,
            ].join(",");
        </script>"#,
        None,
    );
    assert_eq!(text_of(&doc), "number,true,true,true,true,1,measure");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn storage_round_trips_strings() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            localStorage.setItem("a", 1);
            localStorage.b = "two";
            sessionStorage.setItem("s", "x");
            const before = [localStorage.length, localStorage.getItem("a"), typeof localStorage.getItem("a"), localStorage.b];
            localStorage.removeItem("a");
            const keys = Object.keys(localStorage).join("+");
            localStorage.clear();
            document.getElementById("out").textContent = [
                ...before, keys, localStorage.length, localStorage.getItem("missing"), sessionStorage.getItem("s"),
            ].join(",");
        </script>"#,
        None,
    );
    assert_eq!(text_of(&doc), "2,1,string,two,b,0,,x");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn match_media_reads_the_viewport() {
    let viewport = Viewport::new(1600, 1000, 2.0, ColorScheme::Dark);
    let mut doc = page(
        r#"<div id="out"></div><script>
            const q = (text) => matchMedia(text).matches;
            const list = matchMedia("(min-width: 700px)");
            document.getElementById("out").textContent = [
                q("(min-width: 700px)"), q("(min-width: 900px)"), q("(max-width: 800px)"),
                q("(prefers-color-scheme: dark)"), q("(prefers-color-scheme: light)"),
                q("(pointer: fine)"), q("(hover: hover)"), q("(hover: none)"),
                q("(prefers-reduced-motion: reduce)"), q("screen and (min-resolution: 2dppx)"),
                q("print"), q("not print"), q("(orientation: portrait)"), q("(min-width: 900px), (max-width: 1000px)"),
                list.media, typeof list.addListener, typeof list.addEventListener, list instanceof EventTarget,
            ].join(",");
        </script>"#,
        Some(viewport),
    );
    assert_eq!(
        text_of(&doc),
        "true,false,true,true,false,true,true,false,false,true,false,true,false,true,(min-width: 700px),function,function,true"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn match_media_notifies_listeners_when_the_viewport_changes() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const list = matchMedia("(prefers-color-scheme: dark)");
            const seen = [list.matches];
            list.addEventListener("change", (event) => {
                seen.push(event.matches, event.media, event instanceof MediaQueryListEvent);
                document.getElementById("out").textContent = seen.join(",");
            });
        </script>"#,
        Some(Viewport::new(800, 600, 1.0, ColorScheme::Light)),
    );
    doc.poll(None);
    doc.inner_mut()
        .set_viewport(Viewport::new(800, 600, 1.0, ColorScheme::Dark));
    assert_eq!(
        pump(&mut doc),
        "false,true,(prefers-color-scheme: dark),true"
    );
}

#[test]
fn crypto_fills_integer_arrays() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const bytes = crypto.getRandomValues(new Uint8Array(32));
            const words = crypto.getRandomValues(new Uint32Array(4));
            const id = crypto.randomUUID();
            let floats = "";
            try { crypto.getRandomValues(new Float64Array(2)); } catch (error) { floats = error.name; }
            document.getElementById("out").textContent = [
                bytes.some((byte) => byte !== 0), words.length,
                /^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/.test(id),
                id !== crypto.randomUUID(), floats,
            ].join(",");
        </script>"#,
        None,
    );
    assert_eq!(text_of(&doc), "true,4,true,true,TypeMismatchError");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn idle_callbacks_run_and_cancel() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const seen = [];
            const cancelled = requestIdleCallback(() => seen.push("cancelled"));
            cancelIdleCallback(cancelled);
            requestIdleCallback((deadline) => {
                seen.push("idle", deadline.didTimeout, typeof deadline.timeRemaining());
                document.getElementById("out").textContent = seen.join(",");
            });
        </script>"#,
        None,
    );
    assert_eq!(pump(&mut doc), "idle,false,number");
}

#[test]
fn file_reader_reads_blobs_as_data_urls() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const blob = new Blob(["hi ", new Uint8Array([0x21])], { type: "text/plain" });
            const reader = new FileReader();
            reader.onload = () => {
                blob.text().then((text) => {
                    document.getElementById("out").textContent =
                        [reader.result, reader.readyState, blob.size, blob.type, text, blob.slice(1, 2).size].join(",");
                });
            };
            reader.readAsDataURL(blob);
        </script>"#,
        None,
    );
    assert_eq!(
        pump(&mut doc),
        "data:text/plain;base64,aGkgIQ==,2,4,text/plain,hi !,1"
    );
}

#[test]
fn dialogs_open_close_and_report_return_values() {
    let mut doc = page(
        r#"<dialog id="dlg"><button id="ok">OK</button></dialog><div id="out"></div><script>
            const dlg = document.getElementById("dlg");
            const seen = [dlg.open, dlg.returnValue === ""];
            dlg.addEventListener("cancel", (event) => seen.push("cancel " + event.cancelable));
            dlg.addEventListener("close", () => {
                seen.push("close " + dlg.returnValue + " " + dlg.open);
                document.getElementById("out").textContent = seen.join(",");
            });
            dlg.showModal();
            seen.push(dlg.open, dlg.hasAttribute("open"), document.activeElement && document.activeElement.id);
            let error = "";
            try { dlg.show(); } catch (e) { error = e.name; }
            seen.push(error);
            dlg.requestClose("yes");
            seen.push(dlg.open);
        </script>"#,
        None,
    );
    assert_eq!(
        pump(&mut doc),
        "false,true,true,true,ok,InvalidStateError,cancel true,false,close yes false"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn pointer_capture_tracks_pointers() {
    let mut doc = page(
        r#"<div id="a"></div><div id="b"></div><div id="out"></div><script>
            const a = document.getElementById("a");
            const b = document.getElementById("b");
            const seen = [];
            a.addEventListener("gotpointercapture", (e) => seen.push("got " + e.pointerId));
            a.addEventListener("lostpointercapture", (e) => seen.push("lost " + e.pointerId));
            a.setPointerCapture(1);
            seen.push(a.hasPointerCapture(1), b.hasPointerCapture(1));
            b.setPointerCapture(1);
            seen.push(a.hasPointerCapture(1), b.hasPointerCapture(1));
            b.releasePointerCapture(1);
            seen.push(b.hasPointerCapture(1));
            const detached = document.createElement("div");
            try { detached.setPointerCapture(1); } catch (e) { seen.push(e.name); }
            document.getElementById("out").textContent = seen.join(",");
        </script>"#,
        None,
    );
    assert_eq!(
        text_of(&doc),
        "got 1,true,false,lost 1,false,true,false,InvalidStateError"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}
