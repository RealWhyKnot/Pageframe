    {
        const currentUrl = () => new URL(__pageframe_document_url());
        const entries = [{ url: __pageframe_document_url(), state: null }];
        let entryIndex = 0;
        const sameDocument = (a, b) => {
            const left = new URL(a.href);
            const right = new URL(b.href);
            left.hash = "";
            right.hash = "";
            return left.href === right.href;
        };
        const commit = (href) => {
            __pageframe_set_document_url(href);
            entries[entryIndex].url = href;
        };
        const fireHashChange = (oldURL, newURL, state) => {
            setTimeout(() => {
                globalThis.dispatchEvent(new PopStateEvent("popstate", { state }));
                if (oldURL !== newURL && sameDocument(new URL(oldURL), new URL(newURL))) {
                    globalThis.dispatchEvent(new HashChangeEvent("hashchange", { oldURL, newURL }));
                }
            }, 0);
        };
        const securityError = (url) =>
            new DOMException("Failed to execute on 'History': A history state object with URL '" + url + "' cannot be created in a document with origin '" + currentUrl().origin + "'.", "SecurityError");
        const navigate = (input, replace) => {
            const current = currentUrl();
            let target;
            try {
                target = new URL(String(input), current.href);
            } catch (error) {
                throw new DOMException("Failed to execute on 'Location': '" + input + "' is not a valid URL.", "SyntaxError");
            }
            if (target.href === current.href) {
                if (target.hash === "" && !String(input).endsWith("#")) __pageframe_navigate(target.href);
                return;
            }
            if (sameDocument(current, target) && (target.hash !== "" || String(input).includes("#"))) {
                if (replace) {
                    entries[entryIndex] = { url: target.href, state: null };
                } else {
                    entries.splice(entryIndex + 1);
                    entries.push({ url: target.href, state: null });
                    entryIndex = entries.length - 1;
                }
                commit(target.href);
                fireHashChange(current.href, target.href, null);
                return;
            }
            __pageframe_navigate(target.href);
        };
        const withPart = (part, value) => {
            const url = currentUrl();
            url[part] = value;
            return url.href;
        };

        class Location {
            get href() { return currentUrl().href; }
            set href(value) { navigate(value, false); }
            get origin() { return currentUrl().origin; }
            get protocol() { return currentUrl().protocol; }
            set protocol(value) { navigate(withPart("protocol", value), false); }
            get host() { return currentUrl().host; }
            set host(value) { navigate(withPart("host", value), false); }
            get hostname() { return currentUrl().hostname; }
            set hostname(value) { navigate(withPart("hostname", value), false); }
            get port() { return currentUrl().port; }
            set port(value) { navigate(withPart("port", value), false); }
            get pathname() { return currentUrl().pathname; }
            set pathname(value) { navigate(withPart("pathname", value), false); }
            get search() { return currentUrl().search; }
            set search(value) { navigate(withPart("search", value), false); }
            get hash() { return currentUrl().hash; }
            set hash(value) {
                const text = String(value);
                const fragment = text.startsWith("#") ? text.slice(1) : text;
                const url = currentUrl();
                if (url.hash.slice(1) === fragment && (fragment !== "" || url.href.endsWith("#"))) return;
                url.hash = fragment;
                navigate(url.href + (fragment === "" ? "#" : ""), false);
            }
            get ancestorOrigins() { return []; }
            assign(url) { navigate(url, false); }
            replace(url) { navigate(url, true); }
            reload() { __pageframe_navigate(currentUrl().href); }
            toString() { return this.href; }
            valueOf() { return this; }
        }
        const location = new Location();
        Object.defineProperty(globalThis, "location", {
            configurable: true,
            enumerable: true,
            get() { return location; },
            set(value) { location.href = value; },
        });
        globalThis.Location = Location;

        const traverse = (delta) => {
            const next = entryIndex + delta;
            if (next < 0 || next >= entries.length) return;
            const previous = entries[entryIndex];
            entryIndex = next;
            const entry = entries[next];
            __pageframe_set_document_url(entry.url);
            fireHashChange(previous.url, entry.url, entry.state);
        };
        const canRewrite = (current, target) => {
            if (["protocol", "username", "password", "host", "port"].some((part) => current[part] !== target[part])) {
                return false;
            }
            if (target.protocol === "http:" || target.protocol === "https:") return true;
            if (target.protocol === "file:") return current.pathname === target.pathname;
            return sameDocument(current, target);
        };
        const stateUrl = (url) => {
            const current = currentUrl();
            if (url === undefined || url === null) return current.href;
            let target;
            try {
                target = new URL(String(url), current.href);
            } catch (error) {
                throw new DOMException("Failed to execute on 'History': '" + url + "' is not a valid URL.", "SyntaxError");
            }
            if (!canRewrite(current, target)) throw securityError(target.href);
            return target.href;
        };
        class History {
            get length() { return entries.length; }
            get state() { return entries[entryIndex].state; }
            get scrollRestoration() { return this._scrollRestoration ?? "auto"; }
            set scrollRestoration(value) {
                if (value === "auto" || value === "manual") this._scrollRestoration = value;
            }
            pushState(state, _unused, url) {
                const href = stateUrl(url);
                entries.splice(entryIndex + 1);
                entries.push({ url: href, state: state === undefined ? null : structuredClone(state) });
                entryIndex = entries.length - 1;
                commit(href);
            }
            replaceState(state, _unused, url) {
                const href = stateUrl(url);
                entries[entryIndex] = { url: href, state: state === undefined ? null : structuredClone(state) };
                commit(href);
            }
            go(delta) {
                const steps = Math.trunc(Number(delta) || 0);
                if (steps === 0) {
                    location.reload();
                    return;
                }
                setTimeout(() => traverse(steps), 0);
            }
            back() { this.go(-1); }
            forward() { this.go(1); }
        }
        globalThis.History = History;
        globalThis.history = new History();

        const documentProto = Object.getPrototypeOf(document);
        const documentAccessors = {
            location: { get() { return location; }, set(value) { location.href = value; } },
            URL: { get() { return currentUrl().href; } },
            documentURI: { get() { return currentUrl().href; } },
            baseURI: { get() { return currentUrl().href; } },
            visibilityState: { get() { return "visible"; } },
            hidden: { get() { return false; } },
        };
        for (const [name, accessor] of Object.entries(documentAccessors)) {
            if (!(name in documentProto)) {
                Object.defineProperty(documentProto, name, { configurable: true, enumerable: true, ...accessor });
            }
        }
    }
