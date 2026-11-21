    {
        const send = __pageframe_ipc_send;
        const pending = new Map();
        const channels = new Map();
        const parse = (json) => (json === undefined || json === "" ? undefined : JSON.parse(json));
        const ipc = {
            invoke(channel, payload) {
                let json;
                try {
                    json = JSON.stringify(payload === undefined ? null : payload);
                } catch (error) {
                    return Promise.reject(error);
                }
                const name = String(channel);
                return new Promise((resolve, reject) => {
                    pending.set(send("invoke", name, json), { resolve, reject });
                });
            },
            on(channel, listener) {
                if (typeof listener !== "function") throw new TypeError("ipc.on: listener must be a function");
                const name = String(channel);
                let set = channels.get(name);
                if (!set) {
                    set = new Set();
                    channels.set(name, set);
                }
                const entry = { listener };
                set.add(entry);
                return () => {
                    set.delete(entry);
                };
            },
            postMessage(message) {
                send("post", "", String(message));
            },
        };
        Object.freeze(ipc);
        Object.defineProperty(globalThis, "ipc", { value: ipc, writable: true, configurable: true });
        Object.defineProperty(globalThis, "__pageframe_ipc_settle", {
            value: function settle(id, ok, value) {
                const entry = pending.get(id);
                if (!entry) return;
                pending.delete(id);
                if (!ok) {
                    entry.reject(new Error(String(value)));
                    return;
                }
                try {
                    entry.resolve(parse(value));
                } catch (error) {
                    entry.reject(error);
                }
            },
            configurable: true,
        });
        Object.defineProperty(globalThis, "__pageframe_window_event", {
            value: (name) => globalThis.dispatchEvent(new Event(name)),
            configurable: true,
        });
        Object.defineProperty(globalThis, "__pageframe_ipc_emit", {
            value: function emit(channel, json) {
                const set = channels.get(channel);
                if (!set || set.size === 0) return;
                const payload = parse(json);
                for (const entry of [...set]) {
                    if (!set.has(entry)) continue;
                    try {
                        entry.listener.call(globalThis, payload);
                    } catch (error) {
                        if (typeof globalThis.reportError === "function") globalThis.reportError(error);
                    }
                }
            },
            configurable: true,
        });
    }
