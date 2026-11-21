use pageframe_dom::DocumentConfig;
use pageframe_script::ScriptDocument;

fn run(body: &str) -> String {
    let mut doc =
        ScriptDocument::from_html("<!doctype html><body></body>", DocumentConfig::default());
    doc.eval(&format!(
        "__pageframe_send_message(String((() => {{\n{body}\n}})()));"
    ));
    assert_eq!(doc.take_js_errors(), Vec::<String>::new());
    doc.take_messages().join("\n")
}

fn tokens(text: &str) -> Vec<&str> {
    text.split(['|', ' ', ',']).collect()
}

fn assert_close(actual: &str, expected: &str, tolerance: f64) {
    let (got, want) = (tokens(actual), tokens(expected));
    let same = got.len() == want.len()
        && got
            .iter()
            .zip(&want)
            .all(|(g, w)| match (g.parse::<f64>(), w.parse::<f64>()) {
                (Ok(g), Ok(w)) => (g - w).abs() <= tolerance,
                _ => g == w,
            });
    assert!(same, "got      {actual}\nexpected {expected}");
}

#[test]
fn identity_matches_chrome() {
    let script = r##"
const c = document.createElement("canvas");
const ctx = c.getContext("2d");
return [ctx === c.getContext("2d"), ctx.canvas === c, ctx instanceof CanvasRenderingContext2D,
    c.getContext("bogus"), c.width, c.height].join(",");
"##;
    let expected = r##"true,true,true,,300,150"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn defaults_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
return ["fillStyle", "strokeStyle", "globalAlpha", "globalCompositeOperation", "lineWidth", "lineCap",
    "lineJoin", "miterLimit", "lineDashOffset", "font", "textAlign", "textBaseline", "direction",
    "shadowColor", "shadowBlur", "shadowOffsetX", "shadowOffsetY", "imageSmoothingEnabled",
    "imageSmoothingQuality", "filter", "letterSpacing"].map((k) => k + "=" + ctx[k]).join("|");
"##;
    let expected = r##"fillStyle=#000000|strokeStyle=#000000|globalAlpha=1|globalCompositeOperation=source-over|lineWidth=1|lineCap=butt|lineJoin=miter|miterLimit=10|lineDashOffset=0|font=10px sans-serif|textAlign=start|textBaseline=alphabetic|direction=inherit|shadowColor=rgba(0, 0, 0, 0)|shadowBlur=0|shadowOffsetX=0|shadowOffsetY=0|imageSmoothingEnabled=true|imageSmoothingQuality=low|filter=none|letterSpacing=0px"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn setters_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
const out = [];
ctx.fillStyle = "red"; out.push(ctx.fillStyle);
ctx.fillStyle = "rgba(255, 0, 0, 0.5)"; out.push(ctx.fillStyle);
ctx.fillStyle = "nope"; out.push(ctx.fillStyle);
ctx.strokeStyle = "#0f08"; out.push(ctx.strokeStyle);
ctx.strokeStyle = "hsl(120, 100%, 25%)"; out.push(ctx.strokeStyle);
ctx.lineWidth = -1; out.push(ctx.lineWidth);
ctx.lineWidth = 2.5; out.push(ctx.lineWidth);
ctx.globalAlpha = 2; out.push(ctx.globalAlpha);
ctx.globalAlpha = 0.25; out.push(ctx.globalAlpha);
ctx.globalCompositeOperation = "bogus"; out.push(ctx.globalCompositeOperation);
ctx.globalCompositeOperation = "destination-out"; out.push(ctx.globalCompositeOperation);
ctx.lineCap = "round"; ctx.lineCap = "nope"; out.push(ctx.lineCap);
ctx.lineJoin = "bevel"; out.push(ctx.lineJoin);
ctx.textAlign = "center"; ctx.textAlign = "middle"; out.push(ctx.textAlign);
ctx.textBaseline = "middle"; out.push(ctx.textBaseline);
ctx.font = "11px 'Segoe UI', system-ui, sans-serif"; out.push(ctx.font);
ctx.font = "600 15px/12px 'Segoe UI', system-ui, sans-serif"; out.push(ctx.font);
ctx.font = "italic bold 12pt Arial"; out.push(ctx.font);
ctx.font = "garbage"; out.push(ctx.font);
ctx.shadowColor = "rgba(0, 0, 0, 0.35)"; out.push(ctx.shadowColor);
ctx.shadowBlur = -3; out.push(ctx.shadowBlur);
ctx.imageSmoothingQuality = "high"; out.push(ctx.imageSmoothingQuality);
ctx.setLineDash([4, 2, 1]); out.push(ctx.getLineDash().join(" "));
ctx.setLineDash([1, -1]); out.push(ctx.getLineDash().join(" "));
return out.join("|");
"##;
    let expected = r##"#ff0000|rgba(255, 0, 0, 0.5)|rgba(255, 0, 0, 0.5)|rgba(0, 255, 0, 0.533)|#008000|1|2.5|1|0.25|source-over|destination-out|round|bevel|center|middle|11px "Segoe UI", system-ui, sans-serif|600 15px "Segoe UI", system-ui, sans-serif|italic bold 16px Arial|italic bold 16px Arial|rgba(0, 0, 0, 0.35)|0|high|4 2 1 4 2 1|4 2 1 4 2 1"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn stack_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
ctx.restore();
ctx.fillStyle = "#123456";
ctx.lineWidth = 3;
ctx.save();
ctx.fillStyle = "#abcdef";
ctx.lineWidth = 7;
ctx.translate(10, 20);
ctx.setLineDash([5, 5]);
const inner = [ctx.fillStyle, ctx.lineWidth, ctx.getTransform().e, ctx.getLineDash().length].join(",");
ctx.restore();
const outer = [ctx.fillStyle, ctx.lineWidth, ctx.getTransform().e, ctx.getLineDash().length].join(",");
ctx.save();
ctx.reset();
return inner + "|" + outer + "|" + ctx.fillStyle;
"##;
    let expected = r##"#abcdef,7,10,2|#123456,3,0,0|#000000"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn transform_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
ctx.translate(5, 6);
ctx.scale(2, 3);
const a = ctx.getTransform();
ctx.setTransform(1, 0, 0, 1, 7, 8);
const b = ctx.getTransform();
ctx.setTransform({ a: 2, d: 2 });
const c = ctx.getTransform();
ctx.resetTransform();
const d = ctx.getTransform();
return [a, b, c, d].map((m) => [m.a, m.b, m.c, m.d, m.e, m.f].join(" ")).join("|") + "|" + (a instanceof DOMMatrix);
"##;
    let expected = r##"2 0 0 3 5 6|1 0 0 1 7 8|2 0 0 2 0 0|1 0 0 1 0 0|true"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn pixels_matches_chrome() {
    let script = r##"
const c = document.createElement("canvas");
c.width = 20;
c.height = 10;
const ctx = c.getContext("2d");
ctx.fillStyle = "rgba(255, 0, 0, 0.5)";
ctx.fillRect(0, 0, 10, 10);
ctx.fillStyle = "#00f";
ctx.fillRect(10.5, 0, 5, 10);
const d = ctx.getImageData(0, 0, 20, 1).data;
const at = (x) => Array.from(d.slice(x * 4, x * 4 + 4)).join(" ");
return [at(0), at(10), at(11), at(15), at(16), d.length].join("|");
"##;
    let expected = r##"255 0 0 128|0 0 255 128|0 0 255 255|0 0 255 128|0 0 0 0|80"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn image_data_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
const img = ctx.createImageData(2, 2);
img.data.set([255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255, 10, 20, 30, 0]);
ctx.putImageData(img, 3, 4);
const back = ctx.getImageData(3, 4, 2, 2);
const partial = ctx.getImageData(-1, -1, 2, 2).data[3];
let err = "";
try { ctx.getImageData(0, 0, 0, 5); } catch (e) { err = e.name; }
return [Array.from(back.data).join(" "), back.width, back.height, back instanceof ImageData, partial, err,
    new ImageData(3, 1).data.length].join("|");
"##;
    let expected = r##"255 0 0 255 0 255 0 128 0 0 255 255 0 0 0 0|2|2|true|0|IndexSizeError|12"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn errors_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
const names = [];
const grad = ctx.createLinearGradient(0, 0, 10, 0);
for (const f of [() => ctx.arc(0, 0, -1, 0, 1), () => grad.addColorStop(2, "red"),
    () => grad.addColorStop(0, "nope"), () => ctx.ellipse(0, 0, 1, -1, 0, 0, 1),
    () => ctx.createRadialGradient(0, 0, -1, 0, 0, 1), () => ctx.roundRect(0, 0, 1, 1, [1, 2, 3, 4, 5])]) {
    try { f(); names.push("ok"); } catch (e) { names.push(e.name); }
}
return names.join(",") + "|" + (grad instanceof CanvasGradient);
"##;
    let expected = r##"IndexSizeError,IndexSizeError,SyntaxError,IndexSizeError,IndexSizeError,RangeError|true"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn gradient_matches_chrome() {
    let script = r##"
const c = document.createElement("canvas");
c.width = 100;
c.height = 1;
const ctx = c.getContext("2d");
const g = ctx.createLinearGradient(0, 0, 100, 0);
g.addColorStop(0, "#ff0000");
g.addColorStop(1, "#0000ff");
ctx.fillStyle = g;
ctx.fillRect(0, 0, 100, 1);
const d = ctx.getImageData(0, 0, 100, 1).data;
const at = (x) => Array.from(d.slice(x * 4, x * 4 + 4)).join(" ");
return [at(0), at(25), at(50), at(99), ctx.fillStyle instanceof CanvasGradient].join("|");
"##;
    let expected = r##"253 0 1 255|190 0 65 255|126 0 128 255|2 0 254 255|true"##;
    assert_close(&run(script), expected, 2.0);
}

