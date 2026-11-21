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
                state = { returnValue: "", modal: false };
                dialogs.set(dialog, state);
            }
            return state;
        };
        const modalStack = [];
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
                if (dialogState(this).modal) throw invalidState("show", "The dialog is already open as a modal dialog.");
                return;
            }
            dialogState(this).modal = false;
            this.setAttribute("open", "");
        });
        method("showModal", function showModal() {
            if (!isTag(this, "dialog")) return;
            const state = dialogState(this);
            if (this.hasAttribute("open")) {
                if (state.modal) return;
                throw invalidState("showModal", "The dialog is already open as a non-modal dialog.");
            }
            if (!this.isConnected) throw invalidState("showModal", "The element is not in a Document.");
            state.modal = true;
            state.opener = document.activeElement;
            this.setAttribute("open", "");
            modalStack.push(this);
            const target = this.querySelector("[autofocus]") ?? this.querySelector("button, input, select, textarea, a[href], [tabindex]");
            if (target && typeof target.focus === "function") target.focus();
        });
        method("close", function close(result) {
            if (!isTag(this, "dialog") || !this.hasAttribute("open")) return;
            const state = dialogState(this);
            this.removeAttribute("open");
            if (result !== undefined) state.returnValue = String(result);
            state.modal = false;
            const index = modalStack.indexOf(this);
            if (index >= 0) modalStack.splice(index, 1);
            setTimeout(() => this.dispatchEvent(new Event("close")), 0);
        });
        method("requestClose", function requestClose(result) {
            if (!isTag(this, "dialog") || !this.hasAttribute("open")) return;
            if (this.dispatchEvent(new Event("cancel", { cancelable: true }))) this.close(result);
        });
        globalThis.addEventListener("keydown", (event) => {
            if (event.key !== "Escape" || event.defaultPrevented) return;
            const dialog = modalStack[modalStack.length - 1];
            if (!dialog || !dialog.isConnected || !dialog.hasAttribute("open")) return;
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
    }
