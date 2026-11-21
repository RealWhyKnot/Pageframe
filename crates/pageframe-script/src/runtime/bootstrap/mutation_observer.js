    {
        const setObserving = __pageframe_mutation_observing;
        const begin = __pageframe_mutation_begin;
        const end = __pageframe_mutation_end;
        const takeNative = __pageframe_take_mutations;
        const states = new WeakMap();
        const slots = new WeakMap();
        const observers = [];
        let active = false;

        const fail = (operation, message) =>
            new TypeError("Failed to execute '" + operation + "' on 'MutationObserver': " + message);
        const stateOf = (observer) => {
            const state = states.get(observer);
            if (!state) throw new TypeError("Illegal invocation");
            return state;
        };
        const nodeList = (nodes) => {
            const list = nodes.slice();
            Object.defineProperty(list, "item", { value: (index) => list[index] ?? null });
            return Object.freeze(list);
        };

        function MutationRecord() {
            throw new TypeError("Illegal constructor");
        }
        const recordFields = ["type", "target", "addedNodes", "removedNodes", "previousSibling",
            "nextSibling", "attributeName", "attributeNamespace", "oldValue"];
        for (const field of recordFields) {
            Object.defineProperty(MutationRecord.prototype, field, {
                get() {
                    const slot = slots.get(this);
                    if (!slot) throw new TypeError("Illegal invocation");
                    return slot[field];
                },
                enumerable: true,
                configurable: true,
            });
        }
        const makeRecord = (raw, withOld) => {
            const record = Object.create(MutationRecord.prototype);
            slots.set(record, {
                type: raw.type,
                target: raw.target,
                addedNodes: nodeList(raw.addedNodes),
                removedNodes: nodeList(raw.removedNodes),
                previousSibling: raw.previousSibling,
                nextSibling: raw.nextSibling,
                attributeName: raw.attributeName,
                attributeNamespace: null,
                oldValue: withOld ? raw.oldValue : null,
            });
            return record;
        };

        const interest = (options, raw) => {
            if (raw.type === "childList") return options.childList ? { old: false } : null;
            if (raw.type === "characterData") {
                return options.characterData ? { old: options.characterDataOldValue } : null;
            }
            if (!options.attributes) return null;
            if (options.attributeFilter && !options.attributeFilter.has(raw.attributeName)) return null;
            return { old: options.attributeOldValue };
        };
        const distribute = () => {
            for (const raw of takeNative()) {
                for (const observer of observers) {
                    const state = states.get(observer);
                    let interested = false;
                    let withOld = false;
                    for (const node of raw.path) {
                        const options = state.targets.get(node);
                        if (!options || (node !== raw.target && !options.subtree)) continue;
                        const match = interest(options, raw);
                        if (!match) continue;
                        interested = true;
                        withOld = withOld || match.old;
                    }
                    if (interested) state.queue.push(makeRecord(raw, withOld));
                }
            }
        };
        const refresh = () => {
            active = observers.some((observer) => states.get(observer).targets.size > 0);
            setObserving(active);
        };
        Object.defineProperty(globalThis, "__pageframe_mutation_deliver", {
            value: function deliver() {
                distribute();
                for (const observer of observers.slice()) {
                    const state = states.get(observer);
                    if (state.queue.length === 0) continue;
                    const records = state.queue.splice(0);
                    try {
                        state.callback.call(observer, records, observer);
                    } catch (error) {
                        if (typeof globalThis.reportError === "function") globalThis.reportError(error);
                    }
                }
            },
            configurable: true,
        });

        class MutationObserver {
            constructor(callback) {
                if (typeof callback !== "function") {
                    throw new TypeError("Failed to construct 'MutationObserver': parameter 1 is not of type 'Function'.");
                }
                states.set(this, { callback, targets: new Map(), queue: [] });
            }
            observe(target, options = {}) {
                const state = stateOf(this);
                if (!(target instanceof Node)) throw fail("observe", "parameter 1 is not of type 'Node'.");
                let { attributes, characterData } = options;
                const { attributeOldValue, characterDataOldValue, attributeFilter } = options;
                if (attributes === undefined && (attributeOldValue !== undefined || attributeFilter !== undefined)) {
                    attributes = true;
                }
                if (characterData === undefined && characterDataOldValue !== undefined) characterData = true;
                const childList = Boolean(options.childList);
                if (!childList && !attributes && !characterData) {
                    throw fail("observe", "The options object must set at least one of 'attributes', 'characterData', or 'childList' to true.");
                }
                if (attributeOldValue && !attributes) {
                    throw fail("observe", "The options object may only set 'attributeOldValue' to true when 'attributes' is true or not present.");
                }
                if (attributeFilter !== undefined && !attributes) {
                    throw fail("observe", "The options object may only set 'attributeFilter' when 'attributes' is true or not present.");
                }
                if (characterDataOldValue && !characterData) {
                    throw fail("observe", "The options object may only set 'characterDataOldValue' to true when 'characterData' is true or not present.");
                }
                state.targets.set(target, {
                    childList,
                    attributes: Boolean(attributes),
                    characterData: Boolean(characterData),
                    subtree: Boolean(options.subtree),
                    attributeOldValue: Boolean(attributeOldValue),
                    characterDataOldValue: Boolean(characterDataOldValue),
                    attributeFilter: attributeFilter === undefined ? null : new Set(Array.from(attributeFilter, String)),
                });
                if (!observers.includes(this)) observers.push(this);
                refresh();
            }
            disconnect() {
                const state = stateOf(this);
                distribute();
                state.targets.clear();
                state.queue.length = 0;
                const index = observers.indexOf(this);
                if (index !== -1) observers.splice(index, 1);
                refresh();
            }
            takeRecords() {
                const state = stateOf(this);
                distribute();
                return state.queue.splice(0);
            }
        }
        Object.defineProperty(MutationObserver.prototype, Symbol.toStringTag, { value: "MutationObserver", configurable: true });
        Object.defineProperty(MutationRecord.prototype, Symbol.toStringTag, { value: "MutationRecord", configurable: true });
        for (const [name, value] of [["MutationObserver", MutationObserver], ["MutationRecord", MutationRecord]]) {
            Object.defineProperty(globalThis, name, { value, writable: true, configurable: true });
        }
        if (!globalThis.WebKitMutationObserver) {
            Object.defineProperty(globalThis, "WebKitMutationObserver", { value: MutationObserver, writable: true, configurable: true });
        }

        const methods = ["appendChild", "insertBefore", "removeChild", "replaceChild", "remove", "before", "after",
            "replaceWith", "append", "prepend", "replaceChildren", "insertAdjacentHTML", "insertAdjacentElement",
            "insertAdjacentText", "normalize", "splitText", "appendData", "insertData", "deleteData", "replaceData"];
        const setters = ["textContent", "nodeValue", "data", "innerHTML", "outerHTML", "innerText", "outerText"];
        const samples = [document.documentElement, document.createTextNode(""), document.createComment(""),
            document, document.createDocumentFragment()].filter(Boolean);
        const owners = (name) => {
            const found = new Set();
            for (const sample of samples) {
                for (let proto = Object.getPrototypeOf(sample); proto; proto = Object.getPrototypeOf(proto)) {
                    if (Object.prototype.hasOwnProperty.call(proto, name)) {
                        found.add(proto);
                        break;
                    }
                }
            }
            return found;
        };
        const named = (fn, name, length) => {
            Object.defineProperty(fn, "name", { value: name, configurable: true });
            Object.defineProperty(fn, "length", { value: length, configurable: true });
            return fn;
        };
        for (const name of methods) {
            for (const owner of owners(name)) {
                const descriptor = Object.getOwnPropertyDescriptor(owner, name);
                const original = descriptor.value;
                if (typeof original !== "function") continue;
                const wrapper = {
                    [name](...args) {
                        if (!active) return original.apply(this, args);
                        begin(this, ...args);
                        try {
                            return original.apply(this, args);
                        } finally {
                            end();
                        }
                    },
                }[name];
                descriptor.value = named(wrapper, name, original.length);
                Object.defineProperty(owner, name, descriptor);
            }
        }
        for (const name of setters) {
            for (const owner of owners(name)) {
                const descriptor = Object.getOwnPropertyDescriptor(owner, name);
                const original = descriptor.set;
                if (typeof original !== "function") continue;
                descriptor.set = named(function (value) {
                    if (!active) {
                        original.call(this, value);
                        return;
                    }
                    begin(this);
                    try {
                        original.call(this, value);
                    } finally {
                        end();
                    }
                }, "set " + name, 1);
                Object.defineProperty(owner, name, descriptor);
            }
        }
    }