#[test]
fn offscreen_matches_chrome() {
    let script = r##"
const off = new OffscreenCanvas(20, 10);
const octx = off.getContext("2d");
octx.fillStyle = "#00ff00";
octx.fillRect(0, 0, 20, 10);
const c = document.createElement("canvas");
c.width = 40;
c.height = 10;
const ctx = c.getContext("2d");
ctx.drawImage(off, 10, 0);
ctx.drawImage(off, 0, 0, 10, 10, 30, 0, 5, 5);
const d = ctx.getImageData(0, 0, 40, 10).data;
const at = (x, y) => Array.from(d.slice((y * 40 + x) * 4, (y * 40 + x) * 4 + 4)).join(" ");
const same = octx === off.getContext("2d") && octx.canvas === off;
off.width = 5;
const bitmap = off.transferToImageBitmap();
return [same, at(9, 0), at(10, 0), at(29, 9), at(32, 2), at(32, 7), off.width, off.height,
    bitmap.width, bitmap.height, off.getContext("webgl")].join("|");
"##;
    let expected = r##"true|0 0 0 0|0 255 0 255|0 255 0 255|0 255 0 255|0 0 0 0|5|10|5|10|"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn resize_matches_chrome() {
    let script = r##"
const c = document.createElement("canvas");
const ctx = c.getContext("2d");
ctx.fillStyle = "red";
ctx.translate(3, 3);
ctx.fillRect(0, 0, 300, 150);
const before = ctx.getImageData(5, 5, 1, 1).data[3];
c.width = c.width;
const after = ctx.getImageData(5, 5, 1, 1).data[3];
const state = ctx.fillStyle + " " + ctx.getTransform().e;
c.height = 40;
const size = ctx.getImageData(0, 0, 1, 1).data.length + " " + c.height + " " + (ctx === c.getContext("2d"));
c.setAttribute("width", "64");
return [before, after, state, size, c.width, ctx.canvas.width].join("|");
"##;
    let expected = r##"255|0|#000000 0|4 40 true|64|64"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn canvas_copy_matches_chrome() {
    let script = r##"
const a = document.createElement("canvas");
a.width = 4;
a.height = 4;
const actx = a.getContext("2d");
actx.fillStyle = "#ff00ff";
actx.fillRect(0, 0, 2, 4);
const b = document.createElement("canvas");
b.width = 4;
b.height = 4;
const bctx = b.getContext("2d");
bctx.drawImage(a, 0, 0);
bctx.drawImage(b, 2, 0);
const d = bctx.getImageData(0, 0, 4, 1).data;
return Array.from(d).join(" ");
"##;
    let expected = r##"255 0 255 255 255 0 255 255 255 0 255 255 255 0 255 255"##;
    assert_close(&run(script), expected, 0.0);
}

