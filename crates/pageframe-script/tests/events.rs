use pageframe_dom::{Document, DocumentConfig};
use pageframe_script::ScriptDocument;

fn run(html: &str) -> (String, Vec<String>) {
    let mut doc = ScriptDocument::from_html(html, DocumentConfig::default()).without_timer_thread();
    doc.execute_scripts();
    let errors = doc.take_js_errors();
    let inner = doc.inner();
    let id = inner.query_selector("#out").unwrap().unwrap();
    (inner.get_node(id).unwrap().text_content(), errors)
}

#[test]
fn event_constructors_build_events() {
    let (text, errors) = run(r#"<div id="out"></div><script>
            const plain = new Event("ping", { bubbles: true, cancelable: true });
            const custom = new CustomEvent("note", { detail: { n: 4 } });
            const pointer = new PointerEvent("pointerdown", { pointerId: 7, pointerType: "pen", clientX: 12, isPrimary: true });
            const message = new MessageEvent("message", { data: "hi", lastEventId: "3" });
            const missing = [];
            try { new Event(); } catch (error) { missing.push(error.constructor.name); }
            try { Event("x"); } catch (error) { missing.push(error.constructor.name); }
            document.getElementById("out").textContent = [
                plain.type, plain.bubbles, plain.cancelable, plain.isTrusted, plain.eventPhase, plain.target,
                custom.detail.n, custom instanceof Event, custom instanceof CustomEvent,
                pointer.pointerId, pointer.pointerType, pointer.clientX, pointer.x, pointer.isPrimary, pointer.button,
                pointer instanceof MouseEvent, pointer instanceof UIEvent,
                message.data, message.lastEventId, message.origin === "",
                new CustomEvent("x").detail, Event.AT_TARGET, typeof plain.timeStamp,
                ...missing,
            ].join(",");
        </script>"#);
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        text,
        "ping,true,true,false,0,,4,true,true,7,pen,12,12,true,0,true,true,hi,3,true,,2,number,TypeError,TypeError"
    );
}

#[test]
fn dispatch_event_bubbles_through_nodes_and_window() {
    let (text, errors) = run(
        r#"<div id="outer"><span id="inner"></span></div><div id="out"></div><script>
            const seen = [];
            const inner = document.getElementById("inner");
            const outer = document.getElementById("outer");
            inner.addEventListener("ping", (e) => seen.push("inner:" + e.eventPhase + ":" + (e.currentTarget === inner)));
            outer.addEventListener("ping", (e) => seen.push("outer:" + e.eventPhase + ":" + (e.target === inner)));
            document.addEventListener("ping", () => seen.push("document"));
            window.addEventListener("ping", (e) => seen.push("window:" + (e.currentTarget === window)));
            const result = inner.dispatchEvent(new CustomEvent("ping", { bubbles: true, detail: 1 }));
            seen.push(result);
            inner.dispatchEvent(new Event("ping"));
            const stop = new Event("ping", { bubbles: true });
            inner.addEventListener("ping", (e) => e.stopPropagation(), { once: true });
            inner.dispatchEvent(stop);
            let thrown = "";
            try { inner.dispatchEvent({ type: "ping" }); } catch (error) { thrown = error.constructor.name; }
            seen.push(thrown);
            document.getElementById("out").textContent = seen.join(" ");
        </script>"#,
    );
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        text,
        "inner:2:true outer:3:true document window:true true inner:2:true inner:2:true TypeError"
    );
}

#[test]
fn prevent_default_only_applies_to_cancelable_events() {
    let (text, errors) = run(r#"<div id="out"></div><script>
            const out = document.getElementById("out");
            out.addEventListener("go", (e) => e.preventDefault());
            const cancelable = out.dispatchEvent(new Event("go", { cancelable: true }));
            const plain = out.dispatchEvent(new Event("go"));
            const event = new Event("go", { cancelable: true });
            out.dispatchEvent(event);
            window.addEventListener("hello", (e) => e.preventDefault());
            const fromWindow = window.dispatchEvent(new Event("hello", { cancelable: true }));
            out.textContent = [cancelable, plain, event.defaultPrevented, event.returnValue, fromWindow].join(",");
        </script>"#);
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(text, "false,true,true,false,false");
}

#[test]
fn event_handler_properties_receive_dispatched_events() {
    let (text, errors) = run(r#"<div id="out"></div><script>
            const out = document.getElementById("out");
            const seen = [];
            out.onclick = (e) => seen.push("onclick " + e.type);
            out.dispatchEvent(new MouseEvent("click", { bubbles: true, clientX: 3 }));
            window.onmessage = (e) => seen.push("message " + e.data);
            window.dispatchEvent(new MessageEvent("message", { data: 5 }));
            out.textContent = seen.join("|");
        </script>"#);
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(text, "onclick click|message 5");
}

#[test]
fn event_target_can_be_constructed_and_subclassed() {
    let (text, errors) = run(r#"<div id="out"></div><script>
            class Bus extends EventTarget {}
            const bus = new Bus();
            const seen = [];
            const once = () => seen.push("once");
            bus.addEventListener("tick", once, { once: true });
            bus.addEventListener("tick", { handleEvent: (e) => seen.push("object " + (e.target === bus)) });
            bus.dispatchEvent(new Event("tick"));
            bus.dispatchEvent(new Event("tick"));
            const removed = () => seen.push("removed");
            bus.addEventListener("tick", removed);
            bus.removeEventListener("tick", removed);
            bus.dispatchEvent(new Event("tick"));
            document.getElementById("out").textContent = [
                seen.join(" "),
                bus instanceof EventTarget,
                document.body instanceof EventTarget,
                window instanceof EventTarget,
                ({}) instanceof EventTarget,
            ].join(",");
        </script>"#);
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        text,
        "once object true object true object true,true,true,true,false"
    );
}

#[test]
fn redispatching_an_event_in_flight_throws() {
    let (text, errors) = run(r#"<div id="target"></div><div id="out"></div><script>
            const target = document.getElementById("target");
            const custom = new EventTarget();
            const seen = [];
            target.addEventListener("ping", (event) => {
                try { target.dispatchEvent(event); } catch (error) { seen.push(error.name); }
                try { custom.dispatchEvent(event); } catch (error) { seen.push(error.name); }
            }, { once: true });
            custom.addEventListener("pong", (event) => {
                try { custom.dispatchEvent(event); } catch (error) { seen.push(error.name); }
            });
            const ping = new Event("ping");
            target.dispatchEvent(ping);
            custom.dispatchEvent(new Event("pong"));
            seen.push(target.dispatchEvent(ping));
            document.getElementById("out").textContent = seen.join(",");
        </script>"#);
    assert_eq!(errors, Vec::<String>::new());
    assert_eq!(
        text,
        "InvalidStateError,InvalidStateError,InvalidStateError,true"
    );
}
