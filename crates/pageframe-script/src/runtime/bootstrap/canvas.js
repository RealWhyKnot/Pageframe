    {
        const contextProto = CanvasRenderingContext2D.prototype;
        const readPixels = __pageframe_canvas_read;
        const writePixels = __pageframe_canvas_write;
        const createContext = __pageframe_canvas_create;
        const resizeContext = __pageframe_canvas_resize;
        const snapshotContext = __pageframe_canvas_snapshot;
        const contextMatrix = __pageframe_canvas_matrix;
        const encodeDataUrl = __pageframe_canvas_data_url;
        const encodeBytes = __pageframe_canvas_encode;
        const nativeDrawImage = contextProto.drawImage;
        const offscreenState = new WeakMap();
        const bitmapState = new WeakMap();
        const maxArea = 268435456;

        const toDimension = (value, name) => {
            const number = Number(value);
            if (!Number.isFinite(number)) {
                throw new TypeError(`Failed to construct '${name}': Value is not of type 'unsigned long'.`);
            }
            return Math.trunc(number) >>> 0;
        };

        class ImageData {
            #width;
            #height;
            #data;
            constructor(first, second, third) {
                if (first instanceof Uint8ClampedArray) {
                    const length = first.length;
                    const width = toDimension(second, "ImageData");
                    if (width === 0 || length % 4 !== 0 || (length / 4) % width !== 0) {
                        throw new DOMException(
                            "Failed to construct 'ImageData': The input data length is not a multiple of (4 * width).",
                            "IndexSizeError",
                        );
                    }
                    const height = length / 4 / width;
                    if (third !== undefined && toDimension(third, "ImageData") !== height) {
                        throw new DOMException(
                            "Failed to construct 'ImageData': The input data length is not equal to (4 * width * height).",
                            "IndexSizeError",
                        );
                    }
                    this.#width = width;
                    this.#height = height;
                    this.#data = first;
                    return;
                }
                const width = toDimension(first, "ImageData");
                const height = toDimension(second, "ImageData");
                if (width === 0 || height === 0) {
                    throw new DOMException(
                        `Failed to construct 'ImageData': The source ${width === 0 ? "width" : "height"} is zero or not a number.`,
                        "IndexSizeError",
                    );
                }
                if (width * height > maxArea) {
                    throw new RangeError("Out of memory at ImageData creation");
                }
                this.#width = width;
                this.#height = height;
                this.#data = new Uint8ClampedArray(width * height * 4);
            }
            get width() {
                return this.#width;
            }
            get height() {
                return this.#height;
            }
            get data() {
                return this.#data;
            }
            get colorSpace() {
                return "srgb";
            }
        }

        class ImageBitmap {
            constructor() {
                throw new TypeError("Illegal constructor");
            }
            get width() {
                return bitmapState.get(this)?.width ?? 0;
            }
            get height() {
                return bitmapState.get(this)?.height ?? 0;
            }
            close() {
                const state = bitmapState.get(this);
                if (state) {
                    state.width = 0;
                    state.height = 0;
                    state.context = null;
                }
            }
        }

        const encodeType = (type) => (type === undefined ? undefined : String(type));
        const encodeQuality = (quality) => (typeof quality === "number" ? quality : undefined);

        const encodeBlob = (source, type, quality) => {
            const encoded = encodeBytes(source, encodeType(type), encodeQuality(quality));
            if (!encoded) return null;
            return new Blob([encoded[1]], { type: encoded[0] });
        };

        const makeBitmap = (context, width, height) => {
            const bitmap = Object.create(ImageBitmap.prototype);
            bitmapState.set(bitmap, { context, width, height });
            return bitmap;
        };

        const offscreenContext = (canvas) => {
            const state = offscreenState.get(canvas);
            if (!state.context) {
                state.context = createContext(canvas, state.width, state.height);
            }
            return state.context;
        };

        class OffscreenCanvas extends EventTarget {
            constructor(width, height) {
                super();
                if (arguments.length < 2) {
                    throw new TypeError(
                        `Failed to construct 'OffscreenCanvas': 2 arguments required, but only ${arguments.length} present.`,
                    );
                }
                offscreenState.set(this, {
                    width: toDimension(width, "OffscreenCanvas"),
                    height: toDimension(height, "OffscreenCanvas"),
                    context: null,
                });
            }
            get width() {
                return offscreenState.get(this).width;
            }
            set width(value) {
                const state = offscreenState.get(this);
                state.width = toDimension(value, "OffscreenCanvas");
                if (state.context) resizeContext(state.context, state.width, state.height);
            }
            get height() {
                return offscreenState.get(this).height;
            }
            set height(value) {
                const state = offscreenState.get(this);
                state.height = toDimension(value, "OffscreenCanvas");
                if (state.context) resizeContext(state.context, state.width, state.height);
            }
            getContext(type) {
                const kind = String(type);
                if (kind === "2d") return offscreenContext(this);
                if (["bitmaprenderer", "webgl", "webgl2", "webgpu"].includes(kind)) return null;
                throw new TypeError(
                    `Failed to execute 'getContext' on 'OffscreenCanvas': The provided value '${kind}' is not a valid enum value of type OffscreenRenderingContextType.`,
                );
            }
            transferToImageBitmap() {
                const state = offscreenState.get(this);
                const copy = snapshotContext(offscreenContext(this));
                return makeBitmap(copy, state.width, state.height);
            }
            convertToBlob(options = {}) {
                const state = offscreenState.get(this);
                if (state.width === 0 || state.height === 0) {
                    return Promise.reject(new DOMException(
                        "Failed to execute 'convertToBlob' on 'OffscreenCanvas': The canvas has no pixels.",
                        "IndexSizeError",
                    ));
                }
                const { type, quality } = options ?? {};
                const blob = encodeBlob(offscreenContext(this), type, quality);
                if (!blob) {
                    return Promise.reject(new DOMException(
                        "Failed to execute 'convertToBlob' on 'OffscreenCanvas': Encoding failed.",
                        "EncodingError",
                    ));
                }
                return Promise.resolve(blob);
            }
        }

        const unwrapSource = (source) => {
            if (offscreenState.has(source)) return offscreenContext(source);
            if (bitmapState.has(source)) {
                const state = bitmapState.get(source);
                if (!state || !state.context) {
                    throw new DOMException(
                        "Failed to execute 'drawImage' on 'CanvasRenderingContext2D': The image source is detached.",
                        "InvalidStateError",
                    );
                }
                return state.context;
            }
            return source;
        };

        contextProto.drawImage = function drawImage(source, ...rest) {
            return nativeDrawImage.call(this, unwrapSource(source), ...rest);
        };

        const checkSize = (sw, sh, method) => {
            if (sw === 0 || sh === 0) {
                throw new DOMException(
                    `Failed to execute '${method}' on 'CanvasRenderingContext2D': The source ${sw === 0 ? "width" : "height"} is 0.`,
                    "IndexSizeError",
                );
            }
        };

        contextProto.createImageData = function createImageData(first, second) {
            if (first instanceof ImageData) return new ImageData(first.width, first.height);
            let sw = Math.trunc(Number(first));
            let sh = Math.trunc(Number(second));
            if (!Number.isFinite(sw) || !Number.isFinite(sh)) {
                throw new TypeError("Failed to execute 'createImageData' on 'CanvasRenderingContext2D': The provided double value is non-finite.");
            }
            checkSize(sw, sh, "createImageData");
            return new ImageData(Math.abs(sw), Math.abs(sh));
        };

        contextProto.getImageData = function getImageData(sx, sy, sw, sh) {
            if (arguments.length < 4) {
                throw new TypeError(
                    `Failed to execute 'getImageData' on 'CanvasRenderingContext2D': 4 arguments required, but only ${arguments.length} present.`,
                );
            }
            let [x, y, w, h] = [sx, sy, sw, sh].map((v) => Math.trunc(Number(v)));
            if (![x, y, w, h].every(Number.isFinite)) {
                throw new TypeError("Failed to execute 'getImageData' on 'CanvasRenderingContext2D': The provided double value is non-finite.");
            }
            checkSize(w, h, "getImageData");
            if (w < 0) {
                x += w;
                w = -w;
            }
            if (h < 0) {
                y += h;
                h = -h;
            }
            const buffer = readPixels(this, x, y, w, h);
            return new ImageData(new Uint8ClampedArray(buffer), w, h);
        };

        contextProto.putImageData = function putImageData(image, dx, dy, dirtyX, dirtyY, dirtyWidth, dirtyHeight) {
            if (!(image instanceof ImageData)) {
                throw new TypeError("Failed to execute 'putImageData' on 'CanvasRenderingContext2D': parameter 1 is not of type 'ImageData'.");
            }
            const x = Math.trunc(Number(dx));
            const y = Math.trunc(Number(dy));
            if (!Number.isFinite(x) || !Number.isFinite(y)) return;
            const dirty = arguments.length >= 7
                ? [dirtyX, dirtyY, dirtyWidth, dirtyHeight].map(Number)
                : [0, 0, image.width, image.height];
            if (!dirty.every(Number.isFinite)) return;
            const bytes = image.data.slice().buffer;
            writePixels(this, bytes, image.width, image.height, x, y, ...dirty);
        };

        contextProto.getTransform = function getTransform() {
            return new DOMMatrix(contextMatrix(this));
        };

        const canvasProto = Object.getPrototypeOf(document.createElement("canvas"));
        const defineCanvasMethod = (name, method) => {
            Object.defineProperty(canvasProto, name, { value: method, writable: true, configurable: true });
        };
        defineCanvasMethod("toDataURL", function toDataURL(type, quality) {
            return encodeDataUrl(this, encodeType(type), encodeQuality(quality));
        });
        defineCanvasMethod("toBlob", function toBlob(callback, type, quality) {
            if (typeof callback !== "function") {
                throw new TypeError("Failed to execute 'toBlob' on 'HTMLCanvasElement': parameter 1 is not of type 'BlobCallback'.");
            }
            const blob = encodeBlob(this, type, quality);
            setTimeout(() => callback(blob), 0);
        });

        globalThis.ImageData = ImageData;
        globalThis.ImageBitmap = ImageBitmap;
        globalThis.OffscreenCanvas = OffscreenCanvas;
        globalThis.OffscreenCanvasRenderingContext2D = CanvasRenderingContext2D;
    }