#[test]
fn measure_matches_chrome() {
    let script = r##"
const ctx = document.createElement("canvas").getContext("2d");
const out = [];
for (const font of ["10px sans-serif", "11px 'Segoe UI', system-ui, sans-serif", "600 15px/12px 'Segoe UI', system-ui, sans-serif", "9px 'Segoe UI', system-ui, sans-serif"]) {
    ctx.font = font;
    for (const text of ["Hello", "1.25 ms", "60 fps", "Frame time"]) {
        const m = ctx.measureText(text);
        out.push([m.width, m.actualBoundingBoxAscent, m.actualBoundingBoxDescent, m.fontBoundingBoxAscent,
            m.fontBoundingBoxDescent, m.actualBoundingBoxLeft, m.actualBoundingBoxRight].map((v) => v.toFixed(3)).join(" "));
    }
}
return out.join("|");
"##;
    let expected = r##"22.788 7.000 0.000 9.000 2.000 0.000 23.227|35.571 7.000 0.000 9.000 2.000 0.000 35.571|27.241 7.000 2.000 9.000 2.000 0.000 27.241|50.562 7.000 0.000 9.000 2.000 0.000 51.000|25.335 8.000 0.000 12.000 3.000 0.000 25.890|37.329 8.000 0.000 12.000 3.000 0.000 37.662|29.450 8.000 2.000 12.000 3.000 0.000 29.782|54.651 8.000 0.000 12.000 3.000 0.000 54.898|35.793 11.000 0.000 16.000 4.000 -1.000 35.836|50.186 11.000 0.000 16.000 4.000 0.000 50.718|41.499 11.000 3.000 16.000 4.000 0.000 42.032|76.890 11.000 0.000 16.000 4.000 -1.000 76.921|20.729 7.000 0.000 10.000 2.000 0.000 21.456|30.542 7.000 0.000 10.000 2.000 0.000 30.723|24.095 7.000 2.000 10.000 2.000 0.000 24.276|44.714 7.000 0.000 10.000 2.000 0.000 45.008"##;
    assert_close(&run(script), expected, 1.0);
}

