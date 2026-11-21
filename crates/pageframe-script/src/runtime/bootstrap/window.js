    if (typeof globalThis.queueMicrotask !== "function") {
        globalThis.queueMicrotask = function (callback) {
            Promise.resolve().then(callback);
        };
    }
