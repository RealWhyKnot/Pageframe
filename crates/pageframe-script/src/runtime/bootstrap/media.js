    {
        const openNative = __pageframe_media_open;
        const closeNative = __pageframe_media_close;
        const controlNative = __pageframe_media_control;
        const timeNative = __pageframe_media_time;
        const canPlayNative = __pageframe_media_can_play;
        const listenNative = __pageframe_media_listen;
        delete globalThis.__pageframe_media_open;
        delete globalThis.__pageframe_media_close;
        delete globalThis.__pageframe_media_control;
        delete globalThis.__pageframe_media_time;
        delete globalThis.__pageframe_media_can_play;
        delete globalThis.__pageframe_media_listen;

        const elementProto = document.documentElement
            ? Object.getPrototypeOf(document.documentElement)
            : null;
        if (elementProto !== null) {
            const htmlNamespace = "http://www.w3.org/1999/xhtml";
            const isHtml = (element) => (element.namespaceURI ?? htmlNamespace) === htmlNamespace;
            const isMedia = (element) =>
                element !== null && typeof element === "object" && elementProto.isPrototypeOf(element)
                && isHtml(element) && (element.localName === "video" || element.localName === "audio");
            const isVideo = (element) => isMedia(element) && element.localName === "video";

            const NETWORK_EMPTY = 0;
            const NETWORK_IDLE = 1;
            const NETWORK_LOADING = 2;
            const NETWORK_NO_SOURCE = 3;
            const HAVE_NOTHING = 0;
            const HAVE_METADATA = 1;
            const HAVE_CURRENT_DATA = 2;
            const HAVE_FUTURE_DATA = 3;
            const HAVE_ENOUGH_DATA = 4;
            const TIMEUPDATE_INTERVAL = 250;
            const constants = {
                NETWORK_EMPTY, NETWORK_IDLE, NETWORK_LOADING, NETWORK_NO_SOURCE,
                HAVE_NOTHING, HAVE_METADATA, HAVE_CURRENT_DATA, HAVE_FUTURE_DATA, HAVE_ENOUGH_DATA,
            };

            const errorConstants = {
                MEDIA_ERR_ABORTED: 1,
                MEDIA_ERR_NETWORK: 2,
                MEDIA_ERR_DECODE: 3,
                MEDIA_ERR_SRC_NOT_SUPPORTED: 4,
            };
            const errorData = new WeakMap();
            function MediaError() {
                throw new TypeError("Illegal constructor");
            }
            Object.defineProperties(MediaError.prototype, {
                code: { get() { return errorData.get(this)?.code ?? 0; }, configurable: true, enumerable: true },
                message: { get() { return errorData.get(this)?.message ?? ""; }, configurable: true, enumerable: true },
            });
            for (const [name, value] of Object.entries(errorConstants)) {
                Object.defineProperty(MediaError, name, { value, enumerable: true });
                Object.defineProperty(MediaError.prototype, name, { value, enumerable: true });
            }
            globalThis.MediaError = MediaError;
            const mediaError = (code, message) => {
                const error = Object.create(MediaError.prototype);
                errorData.set(error, { code, message: String(message ?? "") });
                return error;
            };

            class TimeRanges {
                #ranges;
                constructor(ranges) {
                    this.#ranges = ranges;
                }
                get length() {
                    return this.#ranges.length;
                }
                start(index) {
                    const range = this.#ranges[index];
                    if (range === undefined) throw new DOMException("The index provided is greater than or equal to the maximum bound.", "IndexSizeError");
                    return range[0];
                }
                end(index) {
                    const range = this.#ranges[index];
                    if (range === undefined) throw new DOMException("The index provided is greater than or equal to the maximum bound.", "IndexSizeError");
                    return range[1];
                }
            }
            const makeRanges = TimeRanges;
            globalThis.TimeRanges = function TimeRanges() {
                throw new TypeError("Illegal constructor");
            };
            globalThis.TimeRanges.prototype = makeRanges.prototype;

            const states = new WeakMap();
            const stateOf = (element) => {
                let state = states.get(element);
                if (!state) {
                    state = {
                        networkState: NETWORK_EMPTY,
                        readyState: HAVE_NOTHING,
                        paused: true,
                        ended: false,
                        seeking: false,
                        time: 0,
                        startAt: 0,
                        duration: NaN,
                        playbackRate: 1,
                        defaultPlaybackRate: 1,
                        volume: 1,
                        muted: element.hasAttribute("muted"),
                        error: null,
                        currentSrc: "",
                        loadedKey: null,
                        open: false,
                        reopen: false,
                        width: 0,
                        height: 0,
                        presented: 0,
                        seekSerial: 0,
                        lastTimeupdate: 0,
                        pendingPlays: [],
                        frameCallbacks: new Map(),
                        nextCallback: 0,
                        loopSent: null,
                        gainSent: null,
                        autoplayed: false,
                        source: null,
                        generation: 0,
                    };
                    states.set(element, state);
                }
                return state;
            };

            const fire = (element, type) => {
                queueMicrotask(() => element.dispatchEvent(new Event(type)));
            };

            const rejectPlays = (state, error) => {
                const pending = state.pendingPlays;
                state.pendingPlays = [];
                for (const play of pending) queueMicrotask(() => play.reject(error));
            };
            const resolvePlays = (state) => {
                const pending = state.pendingPlays;
                state.pendingPlays = [];
                for (const play of pending) queueMicrotask(() => play.resolve());
            };

            const webAudioGain = (state) => (state.source === null ? 1 : state.source.gainValue());

            const syncNative = (element, state) => {
                if (!state.open) return;
                const loop = element.hasAttribute("loop");
                if (state.loopSent !== loop) {
                    state.loopSent = loop;
                    controlNative(element, "loop", loop ? 1 : 0);
                }
                const gain = state.muted ? 0 : state.volume * webAudioGain(state);
                if (state.gainSent !== gain) {
                    state.gainSent = gain;
                    controlNative(element, "gain", gain);
                }
            };

            const closePlayer = (element, state) => {
                if (state.open) closeNative(element);
                state.open = false;
                state.reopen = false;
                state.loopSent = null;
                state.gainSent = null;
            };

            const openPlayer = (element, state, url) => {
                state.open = openNative(element, url);
                state.loopSent = null;
                state.gainSent = null;
                if (!state.open) return false;
                if (state.playbackRate > 0 && state.playbackRate !== 1) {
                    controlNative(element, "rate", state.playbackRate);
                }
                syncNative(element, state);
                return true;
            };

            const failLoad = (element, state, code, message) => {
                state.error = mediaError(code, message);
                state.networkState = code === 4 ? NETWORK_NO_SOURCE : NETWORK_IDLE;
                closePlayer(element, state);
                rejectPlays(state, new DOMException(
                    code === 4 ? "Failed to load because no supported source was found." : String(message),
                    code === 4 ? "NotSupportedError" : "AbortError",
                ));
                fire(element, "error");
            };

            const selectSource = (element) => {
                if (element.hasAttribute("src")) {
                    const raw = element.getAttribute("src");
                    return { key: "src:" + raw, url: raw === "" ? "" : element.src };
                }
                for (const child of element.children) {
                    if (child.localName !== "source" || !child.hasAttribute("src")) continue;
                    const type = child.getAttribute("type");
                    if (type && canPlayNative(type) === "") continue;
                    return { key: "source:" + child.getAttribute("src"), url: child.src };
                }
                return null;
            };

            const loadMedia = (element) => {
                const state = stateOf(element);
                state.generation += 1;
                if (state.pendingPlays.length > 0) {
                    rejectPlays(state, new DOMException("The play() request was interrupted by a new load request.", "AbortError"));
                }
                if (state.networkState !== NETWORK_EMPTY) {
                    fire(element, "emptied");
                }
                closePlayer(element, state);
                state.paused = true;
                state.ended = false;
                state.seeking = false;
                state.readyState = HAVE_NOTHING;
                state.networkState = NETWORK_EMPTY;
                state.error = null;
                state.time = 0;
                state.startAt = 0;
                state.duration = NaN;
                state.width = 0;
                state.height = 0;
                state.presented = 0;
                state.autoplayed = false;
                state.playbackRate = state.defaultPlaybackRate;
                const source = selectSource(element);
                if (source === null) {
                    state.loadedKey = null;
                    state.currentSrc = "";
                    return;
                }
                state.loadedKey = source.key;
                state.currentSrc = source.url;
                state.networkState = NETWORK_LOADING;
                fire(element, "loadstart");
                if (source.url === "") {
                    failLoad(element, state, 4, "Empty src attribute");
                    return;
                }
                if (!openPlayer(element, state, source.url)) {
                    failLoad(element, state, 4, "Unsupported media URL");
                }
            };

            const currentTimeOf = (state, element) => {
                if (state.seeking) return state.time;
                if (state.open) {
                    const time = timeNative(element);
                    if (Number.isFinite(time)) return time;
                }
                return state.time;
            };

            const seekTo = (element, state, target) => {
                let time = Math.max(0, target);
                if (Number.isFinite(state.duration)) time = Math.min(time, state.duration);
                state.time = time;
                state.ended = false;
                if (!state.open) return;
                state.seeking = true;
                state.seekSerial = controlNative(element, "seek", time);
                fire(element, "seeking");
            };

            const playMedia = (element) => {
                const state = stateOf(element);
                if (state.networkState === NETWORK_EMPTY) loadMedia(element);
                if (state.error !== null && state.error.code === 4 || state.networkState === NETWORK_EMPTY) {
                    return Promise.reject(new DOMException("The element has no supported sources.", "NotSupportedError"));
                }
                if (state.reopen && state.currentSrc) {
                    state.reopen = false;
                    state.readyState = Math.min(state.readyState, HAVE_METADATA);
                    if (openPlayer(element, state, state.currentSrc) && state.time > 0) {
                        state.seekSerial = controlNative(element, "seek", state.time);
                    }
                }
                const atEnd = state.ended || (Number.isFinite(state.duration) && currentTimeOf(state, element) >= state.duration);
                if (atEnd && state.readyState >= HAVE_METADATA) seekTo(element, state, 0);
                const promise = new Promise((resolve, reject) => state.pendingPlays.push({ resolve, reject }));
                if (state.paused) {
                    state.paused = false;
                    if (state.open) controlNative(element, "play", 0);
                    fire(element, "play");
                    if (state.readyState >= HAVE_FUTURE_DATA) {
                        fire(element, "playing");
                        resolvePlays(state);
                    }
                } else if (state.readyState >= HAVE_FUTURE_DATA) {
                    resolvePlays(state);
                }
                return promise;
            };

            const pauseMedia = (element) => {
                const state = stateOf(element);
                if (state.networkState === NETWORK_EMPTY && selectSource(element) !== null) loadMedia(element);
                if (state.paused) return;
                state.paused = true;
                if (state.open) controlNative(element, "pause", 0);
                state.time = currentTimeOf(state, element);
                fire(element, "timeupdate");
                fire(element, "pause");
                rejectPlays(state, new DOMException("The play() request was interrupted by a call to pause().", "AbortError"));
            };

            const runFrameCallbacks = (element, state, frame) => {
                if (state.frameCallbacks.size === 0) return;
                const callbacks = [...state.frameCallbacks.values()];
                state.frameCallbacks.clear();
                const now = performance.now();
                const metadata = {
                    presentationTime: now,
                    expectedDisplayTime: now + 1000 / 60,
                    width: frame.width,
                    height: frame.height,
                    mediaTime: frame.mediaTime,
                    presentedFrames: frame.presented,
                    processingDuration: 0,
                };
                for (const callback of callbacks) {
                    try {
                        callback.call(element, now, { ...metadata });
                    } catch (error) {
                        queueMicrotask(() => { throw error; });
                    }
                }
            };

            const reachReadyState = (element, state, target) => {
                if (state.readyState < HAVE_CURRENT_DATA && target >= HAVE_CURRENT_DATA) {
                    state.readyState = HAVE_CURRENT_DATA;
                    fire(element, "loadeddata");
                }
                if (state.readyState < HAVE_FUTURE_DATA && target >= HAVE_FUTURE_DATA) {
                    state.readyState = HAVE_FUTURE_DATA;
                    fire(element, "canplay");
                    if (!state.paused) {
                        fire(element, "playing");
                        resolvePlays(state);
                    }
                }
                if (state.readyState < HAVE_ENOUGH_DATA && target >= HAVE_ENOUGH_DATA) {
                    state.readyState = HAVE_ENOUGH_DATA;
                    fire(element, "canplaythrough");
                }
            };

            const onUpdate = (element, update) => {
                if (update.attach) {
                    const existing = states.get(element);
                    const source = selectSource(element);
                    if (source === null) return;
                    if (existing && existing.loadedKey === source.key) return;
                    loadMedia(element);
                    return;
                }
                const state = stateOf(element);
                if (update.detached) {
                    state.open = false;
                    state.reopen = state.currentSrc !== "" && state.error === null;
                    state.time = update.time;
                    state.seeking = false;
                    if (!state.paused) {
                        state.paused = true;
                        fire(element, "pause");
                        rejectPlays(state, new DOMException("The media element was removed from the document.", "AbortError"));
                    }
                    return;
                }
                if (update.metadata) {
                    const metadata = update.metadata;
                    const first = state.readyState < HAVE_METADATA;
                    state.duration = metadata.duration;
                    if (isVideo(element)) {
                        const resized = state.width !== metadata.width || state.height !== metadata.height;
                        state.width = metadata.width;
                        state.height = metadata.height;
                        if (resized && !first) fire(element, "resize");
                    }
                    if (first) {
                        state.readyState = HAVE_METADATA;
                        fire(element, "durationchange");
                        fire(element, "loadedmetadata");
                        if (state.startAt > 0) {
                            const start = state.startAt;
                            state.startAt = 0;
                            seekTo(element, state, start);
                        }
                    }
                }
                if (update.frame) {
                    const frame = update.frame;
                    state.presented = frame.presented;
                    if (isVideo(element) && (frame.width !== state.width || frame.height !== state.height)) {
                        state.width = frame.width;
                        state.height = frame.height;
                        fire(element, "resize");
                    }
                    if (state.readyState >= HAVE_METADATA) reachReadyState(element, state, HAVE_CURRENT_DATA);
                    runFrameCallbacks(element, state, frame);
                }
                if (update.ready && state.readyState >= HAVE_METADATA) {
                    state.networkState = NETWORK_IDLE;
                    reachReadyState(element, state, HAVE_ENOUGH_DATA);
                    if (state.paused && !state.autoplayed && element.hasAttribute("autoplay")) {
                        state.autoplayed = true;
                        playMedia(element).catch(() => {});
                    }
                }
                if (update.seeked !== undefined && update.seeked === state.seekSerial && state.seeking) {
                    state.seeking = false;
                    state.time = update.time;
                    state.lastTimeupdate = performance.now();
                    fire(element, "timeupdate");
                    fire(element, "seeked");
                }
                if (update.looped) {
                    state.lastTimeupdate = performance.now();
                    fire(element, "seeking");
                    fire(element, "timeupdate");
                    fire(element, "seeked");
                }
                if (update.error) {
                    const code = update.error.code;
                    state.open = false;
                    failLoad(element, state, code, update.error.message);
                    return;
                }
                if (update.ended) {
                    state.ended = true;
                    state.time = update.time;
                    fire(element, "timeupdate");
                    if (!state.paused) {
                        state.paused = true;
                        fire(element, "pause");
                    }
                    fire(element, "ended");
                    return;
                }
                if (update.playing && !state.seeking) {
                    state.time = update.time;
                    const now = performance.now();
                    if (now - state.lastTimeupdate >= TIMEUPDATE_INTERVAL) {
                        state.lastTimeupdate = now;
                        fire(element, "timeupdate");
                    }
                }
                syncNative(element, state);
            };
            listenNative(onUpdate);

            const mediaProto = globalThis.HTMLMediaElement?.prototype;
            const videoProto = globalThis.HTMLVideoElement?.prototype;
            const audioProto = globalThis.HTMLAudioElement?.prototype;
            if (mediaProto && videoProto) Object.setPrototypeOf(videoProto, mediaProto);
            if (mediaProto && audioProto) Object.setPrototypeOf(audioProto, mediaProto);

            const install = (name, descriptor, test, mirror) => {
                const previous = Object.getOwnPropertyDescriptor(elementProto, name);
                const wrapped = { configurable: true, enumerable: true };
                if (descriptor.value !== undefined) {
                    const value = descriptor.value;
                    wrapped.writable = true;
                    wrapped.enumerable = false;
                    wrapped.value = {
                        [name](...args) {
                            if (test(this)) return value.apply(this, args);
                            if (previous?.value) return previous.value.apply(this, args);
                            throw new TypeError("Illegal invocation");
                        },
                    }[name];
                } else {
                    const get = descriptor.get;
                    const set = descriptor.set;
                    wrapped.get = function () {
                        if (test(this)) return get.call(this);
                        if (previous?.get) return previous.get.call(this);
                        return previous && "value" in previous ? previous.value : undefined;
                    };
                    wrapped.set = function (value) {
                        if (test(this)) {
                            if (set) set.call(this, value);
                            return;
                        }
                        if (previous?.set) {
                            previous.set.call(this, value);
                            return;
                        }
                        Object.defineProperty(this, name, { value, writable: true, configurable: true, enumerable: true });
                    };
                }
                Object.defineProperty(elementProto, name, wrapped);
                if (mirror) Object.defineProperty(mirror, name, wrapped);
            };
            const media = (name, descriptor) => install(name, descriptor, isMedia, mediaProto);
            const video = (name, descriptor) => install(name, descriptor, isVideo, videoProto);

            for (const [name, value] of Object.entries(constants)) {
                media(name, { get: () => value });
                if (globalThis.HTMLMediaElement) {
                    Object.defineProperty(globalThis.HTMLMediaElement, name, { value, enumerable: true });
                }
            }

            const reflectBool = (name, after) => media(name, {
                get() { return this.hasAttribute(name); },
                set(value) {
                    if (value) this.setAttribute(name, "");
                    else this.removeAttribute(name);
                    if (after) after(this);
                },
            });
            reflectBool("autoplay");
            reflectBool("controls");
            reflectBool("loop", (element) => syncNative(element, stateOf(element)));
            media("defaultMuted", {
                get() { return this.hasAttribute("muted"); },
                set(value) {
                    if (value) this.setAttribute("muted", "");
                    else this.removeAttribute("muted");
                },
            });
            media("playsInline", {
                get() { return this.hasAttribute("playsinline"); },
                set(value) {
                    if (value) this.setAttribute("playsinline", "");
                    else this.removeAttribute("playsinline");
                },
            });
            media("preload", {
                get() {
                    const value = (this.getAttribute("preload") ?? "").toLowerCase();
                    if (value === "none" || value === "metadata" || value === "auto") return value;
                    if (value === "" && this.hasAttribute("preload")) return "auto";
                    return "metadata";
                },
                set(value) { this.setAttribute("preload", String(value)); },
            });
            media("crossOrigin", {
                get() {
                    if (!this.hasAttribute("crossorigin")) return null;
                    return (this.getAttribute("crossorigin") ?? "").toLowerCase() === "use-credentials" ? "use-credentials" : "anonymous";
                },
                set(value) {
                    if (value === null) this.removeAttribute("crossorigin");
                    else this.setAttribute("crossorigin", String(value));
                },
            });
            video("poster", {
                get() {
                    const raw = this.getAttribute("poster");
                    if (raw === null || raw === "") return "";
                    try {
                        return new URL(raw, document.baseURI).href;
                    } catch {
                        return raw;
                    }
                },
                set(value) { this.setAttribute("poster", String(value)); },
            });

            const srcDescriptor = Object.getOwnPropertyDescriptor(elementProto, "src");
            if (srcDescriptor?.get && srcDescriptor?.set) {
                const getSrc = srcDescriptor.get;
                const setSrc = srcDescriptor.set;
                Object.defineProperty(elementProto, "src", {
                    get() { return getSrc.call(this); },
                    set(value) {
                        setSrc.call(this, value);
                        if (isMedia(this)) loadMedia(this);
                    },
                    configurable: true,
                    enumerable: true,
                });
            }
            media("currentSrc", { get() { return stateOf(this).currentSrc; } });
            media("srcObject", {
                get() { return stateOf(this).srcObject ?? null; },
                set(value) { stateOf(this).srcObject = value ?? null; },
            });
            media("networkState", { get() { return stateOf(this).networkState; } });
            media("readyState", { get() { return stateOf(this).readyState; } });
            media("error", { get() { return stateOf(this).error; } });
            media("paused", { get() { return stateOf(this).paused; } });
            media("ended", { get() { return stateOf(this).ended && !this.hasAttribute("loop"); } });
            media("seeking", { get() { return stateOf(this).seeking; } });
            media("duration", {
                get() {
                    const state = stateOf(this);
                    return state.readyState < HAVE_METADATA ? NaN : state.duration;
                },
            });
            media("currentTime", {
                get() {
                    const state = stateOf(this);
                    if (state.readyState < HAVE_METADATA) return state.startAt;
                    return currentTimeOf(state, this);
                },
                set(value) {
                    const time = Number(value);
                    if (!Number.isFinite(time)) {
                        throw new TypeError("Failed to set the 'currentTime' property on 'HTMLMediaElement': The provided double value is non-finite.");
                    }
                    const state = stateOf(this);
                    if (state.readyState < HAVE_METADATA) {
                        state.startAt = Math.max(0, time);
                        return;
                    }
                    seekTo(this, state, time);
                },
            });
            media("playbackRate", {
                get() { return stateOf(this).playbackRate; },
                set(value) {
                    const rate = Number(value);
                    if (!Number.isFinite(rate)) {
                        throw new TypeError("Failed to set the 'playbackRate' property on 'HTMLMediaElement': The provided double value is non-finite.");
                    }
                    if (rate < 0) {
                        throw new DOMException("The provided playback rate (" + rate + ") is not in the supported playback range.", "NotSupportedError");
                    }
                    const state = stateOf(this);
                    if (state.playbackRate === rate) return;
                    state.playbackRate = rate;
                    if (state.open && rate > 0) controlNative(this, "rate", rate);
                    fire(this, "ratechange");
                },
            });
            media("defaultPlaybackRate", {
                get() { return stateOf(this).defaultPlaybackRate; },
                set(value) {
                    const rate = Number(value);
                    if (!Number.isFinite(rate)) {
                        throw new TypeError("Failed to set the 'defaultPlaybackRate' property on 'HTMLMediaElement': The provided double value is non-finite.");
                    }
                    const state = stateOf(this);
                    if (state.defaultPlaybackRate === rate) return;
                    state.defaultPlaybackRate = rate;
                    fire(this, "ratechange");
                },
            });
            media("volume", {
                get() { return stateOf(this).volume; },
                set(value) {
                    const volume = Number(value);
                    if (!(volume >= 0 && volume <= 1)) {
                        throw new DOMException("Failed to set the 'volume' property on 'HTMLMediaElement': The volume provided (" + volume + ") is outside the range [0, 1].", "IndexSizeError");
                    }
                    const state = stateOf(this);
                    if (state.volume === volume) return;
                    state.volume = volume;
                    syncNative(this, state);
                    fire(this, "volumechange");
                },
            });
            media("muted", {
                get() { return stateOf(this).muted; },
                set(value) {
                    const muted = Boolean(value);
                    const state = stateOf(this);
                    if (state.muted === muted) return;
                    state.muted = muted;
                    syncNative(this, state);
                    fire(this, "volumechange");
                },
            });
            const rangesFor = (state) => {
                if (state.readyState < HAVE_METADATA || !Number.isFinite(state.duration)) return new makeRanges([]);
                return new makeRanges([[0, state.duration]]);
            };
            media("buffered", { get() { return rangesFor(stateOf(this)); } });
            media("seekable", { get() { return rangesFor(stateOf(this)); } });
            media("played", {
                get() {
                    const state = stateOf(this);
                    const time = currentTimeOf(state, this);
                    return new makeRanges(time > 0 ? [[0, time]] : []);
                },
            });
            video("videoWidth", { get() { return stateOf(this).readyState >= HAVE_METADATA ? stateOf(this).width : 0; } });
            video("videoHeight", { get() { return stateOf(this).readyState >= HAVE_METADATA ? stateOf(this).height : 0; } });

            media("load", { value: function load() { loadMedia(this); } });
            media("play", { value: function play() { return playMedia(this); } });
            media("pause", { value: function pause() { pauseMedia(this); } });
            media("fastSeek", {
                value: function fastSeek(time) {
                    this.currentTime = time;
                },
            });
            media("canPlayType", {
                value: function canPlayType(type) {
                    return canPlayNative(String(type));
                },
            });
            video("requestVideoFrameCallback", {
                value: function requestVideoFrameCallback(callback) {
                    if (typeof callback !== "function") {
                        throw new TypeError("Failed to execute 'requestVideoFrameCallback' on 'HTMLVideoElement': parameter 1 is not of type 'Function'.");
                    }
                    const state = stateOf(this);
                    state.nextCallback += 1;
                    state.frameCallbacks.set(state.nextCallback, callback);
                    return state.nextCallback;
                },
            });
            video("cancelVideoFrameCallback", {
                value: function cancelVideoFrameCallback(handle) {
                    stateOf(this).frameCallbacks.delete(Number(handle));
                },
            });
            video("getVideoPlaybackQuality", {
                value: function getVideoPlaybackQuality() {
                    return {
                        creationTime: performance.now(),
                        totalVideoFrames: stateOf(this).presented,
                        droppedVideoFrames: 0,
                        corruptedVideoFrames: 0,
                    };
                },
            });

            function Audio(src) {
                if (new.target === undefined) {
                    throw new TypeError("Failed to construct 'Audio': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
                }
                const audio = document.createElement("audio");
                audio.setAttribute("preload", "auto");
                if (src !== undefined) audio.src = src;
                return audio;
            }
            if (audioProto) Audio.prototype = audioProto;
            globalThis.Audio = Audio;

            const internal = Symbol("internal");
            const illegal = (key) => {
                if (key !== internal) throw new TypeError("Illegal constructor");
            };
            const contexts = new WeakMap();

            class AudioParam {
                #value;
                #context;
                #timers = new Set();
                #onchange;
                constructor(key, context, value, onchange) {
                    illegal(key);
                    this.#context = context;
                    this.#value = value;
                    this.#onchange = onchange;
                    Object.defineProperty(this, "defaultValue", { value, enumerable: true });
                    Object.defineProperty(this, "minValue", { value: -3.4028234663852886e38, enumerable: true });
                    Object.defineProperty(this, "maxValue", { value: 3.4028234663852886e38, enumerable: true });
                }
                get value() {
                    return this.#value;
                }
                set value(value) {
                    const number = Number(value);
                    if (!Number.isFinite(number)) {
                        throw new TypeError("Failed to set the 'value' property on 'AudioParam': The provided float value is non-finite.");
                    }
                    this.#value = Math.fround(number);
                    this.#onchange();
                }
                #schedule(value, time) {
                    const number = Number(value);
                    const at = Number(time);
                    if (!Number.isFinite(number) || !Number.isFinite(at)) {
                        throw new TypeError("The provided value is non-finite.");
                    }
                    if (at < 0) throw new RangeError("Time must be non-negative.");
                    const delay = (at - this.#context.currentTime) * 1000;
                    if (delay <= 0) {
                        this.value = number;
                        return this;
                    }
                    const timer = setTimeout(() => {
                        this.#timers.delete(timer);
                        this.value = number;
                    }, delay);
                    this.#timers.add(timer);
                    return this;
                }
                setValueAtTime(value, time) {
                    return this.#schedule(value, time);
                }
                linearRampToValueAtTime(value, time) {
                    return this.#schedule(value, time);
                }
                exponentialRampToValueAtTime(value, time) {
                    return this.#schedule(value, time);
                }
                setTargetAtTime(value, time) {
                    return this.#schedule(value, time);
                }
                setValueCurveAtTime(values, time, duration) {
                    const list = Array.from(values ?? []);
                    if (list.length < 2) throw new DOMException("The curve must have at least two values.", "InvalidStateError");
                    return this.#schedule(list[list.length - 1], Number(time) + Number(duration));
                }
                cancelScheduledValues() {
                    for (const timer of this.#timers) clearTimeout(timer);
                    this.#timers.clear();
                    return this;
                }
                cancelAndHoldAtTime() {
                    return this.cancelScheduledValues();
                }
            }

            const nodeOutputs = new WeakMap();
            const reachOf = (node, seen = new Set()) => {
                if (node instanceof AudioDestinationNode) return 1;
                if (seen.has(node)) return 0;
                seen.add(node);
                let total = 0;
                for (const output of nodeOutputs.get(node) ?? []) total += reachOf(output, seen);
                seen.delete(node);
                return node instanceof GainNode ? total * node.gain.value : total;
            };

            class AudioNode extends EventTarget {
                #context;
                #outputs = new Set();
                constructor(key, context) {
                    super();
                    illegal(key);
                    this.#context = context;
                    nodeOutputs.set(this, this.#outputs);
                }
                static [Symbol.hasInstance](value) {
                    return Function.prototype[Symbol.hasInstance].call(this, value);
                }
                get context() {
                    return this.#context;
                }
                get numberOfInputs() {
                    return this instanceof MediaElementAudioSourceNode ? 0 : 1;
                }
                get numberOfOutputs() {
                    return this instanceof AudioDestinationNode ? 0 : 1;
                }
                get channelCount() {
                    return 2;
                }
                get channelCountMode() {
                    return "max";
                }
                get channelInterpretation() {
                    return "speakers";
                }
                connect(destination) {
                    if (destination instanceof AudioParam) {
                        return undefined;
                    }
                    if (!(destination instanceof AudioNode)) {
                        throw new TypeError("Failed to execute 'connect' on 'AudioNode': Overload resolution failed.");
                    }
                    if (destination.context !== this.#context) {
                        throw new DOMException("Failed to execute 'connect' on 'AudioNode': cannot connect to an AudioNode belonging to a different audio context.", "InvalidAccessError");
                    }
                    if (destination instanceof MediaElementAudioSourceNode) {
                        throw new DOMException("Failed to execute 'connect' on 'AudioNode': the destination has no inputs.", "IndexSizeError");
                    }
                    this.#outputs.add(destination);
                    contexts.get(this.#context)?.refresh();
                    return destination;
                }
                disconnect(destination) {
                    if (destination === undefined || typeof destination === "number") {
                        this.#outputs.clear();
                    } else if (!this.#outputs.delete(destination)) {
                        throw new DOMException("Failed to execute 'disconnect' on 'AudioNode': the given destination is not connected.", "InvalidAccessError");
                    }
                    contexts.get(this.#context)?.refresh();
                }
            }

            class AudioDestinationNode extends AudioNode {
                get maxChannelCount() {
                    return 2;
                }
            }

            class GainNode extends AudioNode {
                #gain;
                constructor(context, options) {
                    const owner = context === internal ? options.context : context;
                    if (!contexts.has(owner)) {
                        throw new TypeError("Failed to construct 'GainNode': parameter 1 is not of type 'BaseAudioContext'.");
                    }
                    super(internal, owner);
                    const initial = context === internal ? 1 : Number(options?.gain ?? 1);
                    this.#gain = new AudioParam(internal, owner, Number.isFinite(initial) ? initial : 1, () => contexts.get(owner)?.refresh());
                }
                get gain() {
                    return this.#gain;
                }
            }

            class MediaElementAudioSourceNode extends AudioNode {
                #element;
                constructor(context, options) {
                    const owner = context === internal ? options.context : context;
                    const element = options?.mediaElement;
                    if (!contexts.has(owner)) {
                        throw new TypeError("Failed to construct 'MediaElementAudioSourceNode': parameter 1 is not of type 'AudioContext'.");
                    }
                    if (!isMedia(element)) {
                        throw new TypeError("Failed to construct 'MediaElementAudioSourceNode': member mediaElement is not of type 'HTMLMediaElement'.");
                    }
                    const state = stateOf(element);
                    if (state.source !== null) {
                        throw new DOMException("Failed to construct 'MediaElementAudioSourceNode': HTMLMediaElement already connected previously to a different MediaElementSourceNode.", "InvalidStateError");
                    }
                    super(internal, owner);
                    this.#element = element;
                    const node = this;
                    state.source = {
                        node,
                        gainValue: () => contexts.get(owner).gainFor(node),
                    };
                    contexts.get(owner).sources.add(node);
                    syncNative(element, state);
                }
                get mediaElement() {
                    return this.#element;
                }
            }

            class BaseAudioContext extends EventTarget {
                constructor(key) {
                    super();
                    illegal(key);
                }
                static [Symbol.hasInstance](value) {
                    return Function.prototype[Symbol.hasInstance].call(this, value);
                }
            }

            class AudioContext extends BaseAudioContext {
                #state = "running";
                #destination;
                #origin = performance.now();
                #elapsed = 0;
                #onstatechange = null;
                constructor(options) {
                    super(internal);
                    const record = {
                        sources: new Set(),
                        refresh: () => {
                            for (const source of record.sources) {
                                const element = source.mediaElement;
                                syncNative(element, stateOf(element));
                            }
                        },
                        gainFor: (node) => (this.#state === "running" ? reachOf(node) : 0),
                    };
                    contexts.set(this, record);
                    this.#destination = new AudioDestinationNode(internal, this);
                    const rate = Number(options?.sampleRate);
                    Object.defineProperty(this, "sampleRate", {
                        value: Number.isFinite(rate) && rate >= 3000 && rate <= 768000 ? rate : 48000,
                        enumerable: true,
                    });
                }
                get state() {
                    return this.#state;
                }
                get destination() {
                    return this.#destination;
                }
                get currentTime() {
                    if (this.#state !== "running") return this.#elapsed;
                    return this.#elapsed + (performance.now() - this.#origin) / 1000;
                }
                get baseLatency() {
                    return 0.01;
                }
                get outputLatency() {
                    return 0.02;
                }
                get listener() {
                    return null;
                }
                get onstatechange() {
                    return this.#onstatechange;
                }
                set onstatechange(handler) {
                    this.#onstatechange = typeof handler === "function" ? handler : null;
                }
                #setState(next) {
                    if (this.#state === next) return;
                    if (this.#state === "running") {
                        this.#elapsed += (performance.now() - this.#origin) / 1000;
                    }
                    if (next === "running") this.#origin = performance.now();
                    this.#state = next;
                    contexts.get(this).refresh();
                    queueMicrotask(() => {
                        const event = new Event("statechange");
                        if (this.#onstatechange) {
                            try {
                                this.#onstatechange.call(this, event);
                            } catch (error) {
                                queueMicrotask(() => { throw error; });
                            }
                        }
                        this.dispatchEvent(event);
                    });
                }
                #closedError(operation) {
                    return new DOMException("Failed to execute '" + operation + "' on 'AudioContext': Cannot " + operation + " a closed AudioContext.", "InvalidStateError");
                }
                resume() {
                    if (this.#state === "closed") return Promise.reject(this.#closedError("resume"));
                    this.#setState("running");
                    return Promise.resolve();
                }
                suspend() {
                    if (this.#state === "closed") return Promise.reject(this.#closedError("suspend"));
                    this.#setState("suspended");
                    return Promise.resolve();
                }
                close() {
                    if (this.#state === "closed") return Promise.reject(this.#closedError("close"));
                    this.#setState("closed");
                    return Promise.resolve();
                }
                createGain() {
                    if (this.#state === "closed") throw this.#closedError("createGain");
                    return new GainNode(internal, { context: this });
                }
                createMediaElementSource(element) {
                    if (this.#state === "closed") throw this.#closedError("createMediaElementSource");
                    return new MediaElementAudioSourceNode(internal, { context: this, mediaElement: element });
                }
                getOutputTimestamp() {
                    return { contextTime: this.currentTime, performanceTime: performance.now() };
                }
            }

            globalThis.AudioParam = function AudioParam() {
                throw new TypeError("Illegal constructor");
            };
            globalThis.AudioParam.prototype = AudioParam.prototype;
            globalThis.AudioNode = AudioNode;
            globalThis.AudioDestinationNode = AudioDestinationNode;
            globalThis.GainNode = GainNode;
            globalThis.MediaElementAudioSourceNode = MediaElementAudioSourceNode;
            globalThis.BaseAudioContext = BaseAudioContext;
            globalThis.AudioContext = AudioContext;
            globalThis.webkitAudioContext = AudioContext;
        }
    }
