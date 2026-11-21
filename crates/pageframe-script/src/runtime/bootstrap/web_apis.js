    if (typeof globalThis.performance !== "object") {
        const now = __pageframe_performance_now;
        const timeOrigin = Date.now() - now();
        const entries = [];
        class PerformanceEntry {
            constructor(name, entryType, startTime, duration, detail) {
                Object.defineProperties(this, {
                    name: { value: name, enumerable: true },
                    entryType: { value: entryType, enumerable: true },
                    startTime: { value: startTime, enumerable: true },
                    duration: { value: duration, enumerable: true },
                    detail: { value: detail ?? null, enumerable: true },
                });
            }
            toJSON() {
                return { name: this.name, entryType: this.entryType, startTime: this.startTime, duration: this.duration };
            }
        }
        const markTime = (value) => {
            if (value === undefined) return undefined;
            if (typeof value === "number") return value;
            const mark = entries.findLast((entry) => entry.entryType === "mark" && entry.name === String(value));
            if (!mark) {
                throw new DOMException("Failed to execute 'measure' on 'Performance': The mark '" + value + "' does not exist.", "SyntaxError");
            }
            return mark.startTime;
        };
        const performance = {
            timeOrigin,
            now: () => now(),
            mark(name, options) {
                const entry = new PerformanceEntry(String(name), "mark", options?.startTime ?? now(), 0, options?.detail);
                entries.push(entry);
                return entry;
            },
            measure(name, startOrOptions, endMark) {
                const options = typeof startOrOptions === "object" && startOrOptions !== null ? startOrOptions : null;
                const start = markTime(options ? options.start : startOrOptions) ?? 0;
                const end = markTime(options ? options.end : endMark) ?? now();
                const entry = new PerformanceEntry(String(name), "measure", start, end - start, options?.detail);
                entries.push(entry);
                return entry;
            },
            getEntries: () => entries.slice(),
            getEntriesByType: (type) => entries.filter((entry) => entry.entryType === type),
            getEntriesByName: (name, type) =>
                entries.filter((entry) => entry.name === name && (type === undefined || entry.entryType === type)),
            clearMarks(name) {
                for (let i = entries.length - 1; i >= 0; i--) {
                    if (entries[i].entryType === "mark" && (name === undefined || entries[i].name === name)) entries.splice(i, 1);
                }
            },
            clearMeasures(name) {
                for (let i = entries.length - 1; i >= 0; i--) {
                    if (entries[i].entryType === "measure" && (name === undefined || entries[i].name === name)) entries.splice(i, 1);
                }
            },
            toJSON() { return { timeOrigin }; },
        };
        globalThis.PerformanceEntry = PerformanceEntry;
        globalThis.performance = performance;
    }

    {
        const makeStorage = () => {
            const data = new Map();
            const api = {
                get length() { return data.size; },
                key(index) { return [...data.keys()][Number(index)] ?? null; },
                getItem(key) { return data.has(String(key)) ? data.get(String(key)) : null; },
                setItem(key, value) { data.set(String(key), String(value)); },
                removeItem(key) { data.delete(String(key)); },
                clear() { data.clear(); },
            };
            return new Proxy(api, {
                get(target, prop) {
                    if (typeof prop !== "string" || prop in target) return Reflect.get(target, prop);
                    return data.has(prop) ? data.get(prop) : undefined;
                },
                set(target, prop, value) {
                    if (typeof prop !== "string" || prop in target) return Reflect.set(target, prop, value);
                    data.set(prop, String(value));
                    return true;
                },
                has(target, prop) { return prop in target || data.has(prop); },
                deleteProperty(target, prop) {
                    if (typeof prop === "string") data.delete(prop);
                    return true;
                },
                ownKeys() { return [...data.keys()]; },
                getOwnPropertyDescriptor(target, prop) {
                    if (typeof prop === "string" && data.has(prop)) {
                        return { value: data.get(prop), writable: true, enumerable: true, configurable: true };
                    }
                    return undefined;
                },
            });
        };
        globalThis.Storage = function Storage() {
            throw new TypeError("Illegal constructor");
        };
        if (typeof globalThis.localStorage === "undefined") globalThis.localStorage = makeStorage();
        if (typeof globalThis.sessionStorage === "undefined") globalThis.sessionStorage = makeStorage();
    }

    {
        const env = () => {
            const [width, height, resolution, dark] = __pageframe_media_env();
            return { width, height, resolution, dark };
        };
        const lengthPx = (text) => {
            const match = /^(-?[\d.]+)(px|em|rem)?$/.exec(text.trim());
            if (!match) return NaN;
            const value = Number(match[1]);
            return match[2] === "em" || match[2] === "rem" ? value * 16 : value;
        };
        const resolutionValue = (text) => {
            const match = /^([\d.]+)(dppx|x|dpi|dpcm)$/.exec(text.trim());
            if (!match) return NaN;
            const value = Number(match[1]);
            return { dppx: value, x: value, dpi: value / 96, dpcm: value * 2.54 / 96 }[match[2]];
        };
        const ratio = (text) => {
            const [a, b] = text.split("/").map((part) => Number(part.trim()));
            return b === undefined ? a : a / b;
        };
        const feature = (name, value, values) => {
            const compare = (actual, expected, prefix) =>
                prefix === "min" ? actual >= expected : prefix === "max" ? actual <= expected : actual === expected;
            const match = /^(min-|max-)?(.+)$/.exec(name);
            const prefix = match[1] ? match[1].slice(0, 3) : "";
            const base = match[2];
            switch (base) {
                case "width": return value === undefined ? values.width > 0 : compare(values.width, lengthPx(value), prefix);
                case "height": return value === undefined ? values.height > 0 : compare(values.height, lengthPx(value), prefix);
                case "aspect-ratio": return compare(values.width / values.height, ratio(value), prefix);
                case "resolution": return compare(values.resolution, resolutionValue(value), prefix);
                case "-webkit-device-pixel-ratio": return compare(values.resolution, Number(value), prefix);
                case "orientation": return value === (values.height >= values.width ? "portrait" : "landscape");
                case "prefers-color-scheme": return value === undefined || value === (values.dark ? "dark" : "light");
                case "prefers-reduced-motion":
                case "prefers-reduced-transparency":
                case "prefers-reduced-data":
                    return value === undefined ? false : value === "no-preference";
                case "prefers-contrast": return value === undefined ? false : value === "no-preference";
                case "forced-colors": return value === undefined ? false : value === "none";
                case "inverted-colors": return value === undefined ? false : value === "none";
                case "pointer":
                case "any-pointer":
                    return value === undefined || value === "fine";
                case "hover":
                case "any-hover":
                    return value === undefined || value === "hover";
                case "color": return value === undefined ? true : compare(8, Number(value), prefix);
                case "monochrome": return value === undefined ? false : compare(0, Number(value), prefix);
                case "display-mode": return value === "browser";
                case "scripting": return value === undefined || value === "enabled";
                case "update": return value === undefined || value === "fast";
                case "dynamic-range":
                case "video-dynamic-range":
                    return value === "standard";
                case "color-gamut": return value === "srgb";
                default: return null;
            }
        };
        const evaluateCondition = (text, values) => {
            const trimmed = text.trim();
            if (trimmed === "") return true;
            const parts = trimmed.split(/\s+and\s+/i);
            let typeOk = true;
            let negate = false;
            for (let index = 0; index < parts.length; index++) {
                let part = parts[index].trim();
                if (index === 0) {
                    const typed = /^(not\s+|only\s+)?(all|screen|print|speech|tty|tv|projection|handheld|braille|embossed|aural)\b\s*(.*)$/i.exec(part);
                    if (typed) {
                        negate = /^not/i.test(typed[1] ?? "");
                        const mediaType = typed[2].toLowerCase();
                        typeOk = mediaType === "all" || mediaType === "screen";
                        part = typed[3];
                        if (part === "") continue;
                    }
                }
                const notFeature = /^not\s+\((.*)\)$/i.exec(part);
                const inner = notFeature ? notFeature[1] : /^\((.*)\)$/.exec(part)?.[1];
                if (inner === undefined) return false;
                const colon = inner.indexOf(":");
                const name = (colon < 0 ? inner : inner.slice(0, colon)).trim().toLowerCase();
                const value = colon < 0 ? undefined : inner.slice(colon + 1).trim().toLowerCase();
                const result = feature(name, value, values);
                if (result === null) return false;
                if (notFeature ? result : !result) typeOk = false;
            }
            return negate ? !typeOk : typeOk;
        };
        const evaluate = (query) => {
            const values = env();
            return String(query).split(",").some((part) => evaluateCondition(part, values));
        };
        const lists = new Set();
        class MediaQueryList extends EventTarget {
            #media;
            #matches;
            onchange = null;
            constructor(media) {
                super();
                this.#media = media;
                this.#matches = evaluate(media);
            }
            get media() { return this.#media; }
            get matches() { return evaluate(this.#media); }
            addListener(callback) {
                if (typeof callback === "function") this.addEventListener("change", callback);
            }
            removeListener(callback) { this.removeEventListener("change", callback); }
            addEventListener(type, callback, options) {
                super.addEventListener(type, callback, options);
                lists.add(this);
            }
            refresh() {
                const matches = evaluate(this.#media);
                if (matches === this.#matches) return;
                this.#matches = matches;
                this.dispatchEvent(new MediaQueryListEvent("change", { media: this.#media, matches }));
            }
        }
        const refresh = MediaQueryList.prototype.refresh;
        delete MediaQueryList.prototype.refresh;
        globalThis.MediaQueryList = MediaQueryList;
        globalThis.__pageframe_media_changed = () => {
            for (const list of lists) refresh.call(list);
        };
        globalThis.matchMedia = function matchMedia(query) {
            if (arguments.length === 0) {
                throw new TypeError("Failed to execute 'matchMedia' on 'Window': 1 argument required, but only 0 present.");
            }
            const media = String(query).trim().replace(/\s+/g, " ").toLowerCase();
            return new MediaQueryList(media === "" ? "" : media);
        };
    }

    if (typeof globalThis.crypto !== "object") {
        const fill = (array) => {
            if (!ArrayBuffer.isView(array) || array instanceof Float32Array || array instanceof Float64Array || array instanceof DataView) {
                throw new DOMException("Failed to execute 'getRandomValues' on 'Crypto': The provided ArrayBufferView is of type '" + (array?.constructor?.name ?? typeof array) + "', which is not an integer array type.", "TypeMismatchError");
            }
            if (array.byteLength > 65536) {
                throw new DOMException("Failed to execute 'getRandomValues' on 'Crypto': The ArrayBufferView's byte length (" + array.byteLength + ") exceeds the number of bytes of entropy available via this API (65536).", "QuotaExceededError");
            }
            const bytes = __pageframe_random_bytes(array.byteLength);
            new Uint8Array(array.buffer, array.byteOffset, array.byteLength).set(bytes);
            return array;
        };
        globalThis.crypto = {
            getRandomValues: fill,
            randomUUID() {
                const bytes = fill(new Uint8Array(16));
                bytes[6] = (bytes[6] & 0x0f) | 0x40;
                bytes[8] = (bytes[8] & 0x3f) | 0x80;
                const hex = [...bytes].map((byte) => byte.toString(16).padStart(2, "0")).join("");
                return hex.slice(0, 8) + "-" + hex.slice(8, 12) + "-" + hex.slice(12, 16) + "-" + hex.slice(16, 20) + "-" + hex.slice(20);
            },
        };
    }

    if (typeof globalThis.requestIdleCallback !== "function") {
        globalThis.requestIdleCallback = function requestIdleCallback(callback, options) {
            if (typeof callback !== "function") {
                throw new TypeError("Failed to execute 'requestIdleCallback' on 'Window': parameter 1 is not of type 'IdleRequestCallback'.");
            }
            const timeout = Number(options?.timeout) || 0;
            const start = performance.now();
            return setTimeout(() => {
                const didTimeout = timeout > 0 && performance.now() - start >= timeout;
                callback({ didTimeout, timeRemaining: () => Math.max(0, 50 - (performance.now() - start - 1)) });
            }, 1);
        };
        globalThis.cancelIdleCallback = function cancelIdleCallback(handle) {
            clearTimeout(handle);
        };
    }

    if (typeof globalThis.Blob !== "function") {
        const blobBytes = new WeakMap();
        const partBytes = (part) => {
            if (part instanceof Blob) return blobBytes.get(part);
            if (part instanceof ArrayBuffer) return new Uint8Array(part.slice(0));
            if (ArrayBuffer.isView(part)) return new Uint8Array(part.buffer.slice(part.byteOffset, part.byteOffset + part.byteLength));
            return new TextEncoder().encode(String(part));
        };
        class Blob {
            #type;
            constructor(parts = [], options = {}) {
                const chunks = [...parts].map(partBytes);
                const bytes = new Uint8Array(chunks.reduce((total, chunk) => total + chunk.length, 0));
                let offset = 0;
                for (const chunk of chunks) {
                    bytes.set(chunk, offset);
                    offset += chunk.length;
                }
                blobBytes.set(this, bytes);
                const type = String(options?.type ?? "");
                this.#type = /^[\x20-\x7e]*$/.test(type) ? type.toLowerCase() : "";
            }
            get size() { return blobBytes.get(this).length; }
            get type() { return this.#type; }
            slice(start = 0, end = this.size, type = "") {
                const size = this.size;
                const clamp = (value) => (value < 0 ? Math.max(size + value, 0) : Math.min(value, size));
                return new Blob([blobBytes.get(this).slice(clamp(start), clamp(end))], { type });
            }
            text() { return Promise.resolve(new TextDecoder().decode(blobBytes.get(this))); }
            arrayBuffer() { return Promise.resolve(blobBytes.get(this).slice().buffer); }
            bytes() { return Promise.resolve(blobBytes.get(this).slice()); }
            get [Symbol.toStringTag]() { return "Blob"; }
        }
        class File extends Blob {
            #name;
            #lastModified;
            constructor(parts, name, options = {}) {
                super(parts, options);
                this.#name = String(name);
                this.#lastModified = options?.lastModified === undefined ? Date.now() : Number(options.lastModified);
            }
            get name() { return this.#name; }
            get lastModified() { return this.#lastModified; }
            get webkitRelativePath() { return ""; }
            get [Symbol.toStringTag]() { return "File"; }
        }
        class FileReader extends EventTarget {
            static EMPTY = 0;
            static LOADING = 1;
            static DONE = 2;
            #readyState = 0;
            #result = null;
            #error = null;
            #pending = null;
            onloadstart = null;
            onprogress = null;
            onload = null;
            onloadend = null;
            onerror = null;
            onabort = null;
            get readyState() { return this.#readyState; }
            get result() { return this.#result; }
            get error() { return this.#error; }
            #read(blob, convert) {
                if (!(blob instanceof Blob)) {
                    throw new TypeError("Failed to execute 'read' on 'FileReader': parameter 1 is not of type 'Blob'.");
                }
                if (this.#readyState === 1) {
                    throw new DOMException("Failed to execute 'read' on 'FileReader': The object is already busy reading Blobs.", "InvalidStateError");
                }
                this.#readyState = 1;
                this.#result = null;
                const bytes = blobBytes.get(blob);
                const fire = (type) => this.dispatchEvent(new ProgressEvent(type, { lengthComputable: true, loaded: bytes.length, total: bytes.length }));
                const token = {};
                this.#pending = token;
                setTimeout(() => {
                    if (this.#pending !== token) return;
                    fire("loadstart");
                    this.#result = convert(bytes, blob);
                    this.#readyState = 2;
                    this.#pending = null;
                    fire("progress");
                    fire("load");
                    fire("loadend");
                }, 0);
            }
            readAsArrayBuffer(blob) { this.#read(blob, (bytes) => bytes.slice().buffer); }
            readAsText(blob, label) { this.#read(blob, (bytes) => new TextDecoder(label ?? "utf-8").decode(bytes)); }
            readAsBinaryString(blob) { this.#read(blob, (bytes) => Array.from(bytes, (byte) => String.fromCharCode(byte)).join("")); }
            readAsDataURL(blob) {
                this.#read(blob, (bytes, source) => {
                    let binary = "";
                    for (let i = 0; i < bytes.length; i += 0x8000) {
                        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
                    }
                    return "data:" + (source.type || "application/octet-stream") + ";base64," + btoa(binary);
                });
            }
            abort() {
                if (this.#readyState !== 1) return;
                this.#pending = null;
                this.#readyState = 2;
                this.#result = null;
                this.dispatchEvent(new ProgressEvent("abort"));
                this.dispatchEvent(new ProgressEvent("loadend"));
            }
        }
        globalThis.Blob = Blob;
        globalThis.File = File;
        globalThis.FileReader = FileReader;
        globalThis.__pageframe_blob_bytes = (blob) => blobBytes.get(blob);
    }
