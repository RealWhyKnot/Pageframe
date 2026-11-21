    {
        const nativeEvent = __pageframe_new_event;
        const eventFlags = __pageframe_event_flags;
        const eventProto = Object.getPrototypeOf(nativeEvent("pageframe"));
        const constructError = (name, args) => {
            if (args.length === 0) {
                return new TypeError("Failed to construct '" + name + "': 1 argument required, but only 0 present.");
            }
            return null;
        };
        const readonly = (target, values) => {
            for (const [key, value] of Object.entries(values)) {
                Object.defineProperty(target, key, { value, writable: false, enumerable: true, configurable: true });
            }
        };
        const dictionary = (init) => (init === undefined || init === null ? {} : Object(init));

        function Event(type, init) {
            if (new.target === undefined) {
                throw new TypeError("Failed to construct 'Event': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
            }
            const missing = constructError("Event", arguments);
            if (missing) throw missing;
            const options = dictionary(init);
            const event = nativeEvent(String(type), !!options.bubbles, !!options.cancelable, !!options.composed);
            Object.setPrototypeOf(event, new.target.prototype);
            return event;
        }
        Event.prototype = eventProto;
        Object.defineProperty(eventProto, "constructor", { value: Event, writable: true, configurable: true });
        for (const [name, value] of [["NONE", 0], ["CAPTURING_PHASE", 1], ["AT_TARGET", 2], ["BUBBLING_PHASE", 3]]) {
            Object.defineProperty(Event, name, { value, enumerable: true });
            Object.defineProperty(eventProto, name, { value, enumerable: true });
        }
        if (typeof eventProto.initEvent !== "function") {
            Object.defineProperty(eventProto, "initEvent", {
                configurable: true,
                writable: true,
                value(type, bubbles, cancelable) {
                    readonly(this, { type: String(type), bubbles: !!bubbles, cancelable: !!cancelable });
                },
            });
        }

        const subclass = (name, parent, defaults) => {
            const cls = {
                [name]: class extends parent {
                    constructor(type, init) {
                        const missing = constructError(name, arguments);
                        if (missing) throw missing;
                        super(type, init);
                        const options = dictionary(init);
                        const values = {};
                        for (const [key, fallback] of Object.entries(defaults)) {
                            values[key] = options[key] === undefined ? fallback : options[key];
                        }
                        readonly(this, values);
                    }
                },
            }[name];
            globalThis[name] = cls;
            return cls;
        };

        const modifiers = { ctrlKey: false, shiftKey: false, altKey: false, metaKey: false };
        const UIEvent = subclass("UIEvent", Event, { view: null, detail: 0, which: 0 });
        const MouseEvent = subclass("MouseEvent", UIEvent, {
            ...modifiers,
            screenX: 0, screenY: 0, clientX: 0, clientY: 0, button: 0, buttons: 0,
            relatedTarget: null, movementX: 0, movementY: 0,
        });
        for (const [alias, source] of [["x", "clientX"], ["y", "clientY"], ["pageX", "clientX"], ["pageY", "clientY"], ["offsetX", "clientX"], ["offsetY", "clientY"]]) {
            Object.defineProperty(MouseEvent.prototype, alias, {
                configurable: true,
                get() { return this[source]; },
            });
        }
        Object.defineProperty(MouseEvent.prototype, "getModifierState", {
            configurable: true,
            writable: true,
            value(key) {
                return { Control: this.ctrlKey, Shift: this.shiftKey, Alt: this.altKey, Meta: this.metaKey }[key] === true;
            },
        });
        subclass("PointerEvent", MouseEvent, {
            pointerId: 0, width: 1, height: 1, pressure: 0, tangentialPressure: 0,
            tiltX: 0, tiltY: 0, twist: 0, altitudeAngle: Math.PI / 2, azimuthAngle: 0,
            pointerType: "", isPrimary: false,
        });
        PointerEvent.prototype.getCoalescedEvents = function () { return []; };
        PointerEvent.prototype.getPredictedEvents = function () { return []; };
        subclass("WheelEvent", MouseEvent, { deltaX: 0, deltaY: 0, deltaZ: 0, deltaMode: 0 });
        Object.assign(WheelEvent, { DOM_DELTA_PIXEL: 0, DOM_DELTA_LINE: 1, DOM_DELTA_PAGE: 2 });
        subclass("KeyboardEvent", UIEvent, {
            ...modifiers, key: "", code: "", location: 0, repeat: false, isComposing: false,
            charCode: 0, keyCode: 0,
        });
        KeyboardEvent.prototype.getModifierState = MouseEvent.prototype.getModifierState;
        subclass("InputEvent", UIEvent, { data: null, inputType: "", isComposing: false, dataTransfer: null });
        subclass("FocusEvent", UIEvent, { relatedTarget: null });
        subclass("CompositionEvent", UIEvent, { data: "" });
        subclass("TouchEvent", UIEvent, { ...modifiers, touches: [], targetTouches: [], changedTouches: [] });
        subclass("CustomEvent", Event, { detail: null });
        CustomEvent.prototype.initCustomEvent = function (type, bubbles, cancelable, detail) {
            this.initEvent(type, bubbles, cancelable);
            readonly(this, { detail: detail === undefined ? null : detail });
        };
        subclass("MessageEvent", Event, { data: null, origin: "", lastEventId: "", source: null, ports: [] });
        subclass("ErrorEvent", Event, { message: "", filename: "", lineno: 0, colno: 0, error: undefined });
        subclass("HashChangeEvent", Event, { oldURL: "", newURL: "" });
        subclass("PopStateEvent", Event, { state: null, hasUAVisualTransition: false });
        subclass("PageTransitionEvent", Event, { persisted: false });
        subclass("ProgressEvent", Event, { lengthComputable: false, loaded: 0, total: 0 });
        subclass("MediaQueryListEvent", Event, { media: "", matches: false });
        subclass("PromiseRejectionEvent", Event, { promise: undefined, reason: undefined });
        subclass("SubmitEvent", Event, { submitter: null });
        subclass("ToggleEvent", Event, { oldState: "", newState: "" });
        subclass("AnimationEvent", Event, { animationName: "", elapsedTime: 0, pseudoElement: "" });
        subclass("TransitionEvent", Event, { propertyName: "", elapsedTime: 0, pseudoElement: "" });
        globalThis.Event = Event;

        const targetListeners = new WeakMap();
        const dispatching = new WeakSet();
        const nativeTarget = (value) =>
            value === globalThis
            || (typeof globalThis.Node === "function" && value instanceof globalThis.Node);
        const optionFlags = (options) => {
            if (typeof options === "boolean") return { capture: options, once: false, signal: null };
            const value = dictionary(options);
            return { capture: !!value.capture, once: !!value.once, signal: value.signal ?? null };
        };
        class EventTarget {
            constructor() {
                targetListeners.set(this, new Map());
            }
            addEventListener(type, callback, options) {
                if (callback === null || callback === undefined) return;
                const list = targetListeners.get(this);
                if (!list) return;
                const { capture, once, signal } = optionFlags(options);
                if (signal && signal.aborted) return;
                const key = String(type);
                const entries = list.get(key) ?? [];
                if (entries.some((entry) => entry.callback === callback && entry.capture === capture)) return;
                const entry = { callback, capture, once, removed: false };
                entries.push(entry);
                list.set(key, entries);
                if (signal) signal.addEventListener("abort", () => this.removeEventListener(key, callback, { capture }), { once: true });
            }
            removeEventListener(type, callback, options) {
                const list = targetListeners.get(this);
                if (!list) return;
                const { capture } = optionFlags(options);
                const key = String(type);
                const entries = list.get(key) ?? [];
                for (const entry of entries) {
                    if (entry.callback === callback && entry.capture === capture) entry.removed = true;
                }
                list.set(key, entries.filter((entry) => !entry.removed));
            }
            dispatchEvent(event) {
                if (!(event instanceof Event)) {
                    throw new TypeError("Failed to execute 'dispatchEvent' on 'EventTarget': parameter 1 is not of type 'Event'.");
                }
                if (dispatching.has(event) || (eventFlags(event) & 8) !== 0) {
                    throw new DOMException("Failed to execute 'dispatchEvent' on 'EventTarget': The event is already being dispatched.", "InvalidStateError");
                }
                const list = targetListeners.get(this);
                const define = (key, value) => Object.defineProperty(event, key, { value, writable: true, configurable: true });
                define("target", this);
                define("srcElement", this);
                define("currentTarget", this);
                define("eventPhase", 2);
                const handler = this["on" + event.type];
                const entries = [...(list?.get(String(event.type)) ?? [])];
                if (typeof handler === "function") entries.push({ callback: handler, once: false, removed: false, handler: true });
                dispatching.add(event);
                for (const entry of entries) {
                    if (entry.removed) continue;
                    if (entry.once) this.removeEventListener(event.type, entry.callback, { capture: entry.capture });
                    try {
                        if (typeof entry.callback === "function") entry.callback.call(this, event);
                        else if (entry.callback && typeof entry.callback.handleEvent === "function") entry.callback.handleEvent(event);
                    } catch (error) {
                        if (typeof globalThis.reportError === "function") globalThis.reportError(error);
                    }
                    if (eventFlags(event) & 4) break;
                }
                dispatching.delete(event);
                define("currentTarget", null);
                define("eventPhase", 0);
                return (eventFlags(event) & 1) === 0;
            }
            static [Symbol.hasInstance](value) {
                if (typeof value !== "object" && typeof value !== "function") return false;
                if (value === null) return false;
                return EventTarget.prototype.isPrototypeOf(value) || nativeTarget(value);
            }
        }
        globalThis.EventTarget = EventTarget;
        if (typeof globalThis.reportError !== "function") {
            globalThis.reportError = function reportError(error) {
                setTimeout(() => { throw error; }, 0);
            };
        }
    }
