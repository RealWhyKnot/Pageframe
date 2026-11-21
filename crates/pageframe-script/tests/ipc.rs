use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::{IpcMessage, ScriptDocument};

fn document() -> ScriptDocument {
    let mut doc = ScriptDocument::from_html("<div id='root'></div>", DocumentConfig::default())
        .without_timer_thread();
    doc.execute_scripts();
    doc.eval("globalThis.say = (value) => __pageframe_send_message(String(value));");
    doc
}

fn clean(doc: &mut ScriptDocument) -> Vec<String> {
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    doc.take_messages()
}

fn invoke_ids(requests: &[IpcMessage]) -> Vec<(u64, String, String)> {
    requests
        .iter()
        .filter_map(|request| match request {
            IpcMessage::Invoke {
                id,
                channel,
                payload,
            } => Some((*id, channel.clone(), payload.clone())),
            IpcMessage::Post(_) => None,
        })
        .collect()
}

#[test]
fn invoke_ids_never_repeat_across_documents() {
    let mut first = document();
    let mut second = document();
    first.eval(r#"ipc.invoke("a").then((value) => say("first:" + value));"#);
    second.eval(r#"ipc.invoke("a").then((value) => say("second:" + value));"#);
    let first_id = invoke_ids(&first.take_ipc())[0].0;
    let second_id = invoke_ids(&second.take_ipc())[0].0;
    assert_ne!(first_id, second_id);
    second.settle_ipc(first_id, Ok("1"));
    assert_eq!(clean(&mut second), Vec::<String>::new());
    second.settle_ipc(second_id, Ok("2"));
    assert_eq!(clean(&mut second), ["second:2"]);
}

#[test]
fn invoke_sends_request_and_resolves_with_reply() {
    let mut doc = document();
    doc.eval(
        r#"
        ipc.invoke("sum", { a: 2, b: 3 }).then((value) => say("ok:" + JSON.stringify(value)));
        ipc.invoke("broken").catch((error) => say("err:" + error.message));
    "#,
    );
    let ids = invoke_ids(&doc.take_ipc());
    let (sum, broken) = (ids[0].0, ids[1].0);
    assert_eq!(
        ids,
        [
            (sum, String::from("sum"), String::from(r#"{"a":2,"b":3}"#)),
            (broken, String::from("broken"), String::from("null")),
        ]
    );
    assert!(broken > sum);
    assert_eq!(clean(&mut doc), Vec::<String>::new());
    doc.settle_ipc(broken, Err("no such command"));
    doc.settle_ipc(sum, Ok(r#"{"total":5}"#));
    doc.settle_ipc(sum, Ok("6"));
    assert_eq!(
        clean(&mut doc),
        ["err:no such command", r#"ok:{"total":5}"#]
    );
}

#[test]
fn emit_reaches_listeners_until_unsubscribed() {
    let mut doc = document();
    doc.eval(
        r#"
        const off = ipc.on("tick", (payload) => say("a:" + payload.n));
        ipc.on("tick", (payload) => say("b:" + payload.n));
        ipc.on("other", () => say("wrong channel"));
        globalThis.off = off;
    "#,
    );
    doc.emit_ipc("tick", r#"{"n":1}"#);
    doc.eval("off();");
    doc.emit_ipc("tick", r#"{"n":2}"#);
    assert_eq!(clean(&mut doc), ["a:1", "b:1", "b:2"]);
}

#[test]
fn post_message_matches_wry_string_channel() {
    let mut doc = document();
    doc.eval(
        r#"window.ipc.postMessage("hello"); window.ipc.postMessage({ toString: () => "obj" });"#,
    );
    assert_eq!(
        doc.take_ipc(),
        [
            IpcMessage::Post("hello".into()),
            IpcMessage::Post("obj".into())
        ]
    );
    assert_eq!(clean(&mut doc), Vec::<String>::new());
}

#[test]
fn listener_errors_are_reported_without_stopping_others() {
    let mut doc = document();
    doc.eval(
        r#"
        ipc.on("x", () => { throw new Error("boom"); });
        ipc.on("x", () => say("still runs"));
    "#,
    );
    doc.emit_ipc("x", "null");
    assert_eq!(doc.take_messages(), ["still runs"]);
    if let Some(deadline) = doc.next_timer_deadline() {
        std::thread::sleep(deadline.saturating_duration_since(web_time::Instant::now()));
    }
    doc.poll(None);
    let errors = doc.take_js_errors();
    assert_eq!(errors.len(), 1, "{errors:?}");
    assert!(errors[0].contains("boom"), "{errors:?}");
}

#[test]
fn focus_events_reach_window_handlers() {
    let mut doc = document();
    doc.eval(r#"onfocus = () => say("focus"); addEventListener("blur", () => say("blur"));"#);
    doc.dispatch_window_event("focus");
    doc.dispatch_window_event("blur");
    assert_eq!(clean(&mut doc), ["focus", "blur"]);
}
