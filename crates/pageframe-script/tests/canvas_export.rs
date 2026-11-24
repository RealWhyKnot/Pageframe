use std::time::Duration;

use data_url::DataUrl;
use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;

fn page(script: &str) -> ScriptDocument {
    let html =
        format!("<!doctype html><body><div id=\"out\"></div><script>{script}</script></body>");
    let mut doc = ScriptDocument::from_html(&html, DocumentConfig::default())
        .without_timer_thread()
        .with_virtual_time();
    doc.execute_scripts();
    doc
}

fn settle(doc: &mut ScriptDocument) -> Vec<String> {
    for _ in 0..10 {
        let now = doc.clock_now();
        doc.advance_clock_to(now + Duration::from_millis(5));
        doc.poll(None);
    }
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    doc.take_messages()
}

fn decode(url: &str) -> (String, Vec<u8>) {
    let parsed = DataUrl::process(url).expect("valid data URL");
    let mime = parsed.mime_type().to_string();
    let (bytes, _) = parsed.decode_to_vec().expect("valid base64");
    (mime, bytes)
}

const RED_GREEN: &str = r##"
const c = document.createElement("canvas");
c.width = 4;
c.height = 2;
const ctx = c.getContext("2d");
ctx.fillStyle = "rgb(255, 0, 0)";
ctx.fillRect(0, 0, 2, 2);
ctx.fillStyle = "rgba(0, 255, 0, 0.5)";
ctx.fillRect(2, 0, 2, 2);
"##;

#[test]
fn to_data_url_round_trips_png_pixels() {
    let mut doc = page(&format!(
        "{RED_GREEN}__pageframe_send_message(c.toDataURL()); __pageframe_send_message(c.toDataURL(\"image/gif\"));"
    ));
    let messages = settle(&mut doc);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0], messages[1]);
    assert!(messages[0].starts_with("data:image/png;base64,"));
    let (mime, bytes) = decode(&messages[0]);
    assert_eq!(mime, "image/png");
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (4, 2));
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
    let green = image.get_pixel(3, 1).0;
    assert_eq!([green[0], green[1], green[2]], [0, 255, 0]);
    assert!(green[3].abs_diff(128) <= 1);
}

#[test]
fn to_data_url_encodes_jpeg_over_black_and_webp() {
    let mut doc = page(&format!(
        "{RED_GREEN}
        __pageframe_send_message(c.toDataURL(\"IMAGE/JPEG\"));
        __pageframe_send_message(c.toDataURL(\"image/jpeg\", 0.1));
        __pageframe_send_message(c.toDataURL(\"image/webp\"));"
    ));
    let messages = settle(&mut doc);
    let (mime, jpeg) = decode(&messages[0]);
    assert_eq!(mime, "image/jpeg");
    assert_eq!(&jpeg[..3], &[0xff, 0xd8, 0xff]);
    let image = image::load_from_memory(&jpeg).unwrap().to_rgb8();
    let green = image.get_pixel(3, 1).0;
    assert!(green[1].abs_diff(128) <= 24, "{green:?}");
    assert!(green[0] < 40 && green[2] < 40, "{green:?}");
    let (_, low) = decode(&messages[1]);
    assert_eq!(&low[..3], &[0xff, 0xd8, 0xff]);
    assert_ne!(low, jpeg);
    let (mime, webp) = decode(&messages[2]);
    assert_eq!(mime, "image/webp");
    assert_eq!(&webp[..4], b"RIFF");
    let image = image::load_from_memory(&webp).unwrap().to_rgba8();
    assert_eq!(image.get_pixel(0, 0).0, [255, 0, 0, 255]);
}

#[test]
fn to_data_url_without_context_and_zero_size() {
    let mut doc = page(
        r##"
        const blank = document.createElement("canvas");
        blank.width = 3;
        blank.height = 5;
        __pageframe_send_message(blank.toDataURL());
        const empty = document.createElement("canvas");
        empty.width = 0;
        __pageframe_send_message(empty.toDataURL());
        empty.getContext("2d");
        __pageframe_send_message(empty.toDataURL("image/jpeg"));
        "##,
    );
    let messages = settle(&mut doc);
    let (_, bytes) = decode(&messages[0]);
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (3, 5));
    assert!(image.pixels().all(|p| p.0 == [0, 0, 0, 0]));
    assert_eq!(messages[1..], ["data:,", "data:,"]);
}

#[test]
fn convert_to_blob_resolves_readable_png_and_jpeg_blobs() {
    let mut doc = page(
        r##"
        const off = new OffscreenCanvas(6, 3);
        const ctx = off.getContext("2d");
        ctx.fillStyle = "#0000ff";
        ctx.fillRect(0, 0, 6, 3);
        (async () => {
            const blob = await off.convertToBlob();
            const head = new Uint8Array(await blob.arrayBuffer()).slice(0, 8);
            __pageframe_send_message([blob instanceof Blob, blob.type, blob.size > 0, Array.from(head).join(" ")].join("|"));
            __pageframe_send_message((await off.convertToBlob({ type: "image/jpeg", quality: 0.8 })).type);
            __pageframe_send_message((await new OffscreenCanvas(5, 5).convertToBlob({ type: "image/bmp" })).type);
            const reader = new FileReader();
            reader.onload = () => __pageframe_send_message(reader.result);
            reader.readAsDataURL(blob);
        })();
        "##,
    );
    let messages = settle(&mut doc);
    assert_eq!(
        messages[..3],
        [
            "true|image/png|true|137 80 78 71 13 10 26 10",
            "image/jpeg",
            "image/png"
        ]
    );
    let (mime, bytes) = decode(&messages[3]);
    assert_eq!(mime, "image/png");
    let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(image.dimensions(), (6, 3));
    assert_eq!(image.get_pixel(5, 2).0, [0, 0, 255, 255]);
}

#[test]
fn convert_to_blob_rejects_zero_sized_canvas() {
    let mut doc = page(
        r##"
        const results = [];
        const report = (e) => {
            results.push(e.name);
            if (results.length === 2) __pageframe_send_message(results.join(","));
        };
        new OffscreenCanvas(0, 10).convertToBlob().catch(report);
        new OffscreenCanvas(10, 0).convertToBlob({ type: "image/jpeg" }).catch(report);
        "##,
    );
    assert_eq!(settle(&mut doc), ["IndexSizeError,IndexSizeError"]);
}

#[test]
fn to_blob_calls_back_with_encoded_blob() {
    let mut doc = page(&format!(
        "{RED_GREEN}c.toBlob((blob) => __pageframe_send_message(blob.type + \"|\" + (blob.size > 0)), \"image/webp\");"
    ));
    assert_eq!(settle(&mut doc), ["image/webp|true"]);
}
