    const setDialogModal = __pageframe_dialog_set_modal;
    const isDialogModal = __pageframe_dialog_is_modal;
    const activeModal = __pageframe_active_modal;
    delete globalThis.__pageframe_dialog_set_modal;
    delete globalThis.__pageframe_dialog_is_modal;
    delete globalThis.__pageframe_active_modal;
    if (document.documentElement) {
        const elementProto = Object.getPrototypeOf(document.documentElement);
        const isTag = (element, name) => element.localName === name && (element.namespaceURI ?? "http://www.w3.org/1999/xhtml") === "http://www.w3.org/1999/xhtml";
        const method = (name, value) => {
            if (name in elementProto) return;
            Object.defineProperty(elementProto, name, { value, writable: true, configurable: true });
        };
        const accessor = (name, get, set) => {
            if (name in elementProto) return;
            Object.defineProperty(elementProto, name, { get, set, configurable: true, enumerable: true });
        };
        const invalidState = (operation, message) =>
            new DOMException("Failed to execute '" + operation + "' on 'HTMLDialogElement': " + message, "InvalidStateError");

        function Image(width, height) {
            if (new.target === undefined) {
                throw new TypeError("Failed to construct 'Image': Please use the 'new' operator, this DOM object constructor cannot be called as a function.");
            }
            const image = document.createElement("img");
            if (width !== undefined) image.width = width;
            if (height !== undefined) image.height = height;
            return image;
        }
        Image.prototype = globalThis.HTMLImageElement.prototype;
        globalThis.Image = Image;

        method("decode", function decode() {
            if (!isTag(this, "img")) return Promise.resolve();
            const settle = (resolve, reject) => {
                if (this.naturalWidth > 0 || this.naturalHeight > 0) resolve();
                else reject(new DOMException("The source image cannot be decoded.", "EncodingError"));
            };
            return new Promise((resolve, reject) => {
                if (this.complete) {
                    queueMicrotask(() => settle(resolve, reject));
                    return;
                }
                const done = () => {
                    this.removeEventListener("load", done);
                    this.removeEventListener("error", done);
                    settle(resolve, reject);
                };
                this.addEventListener("load", done);
                this.addEventListener("error", done);
            });
        });

        const dialogs = new WeakMap();
        const dialogState = (dialog) => {
            let state = dialogs.get(dialog);
            if (!state) {
                state = { returnValue: "", opener: null };
                dialogs.set(dialog, state);
            }
            return state;
        };
        const focusDialog = (dialog) => {
            const target = dialog.querySelector("[autofocus]") ?? dialog.querySelector("button, input, select, textarea, a[href], [tabindex]");
            const focusable = target && typeof target.focus === "function" ? target : dialog;
            if (typeof focusable.focus === "function") focusable.focus();
        };
        const restoreFocus = (dialog, opener) => {
            const active = document.activeElement;
            const usable = opener && opener !== document.body && opener !== document.documentElement
                && opener.isConnected && typeof opener.focus === "function"
                && !(typeof opener.closest === "function" && opener.closest("dialog:not([open])"));
            if (usable) {
                try {
                    opener.focus();
                } catch {
                    return;
                }
                if (document.activeElement === opener) return;
            }
            if (active && active !== document.body && dialog.contains(active) && typeof active.blur === "function") {
                active.blur();
            }
        };
        accessor("open",
            function () { return this.hasAttribute("open"); },
            function (value) {
                if (value) this.setAttribute("open", "");
                else this.removeAttribute("open");
            });
        accessor("returnValue",
            function () { return isTag(this, "dialog") ? dialogState(this).returnValue : undefined; },
            function (value) {
                if (isTag(this, "dialog")) dialogState(this).returnValue = String(value);
                else Object.defineProperty(this, "returnValue", { value, writable: true, configurable: true, enumerable: true });
            });
        method("show", function show() {
            if (!isTag(this, "dialog")) return;
            if (this.hasAttribute("open")) {
                if (isDialogModal(this)) throw invalidState("show", "The dialog is already open as a modal dialog.");
                return;
            }
            dialogState(this).opener = document.activeElement;
            this.setAttribute("open", "");
        });
        method("showModal", function showModal() {
            if (!isTag(this, "dialog")) return;
            const state = dialogState(this);
            if (this.hasAttribute("open")) {
                if (isDialogModal(this)) return;
                throw invalidState("showModal", "The dialog is already open as a non-modal dialog.");
            }
            if (!this.isConnected) throw invalidState("showModal", "The element is not in a Document.");
            state.opener = document.activeElement;
            this.setAttribute("open", "");
            setDialogModal(this, true);
            focusDialog(this);
        });
        method("close", function close(result) {
            if (!isTag(this, "dialog") || !this.hasAttribute("open")) return;
            const state = dialogState(this);
            const wasModal = isDialogModal(this);
            const active = document.activeElement;
            const hadFocus = active !== null && active !== document.body && this.contains(active);
            this.removeAttribute("open");
            if (result !== undefined) state.returnValue = String(result);
            setDialogModal(this, false);
            const opener = state.opener;
            state.opener = null;
            if (wasModal || hadFocus) restoreFocus(this, opener);
            setTimeout(() => this.dispatchEvent(new Event("close")), 0);
        });
        method("requestClose", function requestClose(result) {
            if (!isTag(this, "dialog") || !this.hasAttribute("open")) return;
            if (this.dispatchEvent(new Event("cancel", { cancelable: true }))) this.close(result);
        });
        globalThis.addEventListener("keydown", (event) => {
            if (event.key !== "Escape" || event.defaultPrevented) return;
            const dialog = activeModal();
            if (!dialog || !dialog.hasAttribute("open")) return;
            event.preventDefault();
            dialog.requestClose();
        });

        const captures = new WeakMap();
        const owners = new Map();
        const notFound = (operation) =>
            new DOMException("Failed to execute '" + operation + "' on 'Element': No active pointer with the given id is found.", "NotFoundError");
        const pointerArg = (operation, args) => {
            if (args.length === 0) {
                throw new TypeError("Failed to execute '" + operation + "' on 'Element': 1 argument required, but only 0 present.");
            }
            const id = Number(args[0]);
            if (!Number.isFinite(id)) throw notFound(operation);
            return Math.trunc(id);
        };
        const lose = (element, pointerId) => {
            captures.get(element)?.delete(pointerId);
            if (owners.get(pointerId) === element) owners.delete(pointerId);
            element.dispatchEvent(new PointerEvent("lostpointercapture", { pointerId, bubbles: true }));
        };
        method("setPointerCapture", function setPointerCapture() {
            const pointerId = pointerArg("setPointerCapture", arguments);
            if (!this.isConnected) {
                throw new DOMException("Failed to execute 'setPointerCapture' on 'Element': InvalidStateError", "InvalidStateError");
            }
            const previous = owners.get(pointerId);
            if (previous === this) return;
            if (previous) lose(previous, pointerId);
            let set = captures.get(this);
            if (!set) {
                set = new Set();
                captures.set(this, set);
            }
            set.add(pointerId);
            owners.set(pointerId, this);
            this.dispatchEvent(new PointerEvent("gotpointercapture", { pointerId, bubbles: true }));
        });
        method("releasePointerCapture", function releasePointerCapture() {
            const pointerId = pointerArg("releasePointerCapture", arguments);
            if (owners.get(pointerId) === this) lose(this, pointerId);
        });
        method("hasPointerCapture", function hasPointerCapture() {
            const pointerId = pointerArg("hasPointerCapture", arguments);
            return captures.get(this)?.has(pointerId) === true;
        });
        const implicitRelease = (event) => {
            const owner = owners.get(event.pointerId);
            if (owner) lose(owner, event.pointerId);
        };
        globalThis.addEventListener("pointerup", implicitRelease);
        globalThis.addEventListener("pointercancel", implicitRelease);

        const clicking = new WeakSet();
        const formControls = new Set(["button", "input", "select", "textarea", "optgroup", "option", "fieldset"]);
        const isDisabled = (element) => {
            if (!formControls.has(element.localName)) return false;
            if (element.hasAttribute("disabled")) return true;
            for (let node = element.parentElement; node; node = node.parentElement) {
                if (node.localName !== "fieldset" || !node.hasAttribute("disabled")) continue;
                const legend = [...node.children].find((child) => child.localName === "legend");
                if (!legend || !legend.contains(element)) return true;
            }
            return false;
        };
        const ownerForm = (element) => {
            const id = element.getAttribute("form");
            if (id) return document.getElementById(id);
            return element.closest("form");
        };
        const labelControl = (label) => {
            const id = label.getAttribute("for");
            if (id) return document.getElementById(id);
            return label.querySelector("button, input:not([type=hidden]), meter, output, progress, select, textarea");
        };
        const fire = (target, type) => target.dispatchEvent(new Event(type, { bubbles: true }));
        const preActivate = (element) => {
            if (element.localName !== "input") return null;
            const type = (element.getAttribute("type") || "").toLowerCase();
            if (type === "checkbox") {
                const was = element.checked;
                element.checked = !was;
                return () => { element.checked = was; };
            }
            if (type !== "radio" || element.checked) return null;
            const name = element.getAttribute("name");
            const form = ownerForm(element);
            const scope = form || document;
            const previous = name
                ? [...scope.querySelectorAll("input[type=radio]")].find((other) => other !== element
                    && other.checked && other.getAttribute("name") === name && ownerForm(other) === form)
                : undefined;
            element.checked = true;
            return () => {
                element.checked = false;
                if (previous) previous.checked = true;
            };
        };
        const activate = (element, event, changed) => {
            const name = element.localName;
            const type = (element.getAttribute("type") || "").toLowerCase();
            if (name === "input" && (type === "checkbox" || (type === "radio" && changed))) {
                fire(element, "input");
                fire(element, "change");
                return;
            }
            if ((name === "a" || name === "area") && element.hasAttribute("href")) {
                location.assign(element.href);
                return;
            }
            if (name === "label") {
                const control = labelControl(element);
                if (control && control !== event.target && !control.contains(event.target)) control.click();
                return;
            }
            const submits = (name === "button" && (type === "" || type === "submit"))
                || (name === "input" && (type === "submit" || type === "image"));
            const form = submits || type === "reset" ? ownerForm(element) : null;
            if (!form) return;
            if (type === "reset") {
                if (typeof form.reset === "function") form.reset();
            } else if (typeof form.requestSubmit === "function") {
                form.requestSubmit(element);
            } else {
                const submit = new Event("submit", { bubbles: true, cancelable: true });
                Object.defineProperty(submit, "submitter", { value: element });
                form.dispatchEvent(submit);
            }
        };
        method("click", function click() {
            if (isDisabled(this) || clicking.has(this)) return;
            clicking.add(this);
            try {
                const revert = preActivate(this);
                const event = new MouseEvent("click", {
                    bubbles: true, cancelable: true, composed: true, view: globalThis, detail: 0,
                });
                if (!this.dispatchEvent(event)) {
                    if (revert) revert();
                    return;
                }
                activate(this, event, revert !== null);
            } finally {
                clicking.delete(this);
            }
        });
    }
