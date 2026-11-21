    const netCallbacks = new Map();
    globalThis.__pageframe_net_deliver = function (id, kind, ...rest) {
        const callback = netCallbacks.get(id);
        if (callback) callback(kind, ...rest);
    };
    globalThis.__pageframe_net_listen = function (id, callback) {
        netCallbacks.set(id, callback);
    };
    globalThis.__pageframe_net_forget = function (id) {
        netCallbacks.delete(id);
    };

    if (typeof globalThis.AbortSignal !== "function") {
        const signalState = new WeakMap();
        const abortSignal = (signal, reason) => {
            const state = signalState.get(signal);
            if (state.aborted) return;
            state.aborted = true;
            state.reason = reason;
            signal.dispatchEvent({ type: "abort", target: signal, currentTarget: signal });
        };
        class AbortSignal {
            onabort = null;
            constructor() {
                signalState.set(this, { aborted: false, reason: undefined, listeners: [] });
            }
            get aborted() { return signalState.get(this).aborted; }
            get reason() { return signalState.get(this).reason; }
            throwIfAborted() {
                if (this.aborted) throw this.reason;
            }
            addEventListener(type, callback, options) {
                if (type !== "abort" || typeof callback !== "function") return;
                const once = typeof options === "object" && options !== null && options.once === true;
                signalState.get(this).listeners.push({ callback, once });
            }
            removeEventListener(type, callback) {
                if (type !== "abort") return;
                const state = signalState.get(this);
                state.listeners = state.listeners.filter((entry) => entry.callback !== callback);
            }
            dispatchEvent(event) {
                const state = signalState.get(this);
                const entries = state.listeners;
                state.listeners = entries.filter((entry) => !entry.once);
                if (typeof this.onabort === "function") this.onabort.call(this, event);
                for (const entry of entries) entry.callback.call(this, event);
                return true;
            }
            static abort(reason) {
                const controller = new AbortController();
                controller.abort(reason);
                return controller.signal;
            }
            static timeout(ms) {
                const controller = new AbortController();
                setTimeout(() => {
                    controller.abort(new DOMException("The operation timed out.", "TimeoutError"));
                }, Number(ms) || 0);
                return controller.signal;
            }
            static any(signals) {
                const controller = new AbortController();
                for (const signal of signals) {
                    if (signal.aborted) {
                        controller.abort(signal.reason);
                        break;
                    }
                    signal.addEventListener("abort", () => controller.abort(signal.reason), { once: true });
                }
                return controller.signal;
            }
        }
        class AbortController {
            #signal = new AbortSignal();
            get signal() { return this.#signal; }
            abort(reason) {
                abortSignal(this.#signal, reason === undefined
                    ? new DOMException("signal is aborted without reason", "AbortError")
                    : reason);
            }
        }
        globalThis.AbortSignal = AbortSignal;
        globalThis.AbortController = AbortController;
    }

    if (typeof globalThis.fetch !== "function") {
        class Headers {
            #map = new Map();
            constructor(init) {
                if (init instanceof Headers) {
                    for (const [k, v] of init) this.append(k, v);
                } else if (Array.isArray(init)) {
                    for (const [k, v] of init) this.append(k, v);
                } else if (init && typeof init === "object") {
                    for (const k of Object.keys(init)) this.append(k, init[k]);
                }
            }
            append(name, value) {
                const key = String(name).toLowerCase();
                const existing = this.#map.get(key);
                this.#map.set(key, existing === undefined ? String(value) : existing + ", " + value);
            }
            set(name, value) { this.#map.set(String(name).toLowerCase(), String(value)); }
            get(name) { return this.#map.get(String(name).toLowerCase()) ?? null; }
            has(name) { return this.#map.has(String(name).toLowerCase()); }
            delete(name) { this.#map.delete(String(name).toLowerCase()); }
            forEach(cb, thisArg) { for (const [k, v] of this) cb.call(thisArg, v, k, this); }
            *entries() { yield* [...this.#map.entries()].sort((a, b) => (a[0] < b[0] ? -1 : 1)); }
            *keys() { for (const [k] of this.entries()) yield k; }
            *values() { for (const [, v] of this.entries()) yield v; }
            [Symbol.iterator]() { return this.entries(); }
        }

        const toBytes = (body) => {
            if (body === undefined || body === null) return new Uint8Array(0);
            if (typeof body === "string") return new TextEncoder().encode(body);
            if (body instanceof URLSearchParams) return new TextEncoder().encode(body.toString());
            if (body instanceof ArrayBuffer) return new Uint8Array(body.slice(0));
            if (ArrayBuffer.isView(body)) {
                return new Uint8Array(body.buffer.slice(body.byteOffset, body.byteOffset + body.byteLength));
            }
            if (typeof Blob === "function" && body instanceof Blob) return globalThis.__pageframe_blob_bytes(body).slice();
            return new TextEncoder().encode(String(body));
        };

        class Response {
            #body;
            #bodyUsed = false;
            constructor(body = null, init = {}) {
                this.#body = toBytes(body);
                const status = init.status === undefined ? 200 : Number(init.status);
                if (!Number.isInteger(status) || status < 200 || status > 599) {
                    throw new RangeError("The status provided (" + status + ") is outside the range [200, 599].");
                }
                Object.defineProperties(this, {
                    status: { value: status, enumerable: true },
                    statusText: { value: init.statusText === undefined ? "" : String(init.statusText), enumerable: true },
                    headers: { value: new Headers(init.headers), enumerable: true },
                    url: { value: init.url === undefined ? "" : String(init.url), enumerable: true },
                    type: { value: "basic", enumerable: true },
                    redirected: { value: init.redirected === true, enumerable: true },
                });
            }
            get ok() { return this.status >= 200 && this.status <= 299; }
            get bodyUsed() { return this.#bodyUsed; }
            #consume() {
                if (this.#bodyUsed) {
                    return Promise.reject(new TypeError("body stream already read"));
                }
                this.#bodyUsed = true;
                return Promise.resolve(this.#body);
            }
            text() { return this.#consume().then((bytes) => new TextDecoder().decode(bytes)); }
            json() { return this.text().then((text) => JSON.parse(text)); }
            arrayBuffer() { return this.#consume().then((bytes) => bytes.slice().buffer); }
            bytes() { return this.#consume().then((bytes) => bytes.slice()); }
            blob() {
                const type = this.headers.get("content-type") ?? "";
                return this.#consume().then((bytes) => new Blob([bytes], { type }));
            }
            clone() {
                if (this.#bodyUsed) throw new TypeError("Response body is already used");
                return new Response(this.#body, {
                    status: this.status, statusText: this.statusText, headers: this.headers, url: this.url,
                    redirected: this.redirected,
                });
            }
            static json(data, init = {}) {
                const headers = new Headers(init.headers);
                if (!headers.has("content-type")) headers.set("content-type", "application/json");
                return new Response(JSON.stringify(data), { ...init, headers });
            }
        }

        const requestBody = (body) => {
            if (body === undefined || body === null) return "";
            if (typeof body === "string") return body;
            if (body instanceof URLSearchParams) return body.toString();
            return toBytes(body).buffer;
        };

        const toResponse = (method, [status, statusText, url, redirected, headers, text]) => {
            const response = new Response(method === "HEAD" ? null : text, {
                status: status >= 200 && status <= 599 ? status : 200,
                statusText,
                headers,
                url,
                redirected,
            });
            return response;
        };

        globalThis.Headers = Headers;
        globalThis.Response = Response;
        globalThis.fetch = function fetch(input, init) {
            return new Promise((resolve, reject) => {
                let url;
                try {
                    url = input && typeof input === "object" && "url" in input ? String(input.url) : String(input);
                } catch (e) {
                    reject(e);
                    return;
                }
                const method = String(init?.method ?? input?.method ?? "GET").toUpperCase();
                const signal = init?.signal ?? input?.signal ?? null;
                if (signal && signal.aborted) {
                    reject(signal.reason);
                    return;
                }
                const headers = new Headers(init?.headers ?? input?.headers);
                const body = init?.body;
                if (body instanceof URLSearchParams && !headers.has("content-type")) {
                    headers.set("content-type", "application/x-www-form-urlencoded;charset=UTF-8");
                } else if (typeof body === "string" && !headers.has("content-type")) {
                    headers.set("content-type", "text/plain;charset=UTF-8");
                } else if (typeof Blob === "function" && body instanceof Blob && body.type && !headers.has("content-type")) {
                    headers.set("content-type", body.type);
                }
                if ((method === "GET" || method === "HEAD") && body !== undefined && body !== null) {
                    reject(new TypeError("Request with GET/HEAD method cannot have body."));
                    return;
                }
                let started;
                try {
                    started = __pageframe_fetch_start(method, url, [...headers], requestBody(body));
                } catch (e) {
                    reject(e);
                    return;
                }
                if (typeof started !== "number") {
                    resolve(toResponse(method, started));
                    return;
                }
                let settled = false;
                const onAbort = () => {
                    if (settled) return;
                    settled = true;
                    __pageframe_net_forget(started);
                    reject(signal.reason);
                };
                if (signal) signal.addEventListener("abort", onAbort, { once: true });
                __pageframe_net_listen(started, (kind, payload) => {
                    __pageframe_net_forget(started);
                    if (signal) signal.removeEventListener("abort", onAbort);
                    if (settled) return;
                    settled = true;
                    if (kind === "response") resolve(toResponse(method, payload));
                    else reject(new TypeError("Failed to fetch: " + payload));
                });
            });
        };
    }
