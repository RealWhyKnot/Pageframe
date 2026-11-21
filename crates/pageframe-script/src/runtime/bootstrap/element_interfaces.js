    {
        const htmlNamespace = "http://www.w3.org/1999/xhtml";
        const svgNamespace = "http://www.w3.org/2000/svg";
        const elementProto = document.documentElement
            ? Object.getPrototypeOf(document.documentElement)
            : null;
        const isElement = (value) =>
            elementProto !== null && typeof value === "object" && value !== null
            && elementProto.isPrototypeOf(value);
        const define = (name, test) => {
            if (typeof globalThis[name] !== "undefined") return;
            const iface = function () {
                throw new TypeError("Illegal constructor");
            };
            Object.defineProperty(iface, "name", { value: name, configurable: true });
            iface.prototype = Object.create(elementProto ?? Object.prototype);
            Object.defineProperty(iface.prototype, "constructor", {
                value: iface,
                writable: true,
                configurable: true,
            });
            Object.defineProperty(iface, Symbol.hasInstance, {
                value: (value) => isElement(value) && test(value),
                configurable: true,
            });
            globalThis[name] = iface;
        };
        const inHtml = (element) => (element.namespaceURI ?? htmlNamespace) === htmlNamespace;
        define("SVGElement", (element) => element.namespaceURI === svgNamespace);
        const tags = {
            HTMLAnchorElement: ["a"],
            HTMLAreaElement: ["area"],
            HTMLAudioElement: ["audio"],
            HTMLBRElement: ["br"],
            HTMLBodyElement: ["body"],
            HTMLButtonElement: ["button"],
            HTMLCanvasElement: ["canvas"],
            HTMLDListElement: ["dl"],
            HTMLDataListElement: ["datalist"],
            HTMLDetailsElement: ["details"],
            HTMLDialogElement: ["dialog"],
            HTMLDivElement: ["div"],
            HTMLEmbedElement: ["embed"],
            HTMLFieldSetElement: ["fieldset"],
            HTMLFormElement: ["form"],
            HTMLHRElement: ["hr"],
            HTMLHeadElement: ["head"],
            HTMLHeadingElement: ["h1", "h2", "h3", "h4", "h5", "h6"],
            HTMLHtmlElement: ["html"],
            HTMLIFrameElement: ["iframe"],
            HTMLImageElement: ["img"],
            HTMLInputElement: ["input"],
            HTMLLIElement: ["li"],
            HTMLLabelElement: ["label"],
            HTMLLegendElement: ["legend"],
            HTMLLinkElement: ["link"],
            HTMLMediaElement: ["audio", "video"],
            HTMLMetaElement: ["meta"],
            HTMLMeterElement: ["meter"],
            HTMLOListElement: ["ol"],
            HTMLObjectElement: ["object"],
            HTMLOptGroupElement: ["optgroup"],
            HTMLOptionElement: ["option"],
            HTMLOutputElement: ["output"],
            HTMLParagraphElement: ["p"],
            HTMLPictureElement: ["picture"],
            HTMLPreElement: ["pre", "listing", "xmp"],
            HTMLProgressElement: ["progress"],
            HTMLQuoteElement: ["blockquote", "q"],
            HTMLScriptElement: ["script"],
            HTMLSelectElement: ["select"],
            HTMLSlotElement: ["slot"],
            HTMLSourceElement: ["source"],
            HTMLSpanElement: ["span"],
            HTMLStyleElement: ["style"],
            HTMLTableCaptionElement: ["caption"],
            HTMLTableCellElement: ["td", "th"],
            HTMLTableColElement: ["col", "colgroup"],
            HTMLTableElement: ["table"],
            HTMLTableRowElement: ["tr"],
            HTMLTableSectionElement: ["thead", "tbody", "tfoot"],
            HTMLTemplateElement: ["template"],
            HTMLTextAreaElement: ["textarea"],
            HTMLTimeElement: ["time"],
            HTMLTitleElement: ["title"],
            HTMLTrackElement: ["track"],
            HTMLUListElement: ["ul"],
            HTMLVideoElement: ["video"],
        };
        for (const [name, localNames] of Object.entries(tags)) {
            define(name, (element) => inHtml(element) && localNames.includes(element.localName));
        }
    }
