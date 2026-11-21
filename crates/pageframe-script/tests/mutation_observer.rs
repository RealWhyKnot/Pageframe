use pageframe_dom::DocumentConfig;
use pageframe_script::ScriptDocument;

fn run(html: &str, script: &str) -> Vec<String> {
    let mut doc = ScriptDocument::from_html(html, DocumentConfig::default());
    doc.eval(script);
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    doc.take_messages()
}

const DESCRIBE: &str = r#"
    const say = (value) => __pageframe_send_message(String(value));
    const name = (node) => node ? (node.id || node.nodeName) : "null";
    const describe = (record) => [record.type, name(record.target),
        [...record.addedNodes].map(name).join("+"), [...record.removedNodes].map(name).join("+"),
        name(record.previousSibling), name(record.nextSibling), record.attributeName, record.oldValue].join("|");
"#;

fn observe(html: &str, script: &str) -> Vec<String> {
    run(html, &format!("{DESCRIBE}{script}"))
}

#[test]
fn child_list_records_are_batched_into_one_microtask() {
    let messages = observe(
        "<div id='root'><p id='a'></p></div>",
        r#"
        const observer = new MutationObserver((records, self) => {
            say("calls:" + records.length + ":" + (self === observer));
            records.forEach((record) => say(describe(record)));
        });
        observer.observe(root, { childList: true });
        const b = document.createElement("span");
        b.id = "b";
        root.appendChild(b);
        a.remove();
        say("sync");
    "#,
    );
    assert_eq!(
        messages,
        [
            "sync",
            "calls:2:true",
            "childList|root|b||a|null||",
            "childList|root||a|null|b||",
        ]
    );
}

#[test]
fn attribute_records_honour_filter_and_old_value() {
    let messages = observe(
        "<div id='root' class='x'></div>",
        r#"
        new MutationObserver((records) => records.forEach((record) => say(describe(record))))
            .observe(root, { attributeFilter: ["class", "data-k"], attributeOldValue: true });
        root.className = "y";
        root.setAttribute("title", "ignored");
        root.setAttribute("data-k", "1");
        root.removeAttribute("class");
        root.removeAttribute("missing");
    "#,
    );
    assert_eq!(
        messages,
        [
            "attributes|root|||null|null|class|x",
            "attributes|root|||null|null|data-k|",
            "attributes|root|||null|null|class|y",
        ]
    );
}

#[test]
fn subtree_character_data_and_take_records() {
    let messages = observe(
        "<div id='root'><p id='p'>one</p></div>",
        r#"
        const observer = new MutationObserver(() => say("callback"));
        observer.observe(root, { subtree: true, characterData: true, characterDataOldValue: true });
        p.firstChild.data = "two";
        p.firstChild.nodeValue = "two!";
        const records = observer.takeRecords();
        say(records.length);
        records.forEach((record) => say(record.type + ":" + record.oldValue + ":" + record.target.data));
        say(observer.takeRecords().length);
    "#,
    );
    assert_eq!(
        messages,
        ["2", "characterData:one:two!", "characterData:two:two!", "0"]
    );
}

#[test]
fn inner_html_and_text_content_report_replaced_children() {
    let messages = observe(
        "<div id='root'><b id='old'>x</b></div>",
        r#"
        new MutationObserver((records) => records.forEach((record) => say(record.type + ":"
            + record.addedNodes.length + ":" + [...record.removedNodes].map(name).join("+"))))
            .observe(root, { childList: true });
        root.innerHTML = "<i>1</i><i>2</i>";
        root.textContent = "plain";
    "#,
    );
    assert_eq!(messages, ["childList:2:old", "childList:1:I+I"]);
}

#[test]
fn disconnecting_an_idle_observer_leaves_others_observing() {
    let messages = observe(
        "<div id='root'></div>",
        r#"
        const watching = new MutationObserver((records) => say("watching:" + records.length));
        watching.observe(root, { attributes: true });
        const idle = new MutationObserver(() => say("idle"));
        idle.disconnect();
        const twice = new MutationObserver(() => say("twice"));
        twice.observe(root, { attributes: true });
        twice.disconnect();
        twice.disconnect();
        root.setAttribute("a", "1");
    "#,
    );
    assert_eq!(messages, ["watching:1"]);
}

#[test]
fn disconnect_stops_delivery_and_options_are_validated() {
    let messages = observe(
        "<div id='root'></div>",
        r#"
        const observer = new MutationObserver(() => say("callback"));
        observer.observe(root, { attributes: true });
        root.setAttribute("a", "1");
        observer.disconnect();
        root.setAttribute("a", "2");
        try { observer.observe(root, {}); } catch (error) { say(error.constructor.name); }
        try { observer.observe(root, { attributes: false, attributeOldValue: true }); } catch (error) { say(error.constructor.name); }
        try { new MutationObserver(1); } catch (error) { say(error.constructor.name); }
        say(Object.prototype.toString.call(observer));
    "#,
    );
    assert_eq!(
        messages,
        [
            "TypeError",
            "TypeError",
            "TypeError",
            "[object MutationObserver]"
        ]
    );
}

#[test]
fn moving_a_node_reports_removal_then_insertion() {
    let messages = observe(
        "<div id='left'><p id='p'></p></div><div id='right'></div>",
        r#"
        new MutationObserver((records) => records.forEach((record) => say(describe(record))))
            .observe(document.body, { childList: true, subtree: true });
        right.appendChild(p);
    "#,
    );
    assert_eq!(
        messages,
        [
            "childList|left||p|null|null||",
            "childList|right|p||null|null||"
        ]
    );
}

#[test]
fn callbacks_can_observe_their_own_mutations_in_a_later_microtask() {
    let messages = observe(
        "<div id='root'></div>",
        r#"
        let rounds = 0;
        new MutationObserver((records) => {
            rounds += 1;
            say("round" + rounds + ":" + records.length);
            if (rounds === 1) root.setAttribute("b", "1");
        }).observe(root, { attributes: true });
        root.setAttribute("a", "1");
        Promise.resolve().then(() => say("microtask"));
    "#,
    );
    assert_eq!(messages, ["round1:1", "microtask", "round2:1"]);
}
