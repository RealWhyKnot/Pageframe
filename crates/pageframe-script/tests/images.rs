use std::sync::Arc;
use std::time::{Duration, Instant};

use pageframe_dom::node::{ImageData, RasterImageData, SpecialElementData};
use pageframe_dom::{Document, DocumentConfig, NodeId};
use pageframe_script::{HttpClient, ScriptDocument};

fn page(html: &str) -> ScriptDocument {
    let config = DocumentConfig {
        base_url: Some("http://127.0.0.1:9/app/".to_string()),
        net_provider: Some(Arc::new(HttpClient::new())),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config).without_timer_thread();
    doc.execute_scripts();
    doc
}

fn offline_page(html: &str) -> ScriptDocument {
    let config = DocumentConfig {
        base_url: Some("http://127.0.0.1:9/app/".to_string()),
        ..Default::default()
    };
    let mut doc = ScriptDocument::from_html(html, config).without_timer_thread();
    doc.execute_scripts();
    doc
}

fn finish_pending_images(doc: &mut ScriptDocument) -> usize {
    let mut inner = doc.inner_mut();
    let mut finished = 0;
    for index in 0..256_u64 {
        let Some(element) = inner
            .get_node_mut(NodeId::from_u64(index))
            .and_then(|node| node.element_data_mut())
        else {
            continue;
        };
        if matches!(&element.special_data, SpecialElementData::ImageFallback(fallback) if fallback.loading)
        {
            let raster = RasterImageData::new(3, 2, Arc::new(vec![255; 24]));
            element.special_data = SpecialElementData::Image(Box::new(ImageData::Raster(raster)));
            finished += 1;
        }
    }
    finished
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
fn src_reflects_the_attribute_and_resolves_against_the_base() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const img = document.createElement("img");
            const before = [img.src === "", img.complete, img.naturalWidth];
            img.src = "thumbs/1.png";
            const div = document.createElement("div");
            div.src = "kept";
            document.getElementById("out").textContent = [
                ...before,
                img.getAttribute("src"),
                img.src,
                div.src,
                div.hasAttribute("src"),
            ].join(",");
        </script>"#,
    );
    assert_eq!(
        text_of(&doc),
        "true,true,0,thumbs/1.png,http://127.0.0.1:9/app/thumbs/1.png,kept,false"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn load_fires_with_natural_size_for_detached_images() {
    let mut doc = offline_page(
        r#"<div id="out"></div><script>
            const img = new Image(10, 20);
            const seen = [img.width, img.height, img instanceof HTMLImageElement];
            let heard = false;
            img.onload = (event) => {
                seen.push(event.type, event.target === img, event.bubbles, img.complete);
                seen.push(img.naturalWidth, img.naturalHeight, img.currentSrc.endsWith("/app/thumbs/1.png"));
                setTimeout(() => {
                    document.getElementById("out").textContent = [...seen, heard].join(",");
                });
            };
            img.addEventListener("load", () => { heard = true; });
            img.src = "thumbs/1.png";
            seen.push(img.isConnected, img.complete);
        </script>"#,
    );
    doc.poll(None);
    assert_eq!(text_of(&doc), "");
    assert_eq!(finish_pending_images(&mut doc), 1);
    assert_eq!(
        pump(&mut doc),
        "10,20,true,false,false,load,true,false,true,3,2,true,true"
    );
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn error_fires_for_images_that_fail_to_load() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            const img = document.createElement("img");
            img.onerror = (event) => {
                document.getElementById("out").textContent =
                    [event.type, img.complete, img.naturalWidth, img.currentSrc].join(",");
            };
            img.src = "data:image/png;base64,AAAA";
        </script>"#,
    );
    assert_eq!(pump(&mut doc), "error,true,0,");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}

#[test]
fn removing_src_cancels_pending_events() {
    let mut doc = offline_page(
        r#"<div id="out"></div><script>
            const img = document.createElement("img");
            let fired = "none";
            img.onload = () => { fired = "load"; };
            img.onerror = () => { fired = "error"; };
            img.src = "thumbs/3.png";
            img.removeAttribute("src");
            setTimeout(() => {
                document.getElementById("out").textContent = fired + "," + img.src;
            }, 50);
        </script>"#,
    );
    let text = pump(&mut doc);
    assert_eq!(text, "none,");
}

#[test]
fn decode_resolves_once_loaded() {
    let mut doc = offline_page(
        r#"<div id="out"></div><script>
            const img = new Image();
            img.src = "thumbs/2.png";
            img.decode().then(() => {
                document.getElementById("out").textContent = "decoded " + img.naturalWidth;
            });
        </script>"#,
    );
    doc.poll(None);
    assert_eq!(finish_pending_images(&mut doc), 1);
    assert_eq!(pump(&mut doc), "decoded 3");
}

#[test]
fn image_constructor_requires_new() {
    let mut doc = page(
        r#"<div id="out"></div><script>
            let message = "";
            try { Image(); } catch (error) { message = error.constructor.name; }
            document.getElementById("out").textContent = message + "," + Image.name;
        </script>"#,
    );
    assert_eq!(text_of(&doc), "TypeError,Image");
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
}
