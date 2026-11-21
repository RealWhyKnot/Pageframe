use std::time::{Duration, Instant};

use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;

fn page(base: &str, html: &str) -> ScriptDocument {
    let config = DocumentConfig {
        base_url: Some(base.to_string()),
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
fn location_parts_include_non_default_ports() {
    let mut doc = page(
        "http://127.0.0.1:8123/app/index.html?x=1#top",
        r#"<div id="out"></div><script>
            document.getElementById("out").textContent = [
                location.host, location.hostname, location.port, location.origin, location.pathname,
                location.search, location.hash, location.protocol, String(location), document.URL,
                window.location === document.location,
            ].join(" ");
        </script>"#,
    );
    assert_eq!(
        text_of(&doc),
        "127.0.0.1:8123 127.0.0.1 8123 http://127.0.0.1:8123 /app/index.html ?x=1 #top http: \
         http://127.0.0.1:8123/app/index.html?x=1#top http://127.0.0.1:8123/app/index.html?x=1#top true"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn default_ports_are_omitted_from_host() {
    let doc = page(
        "http://example.test:80/",
        r#"<div id="out"></div><script>
            document.getElementById("out").textContent = location.host + "|" + location.port;
        </script>"#,
    );
    assert_eq!(text_of(&doc), "example.test|");
}

#[test]
fn setting_hash_fires_popstate_then_hashchange_asynchronously() {
    let mut doc = page(
        "http://127.0.0.1:8123/",
        r##"<div id="out"></div><script>
            const seen = [];
            addEventListener("popstate", (e) => seen.push("popstate " + e.state));
            addEventListener("hashchange", (e) => {
                seen.push("hashchange " + e.oldURL + " " + e.newURL + " " + (e instanceof HashChangeEvent));
                document.getElementById("out").textContent = seen.join("|");
            });
            location.hash = "#/library";
            seen.push("sync " + location.hash + " " + history.length);
        </script>"##,
    );
    assert_eq!(
        pump(&mut doc),
        "sync #/library 2|popstate null|hashchange http://127.0.0.1:8123/ http://127.0.0.1:8123/#/library true"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn replace_swaps_the_current_entry() {
    let mut doc = page(
        "http://127.0.0.1:8123/",
        r##"<div id="out"></div><script>
            location.replace("#/welcome");
            const after = [location.hash, history.length];
            onhashchange = () => {
                document.getElementById("out").textContent = [...after, location.href].join(" ");
            };
        </script>"##,
    );
    assert_eq!(
        pump(&mut doc),
        "#/welcome 1 http://127.0.0.1:8123/#/welcome"
    );
}

#[test]
fn history_push_and_back_restore_state() {
    let mut doc = page(
        "http://127.0.0.1:8123/start",
        r##"<div id="out"></div><script>
            history.pushState({ page: 2 }, "", "/two#a");
            history.replaceState({ page: 3 }, "");
            const pushed = [location.pathname, location.hash, history.state.page, history.length];
            let error = "";
            try { history.pushState(null, "", "http://other.test/"); } catch (e) { error = e.name; }
            let hashchanges = 0;
            addEventListener("hashchange", () => hashchanges++);
            addEventListener("popstate", (e) => {
                const seen = [...pushed, error, location.pathname, JSON.stringify(e.state)];
                setTimeout(() => {
                    document.getElementById("out").textContent = [...seen, hashchanges].join(" ");
                }, 20);
            });
            history.back();
        </script>"##,
    );
    assert_eq!(pump(&mut doc), "/two #a 3 2 SecurityError /start null 0");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn same_hash_does_not_fire_hashchange() {
    let mut doc = page(
        "http://127.0.0.1:8123/#x",
        r##"<div id="out"></div><script>
            let fired = 0;
            addEventListener("hashchange", () => fired++);
            location.hash = "x";
            location.hash = "#x";
            setTimeout(() => {
                document.getElementById("out").textContent = fired + " " + history.length;
            }, 20);
        </script>"##,
    );
    assert_eq!(pump(&mut doc), "0 1");
}

#[test]
fn push_state_only_rewrites_allowed_urls() {
    let mut doc = page(
        "file:///app/index.html",
        r##"<div id="out"></div><script>
            const attempt = (url) => {
                try { history.pushState(null, "", url); return location.href; } catch (e) { return e.name; }
            };
            document.getElementById("out").textContent = [
                attempt("#a"),
                attempt("?q=1"),
                attempt("/other.html"),
                attempt("http://user@127.0.0.1/"),
            ].join(" ");
        </script>"##,
    );
    assert_eq!(
        pump(&mut doc),
        "file:///app/index.html#a file:///app/index.html?q=1 SecurityError SecurityError"
    );
}

#[test]
fn push_state_rejects_credential_changes() {
    let mut doc = page(
        "http://127.0.0.1:8123/start",
        r##"<div id="out"></div><script>
            let error = "";
            try { history.pushState(null, "", "http://user@127.0.0.1:8123/start"); } catch (e) { error = e.name; }
            document.getElementById("out").textContent = error + " " + location.href;
        </script>"##,
    );
    assert_eq!(pump(&mut doc), "SecurityError http://127.0.0.1:8123/start");
}
