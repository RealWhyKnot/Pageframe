    if (typeof globalThis.EventSource !== "function") {
        const makeEvent = (target, type, init) => ({ type, target, currentTarget: target, ...init });
        class EventSource {
            static CONNECTING = 0;
            static OPEN = 1;
            static CLOSED = 2;
            CONNECTING = 0;
            OPEN = 1;
            CLOSED = 2;
            onopen = null;
            onmessage = null;
            onerror = null;
            #url;
            #withCredentials;
            #readyState = 0;
            #listeners = new Map();
            #id = null;
            #lastEventId = "";
            #retry = 3000;
            #timer = null;
            constructor(url, init) {
                this.#url = new URL(String(url), document.baseURI ?? location.href).href;
                this.#withCredentials = init?.withCredentials === true;
                this.#connect();
            }
            get url() { return this.#url; }
            get withCredentials() { return this.#withCredentials; }
            get readyState() { return this.#readyState; }
            addEventListener(type, callback, options) {
                if (typeof callback !== "function" && !(callback && typeof callback.handleEvent === "function")) return;
                const once = typeof options === "object" && options !== null && options.once === true;
                const list = this.#listeners.get(type) ?? [];
                if (list.some((entry) => entry.callback === callback)) return;
                list.push({ callback, once });
                this.#listeners.set(type, list);
            }
            removeEventListener(type, callback) {
                const list = this.#listeners.get(type);
                if (list) this.#listeners.set(type, list.filter((entry) => entry.callback !== callback));
            }
            dispatchEvent(event) {
                const handler = this["on" + event.type];
                if (typeof handler === "function") {
                    try { handler.call(this, event); } catch (e) { setTimeout(() => { throw e; }, 0); }
                }
                const list = this.#listeners.get(event.type) ?? [];
                this.#listeners.set(event.type, list.filter((entry) => !entry.once));
                for (const { callback } of list) {
                    try {
                        if (typeof callback === "function") callback.call(this, event);
                        else callback.handleEvent(event);
                    } catch (e) { setTimeout(() => { throw e; }, 0); }
                }
                return true;
            }
            close() {
                this.#readyState = 2;
                if (this.#timer !== null) {
                    clearTimeout(this.#timer);
                    this.#timer = null;
                }
                if (this.#id !== null) {
                    __pageframe_net_forget(this.#id);
                    __pageframe_event_source_close(this.#id);
                    this.#id = null;
                }
            }
            #connect() {
                this.#timer = null;
                this.#readyState = 0;
                let id;
                try {
                    id = __pageframe_event_source_open(this.#url, this.#lastEventId);
                } catch (e) {
                    this.#fail(false);
                    return;
                }
                this.#id = id;
                __pageframe_net_listen(id, (kind, ...rest) => this.#deliver(kind, rest));
            }
            #deliver(kind, rest) {
                if (this.#readyState === 2) return;
                if (kind === "open") {
                    this.#readyState = 1;
                    this.dispatchEvent(makeEvent(this, "open"));
                } else if (kind === "message") {
                    const [type, data, lastEventId] = rest;
                    this.#lastEventId = lastEventId;
                    const origin = new URL(this.#url).origin;
                    this.dispatchEvent(makeEvent(this, type, { data, lastEventId, origin }));
                } else if (kind === "retry") {
                    this.#retry = rest[0];
                } else if (kind === "end") {
                    const [, reconnect] = rest;
                    __pageframe_net_forget(this.#id);
                    this.#id = null;
                    this.#fail(reconnect === true);
                }
            }
            #fail(reconnect) {
                if (reconnect) {
                    this.#readyState = 0;
                    this.dispatchEvent(makeEvent(this, "error"));
                    if (this.#readyState !== 2) {
                        this.#timer = setTimeout(() => this.#connect(), this.#retry);
                    }
                } else {
                    this.#readyState = 2;
                    this.dispatchEvent(makeEvent(this, "error"));
                }
            }
        }
        globalThis.EventSource = EventSource;
    }
