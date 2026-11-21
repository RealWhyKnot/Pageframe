    {
        const resizeObservers = new Set();
        const intersectionObservers = new Set();
        let scheduled = false;
        let quietTicks = 0;

        const anyObservations = () => {
            for (const observer of resizeObservers) if (observer.__pageframe_size() > 0) return true;
            for (const observer of intersectionObservers) if (observer.__pageframe_size() > 0) return true;
            return false;
        };
        const tick = () => {
            scheduled = false;
            let delivered = false;
            for (const observer of [...resizeObservers]) delivered = observer.__pageframe_check() || delivered;
            for (const observer of [...intersectionObservers]) delivered = observer.__pageframe_check() || delivered;
            quietTicks = delivered ? 0 : quietTicks + 1;
            schedule();
        };
        const schedule = () => {
            if (scheduled || !anyObservations()) return;
            scheduled = true;
            if (quietTicks < 10) requestAnimationFrame(tick);
            else setTimeout(tick, 200);
        };
        const wake = () => {
            quietTicks = 0;
            schedule();
        };
        const isElement = (value) => typeof Element === "function" && value instanceof Element;
        const connected = (node) => (typeof node.isConnected === "boolean"
            ? node.isConnected
            : document.documentElement.contains(node));
        const px = (value) => {
            const number = parseFloat(value);
            return Number.isFinite(number) ? number : 0;
        };
        const boxes = (target) => {
            const rect = target.getBoundingClientRect();
            const style = getComputedStyle(target);
            const left = px(style.paddingLeft) + px(style.borderLeftWidth);
            const top = px(style.paddingTop) + px(style.borderTopWidth);
            const right = px(style.paddingRight) + px(style.borderRightWidth);
            const bottom = px(style.paddingBottom) + px(style.borderBottomWidth);
            const width = Math.max(0, rect.width - left - right);
            const height = Math.max(0, rect.height - top - bottom);
            return {
                border: { inlineSize: rect.width, blockSize: rect.height },
                content: { inlineSize: width, blockSize: height },
                contentRect: new DOMRectReadOnly(px(style.paddingLeft), px(style.paddingTop), width, height),
            };
        };

        class ResizeObserverEntry {
            constructor(target, measured) {
                const ratio = globalThis.devicePixelRatio || 1;
                Object.defineProperties(this, {
                    target: { value: target, enumerable: true },
                    contentRect: { value: measured.contentRect, enumerable: true },
                    borderBoxSize: { value: Object.freeze([Object.freeze({ ...measured.border })]), enumerable: true },
                    contentBoxSize: { value: Object.freeze([Object.freeze({ ...measured.content })]), enumerable: true },
                    devicePixelContentBoxSize: {
                        value: Object.freeze([Object.freeze({
                            inlineSize: Math.round(measured.content.inlineSize * ratio),
                            blockSize: Math.round(measured.content.blockSize * ratio),
                        })]),
                        enumerable: true,
                    },
                });
            }
        }

        class ResizeObserver {
            #callback;
            #targets = new Map();
            constructor(callback) {
                if (typeof callback !== "function") {
                    throw new TypeError("Failed to construct 'ResizeObserver': parameter 1 is not of type 'Function'.");
                }
                this.#callback = callback;
            }
            observe(target, options) {
                if (!isElement(target)) throw new TypeError("ResizeObserver.observe: target is not an Element");
                const box = options?.box ?? "content-box";
                this.#targets.set(target, { box, inline: -1, block: -1 });
                resizeObservers.add(this);
                wake();
            }
            unobserve(target) {
                this.#targets.delete(target);
            }
            disconnect() {
                this.#targets.clear();
                resizeObservers.delete(this);
            }
            __pageframe_size() {
                return this.#targets.size;
            }
            __pageframe_check() {
                const entries = [];
                for (const [target, last] of this.#targets) {
                    if (!connected(target)) continue;
                    const measured = boxes(target);
                    const size = last.box === "border-box" ? measured.border : measured.content;
                    if (size.inlineSize === last.inline && size.blockSize === last.block) continue;
                    last.inline = size.inlineSize;
                    last.block = size.blockSize;
                    entries.push(new ResizeObserverEntry(target, measured));
                }
                if (entries.length === 0) return false;
                try {
                    this.#callback.call(this, entries, this);
                } catch (error) {
                    setTimeout(() => { throw error; }, 0);
                }
                return true;
            }
        }

        const parseMargin = (text) => {
            const parts = String(text ?? "0px").trim().split(/\s+/).filter(Boolean);
            const values = parts.map((part) => {
                const match = /^(-?[\d.]+)(px|%)?$/.exec(part);
                if (!match) throw new SyntaxError("rootMargin must be specified in pixels or percent.");
                return { value: parseFloat(match[1]), percent: match[2] === "%" };
            });
            if (parts.length === 1) return [values[0], values[0], values[0], values[0]];
            if (parts.length === 2) return [values[0], values[1], values[0], values[1]];
            if (parts.length === 3) return [values[0], values[1], values[2], values[1]];
            return values.slice(0, 4);
        };

        class IntersectionObserverEntry {
            constructor(init) {
                for (const key of Object.keys(init)) {
                    Object.defineProperty(this, key, { value: init[key], enumerable: true });
                }
            }
        }

        class IntersectionObserver {
            #callback;
            #root;
            #margin;
            #thresholds;
            #targets = new Map();
            constructor(callback, options = {}) {
                if (typeof callback !== "function") {
                    throw new TypeError("Failed to construct 'IntersectionObserver': parameter 1 is not of type 'Function'.");
                }
                this.#callback = callback;
                this.#root = options.root ?? null;
                this.#margin = parseMargin(options.rootMargin ?? "0px");
                const threshold = options.threshold ?? 0;
                this.#thresholds = (Array.isArray(threshold) ? threshold : [threshold]).map(Number).sort((a, b) => a - b);
                if (this.#thresholds.some((value) => !(value >= 0 && value <= 1))) {
                    throw new RangeError("Threshold values must be numbers between 0 and 1");
                }
            }
            get root() { return this.#root; }
            get rootMargin() {
                return this.#margin.map((part) => part.value + (part.percent ? "%" : "px")).join(" ");
            }
            get thresholds() { return Object.freeze([...this.#thresholds]); }
            observe(target) {
                if (!isElement(target)) throw new TypeError("IntersectionObserver.observe: target is not an Element");
                if (this.#targets.has(target)) return;
                this.#targets.set(target, { index: -1, intersecting: null });
                intersectionObservers.add(this);
                wake();
            }
            unobserve(target) {
                this.#targets.delete(target);
            }
            disconnect() {
                this.#targets.clear();
                intersectionObservers.delete(this);
            }
            takeRecords() {
                return [];
            }
            __pageframe_size() {
                return this.#targets.size;
            }
            #rootBounds() {
                const base = this.#root && isElement(this.#root)
                    ? this.#root.getBoundingClientRect()
                    : { left: 0, top: 0, right: innerWidth, bottom: innerHeight, width: innerWidth, height: innerHeight };
                const [top, right, bottom, left] = this.#margin;
                const amount = (part, size) => (part.percent ? (part.value / 100) * size : part.value);
                const box = {
                    left: base.left - amount(left, base.width),
                    top: base.top - amount(top, base.height),
                    right: base.right + amount(right, base.width),
                    bottom: base.bottom + amount(bottom, base.height),
                };
                return new DOMRectReadOnly(box.left, box.top, box.right - box.left, box.bottom - box.top);
            }
            __pageframe_check() {
                const entries = [];
                const rootBounds = this.#rootBounds();
                const rootConnected = this.#root === null || connected(this.#root);
                for (const [target, last] of this.#targets) {
                    const rect = target.getBoundingClientRect();
                    const left = Math.max(rect.left, rootBounds.left);
                    const top = Math.max(rect.top, rootBounds.top);
                    const right = Math.min(rect.right, rootBounds.right);
                    const bottom = Math.min(rect.bottom, rootBounds.bottom);
                    const contained = this.#root === null || this.#root.contains(target);
                    const intersecting = rootConnected && connected(target) && contained && right >= left && bottom >= top;
                    const area = rect.width * rect.height;
                    const overlap = intersecting ? Math.max(0, right - left) * Math.max(0, bottom - top) : 0;
                    const ratio = !intersecting ? 0 : area > 0 ? overlap / area : 1;
                    let index = -1;
                    this.#thresholds.forEach((threshold, i) => {
                        if (intersecting && ratio >= threshold) index = i;
                    });
                    if (index === last.index && intersecting === last.intersecting) continue;
                    last.index = index;
                    last.intersecting = intersecting;
                    entries.push(new IntersectionObserverEntry({
                        time: typeof performance === "object" ? performance.now() : Date.now(),
                        rootBounds,
                        boundingClientRect: rect,
                        intersectionRect: intersecting
                            ? new DOMRectReadOnly(left, top, Math.max(0, right - left), Math.max(0, bottom - top))
                            : new DOMRectReadOnly(0, 0, 0, 0),
                        isIntersecting: intersecting,
                        intersectionRatio: ratio,
                        target,
                    }));
                }
                if (entries.length === 0) return false;
                try {
                    this.#callback.call(this, entries, this);
                } catch (error) {
                    setTimeout(() => { throw error; }, 0);
                }
                return true;
            }
        }

        if (typeof globalThis.ResizeObserver !== "function") {
            globalThis.ResizeObserver = ResizeObserver;
            globalThis.ResizeObserverEntry = ResizeObserverEntry;
        }
        if (typeof globalThis.IntersectionObserver !== "function") {
            globalThis.IntersectionObserver = IntersectionObserver;
            globalThis.IntersectionObserverEntry = IntersectionObserverEntry;
        }
    }
