use pageframe_dom::DocumentConfig;
use pageframe_script::ScriptDocument;

fn run(html: &str, script: &str) -> Vec<String> {
    let mut doc = ScriptDocument::from_html(html, DocumentConfig::default());
    doc.eval(&format!(
        "const say = (value) => __pageframe_send_message(String(value));\n{script}"
    ));
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    doc.take_messages()
}

#[test]
fn click_dispatches_bubbling_cancelable_mouse_event() {
    let messages = run(
        "<div id='outer'><button id='button'>go</button></div>",
        r#"
        outer.addEventListener("click", (event) => {
            say([event.type, event.constructor.name, event.bubbles, event.cancelable,
                event.composed, event.isTrusted, event.target.id, event.detail].join(","));
        });
        say(typeof HTMLElement.prototype.click);
        button.click();
    "#,
    );
    assert_eq!(
        messages,
        ["function", "click,MouseEvent,true,true,true,false,button,0"]
    );
}

#[test]
fn click_toggles_checkbox_and_reverts_when_canceled() {
    let messages = run(
        "<input id='box' type='checkbox'><button id='off' disabled>x</button>",
        r#"
        box.addEventListener("change", () => say("change:" + box.checked));
        box.click();
        box.addEventListener("click", (event) => event.preventDefault());
        box.click();
        say(box.checked);
        off.addEventListener("click", () => say("disabled clicked"));
        off.click();
    "#,
    );
    assert_eq!(messages, ["change:true", "true"]);
}

#[test]
fn click_on_label_forwards_to_control_and_submit_button_submits() {
    let messages = run(
        "<label id='label' for='box'>L</label><input id='box' type='checkbox'>\
         <form id='form'><button id='send'>send</button></form>",
        r#"
        box.addEventListener("click", () => say("box clicked"));
        label.click();
        say(box.checked);
        form.addEventListener("submit", (event) => {
            say("submit:" + (event.submitter === send));
            event.preventDefault();
        });
        send.click();
    "#,
    );
    assert_eq!(messages, ["box clicked", "true", "submit:true"]);
}

#[test]
fn event_handler_runs_at_position_it_was_first_set() {
    let messages = run(
        "<div id='target'></div>",
        r#"
        target.addEventListener("click", () => say("first"));
        target.onclick = () => say("handler A");
        target.addEventListener("click", () => say("last"));
        target.onclick = () => say("handler B");
        target.click();
        target.onclick = null;
        target.onclick = () => say("handler C");
        target.click();
        say(typeof target.onclick);
    "#,
    );
    assert_eq!(
        messages,
        [
            "first",
            "handler B",
            "last",
            "first",
            "last",
            "handler C",
            "function"
        ]
    );
}

#[test]
fn window_handler_ordering_and_return_false_cancels() {
    let messages = run(
        "<a id='link'>x</a>",
        r#"
        onmessage = () => say("message handler");
        window.addEventListener("message", () => say("message listener"));
        dispatchEvent(new Event("message"));
        link.onclick = () => false;
        const event = new MouseEvent("click", { cancelable: true });
        say(link.dispatchEvent(event) + ":" + event.defaultPrevented);
        say(window.onmessage !== null);
    "#,
    );
    assert_eq!(
        messages,
        ["message handler", "message listener", "false:true", "true"]
    );
}