#[test]
fn hostile_inputs_stay_bounded() {
    let script = r##"
const big = document.createElement("canvas");
big.width = 100000;
const out = [big.width, big.getContext("2d").getImageData(0, 0, 1, 1).data.join(" ")];
const c = document.createElement("canvas");
c.width = 20;
c.height = 20;
const d = c.getContext("2d");
d.arc(10, 10, 1e300, 0, 6.28);
d.fill();
d.beginPath();
d.ellipse(10, 10, 1e200, 1e250, 1e300, -1e300, 1e300);
d.moveTo(0, 0);
d.bezierCurveTo(1e150, 1e150, -1e150, 1e150, 10, 10);
d.arcTo(1e300, 0, 1e300, 1e-300, 1e300);
d.roundRect(0, 0, 1e300, 1e300, [1e300]);
d.stroke();
d.fill();
d.setLineDash([1e-300]);
d.beginPath();
d.moveTo(0, 0);
d.lineTo(20, 20);
d.stroke();
d.setLineDash([]);
d.lineWidth = 1e300;
d.stroke();
d.shadowColor = "black";
d.shadowBlur = 1e300;
d.shadowOffsetX = 1e300;
d.fillRect(0, 0, 10, 10);
d.shadowOffsetX = -1e300;
d.shadowOffsetY = 1e300;
d.fillRect(0, 0, 10, 10);
d.shadowColor = "transparent";
d.putImageData(d.createImageData(2, 2), 1e300, -1e300);
d.putImageData(d.createImageData(2, 2), -1e300, 1e300, -1e300, -1e300, 1e300, 1e300);
out.push(d.getImageData(1e300, -1e300, 1, 1).data.join(" "));
d.font = "1e38in serif";
out.push(d.font);
d.scale(1e200, 1e200);
d.scale(1e200, 1e200);
d.fillRect(0, 0, 1, 1);
d.resetTransform();
d.lineWidth = 3;
d.save();
d.lineWidth = 7;
d.setLineDash(new Array(65535).fill(1));
for (let i = 0; i < 5000; i++) d.save();
for (let i = 0; i < 5000; i++) d.restore();
out.push(d.lineWidth, d.getLineDash().length);
for (let i = 0; i < 1000; i++) d.restore();
out.push(d.lineWidth);
for (const make of [() => d.createImageData(100000, 100000), () => d.getImageData(0, 0, 100000, 100000), () => new ImageData(65536, 65536)]) {
    try { make(); out.push("ok"); } catch (e) { out.push(e.name); }
}
const off = new OffscreenCanvas(100000, 100000).getContext("2d");
off.fillRect(0, 0, 1, 1);
out.push(off.getImageData(0, 0, 1, 1).data.length);
return out.join("|");
"##;
    let expected =
        "100000|0 0 0 0|0 0 0 0|10px sans-serif|7|131070|3|RangeError|RangeError|RangeError|4";
    assert_eq!(run(script), expected);
}
