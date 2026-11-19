use pageframe::{App, Page};
use pageframe_dom::{LocalName, QualName, local_name, ns};
use pageframe_test_harness::Harness;
use pageframe_traits::node_id::NodeId;
use pageframe_traits::shell::Viewport;
use style::computed_values::direction::T as Direction;

const STATIC: &str = "\
<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}body{width:300px}div,p{min-height:24px;margin:0}</style>\n\
<div id=\"a0\" dir=\"auto\"><i id=\"a1\">&#1488;</i><i id=\"a2\">b</i></div>\n\
<div id=\"b0\" dir=\"AUTO\"><i id=\"b1\">1</i> <i id=\"b2\">&#1488;</i>b</div>\n\
<div dir=\"rtl\"><div id=\"c0\" dir=\"auto\"><i id=\"c1\">b</i><i id=\"c2\">&#1488;</i></div></div>\n\
<div dir=\"rtl\"><div id=\"d0\" dir=\"auto\"><i id=\"d1\">1</i><i id=\"d2\">2</i></div></div>\n\
<div id=\"e0\" dir=\"auto\"><span dir=\"ltr\"><i id=\"e1\">b</i></span><i id=\"e2\">&#1488;</i><i id=\"e3\">c</i></div>\n\
<div id=\"f0\" dir=\"auto\"><bdi><i id=\"f1\">b</i></bdi><i id=\"f2\">&#1488;</i><i id=\"f3\">c</i></div>\n\
<div id=\"g0\" dir=\"auto\"><style></style><script>/*abc*/</script><i id=\"g1\">&#1488;</i><i id=\"g2\">b</i></div>\n\
<div id=\"h0\">x<bdi id=\"h3\"><i id=\"h1\">&#1488;</i><i id=\"h2\">b</i></bdi>y</div>\n\
<div id=\"j0\" dir=\"auto\"><textarea id=\"j1\" style=\"width:40px;height:16px\">abc</textarea><i id=\"j2\">&#1488;</i><i id=\"j3\">b</i></div>\n\
<div id=\"k0\" dir=\"auto\" style=\"direction:ltr\"><i id=\"k1\">&#1488;</i><i id=\"k2\">b</i></div>\n\
<div id=\"m0\" dir=\"auto\"><img id=\"m1\" alt=\"&#1488;\" style=\"width:10px;height:10px\"><i id=\"m2\">b</i><i id=\"m3\">c</i></div>\n\
<div id=\"n0\">ab<span id=\"n3\" dir=\"auto\"><i id=\"n1\">&#1488;</i><i id=\"n2\">c</i></span>d</div>\n\
<div id=\"o0\" dir=\"auto\"><i id=\"o1\">&#8207;</i><i id=\"o2\">b</i><i id=\"o3\">c</i></div>\n\
<div id=\"q0\" dir=\"auto\"><input id=\"q1\" value=\"&#1488;\" style=\"width:40px\"><i id=\"q2\">b</i></div>\n\
<pre id=\"r0\" dir=\"auto\" style=\"margin:0;font:inherit\"><i id=\"r1\">&#1488;</i><i id=\"r2\">b</i></pre>";

const STATIC_1X: [(&str, [f64; 4]); 52] = [
    ("a0", [0.0, 0.0, 19200.0, 1536.0]),
    ("a1", [18695.0, 0.0, 505.0, 1024.0]),
    ("a2", [18196.0, 0.0, 499.0, 1024.0]),
    ("b0", [0.0, 1536.0, 19200.0, 1536.0]),
    ("b1", [18701.0, 1536.0, 499.0, 1024.0]),
    ("b2", [17947.0, 1536.0, 505.0, 1024.0]),
    ("c0", [0.0, 3072.0, 19200.0, 1536.0]),
    ("c1", [0.0, 3072.0, 499.0, 1024.0]),
    ("c2", [499.0, 3072.0, 505.0, 1024.0]),
    ("d0", [0.0, 4608.0, 19200.0, 1536.0]),
    ("d1", [0.0, 4608.0, 499.0, 1024.0]),
    ("d2", [499.0, 4608.0, 499.0, 1024.0]),
    ("e0", [0.0, 6144.0, 19200.0, 1536.0]),
    ("e1", [18701.0, 6144.0, 499.0, 1024.0]),
    ("e2", [18196.0, 6144.0, 505.0, 1024.0]),
    ("e3", [17748.0, 6144.0, 448.0, 1024.0]),
    ("f0", [0.0, 7680.0, 19200.0, 1536.0]),
    ("f1", [18701.0, 7680.0, 499.0, 1024.0]),
    ("f2", [18196.0, 7680.0, 505.0, 1024.0]),
    ("f3", [17748.0, 7680.0, 448.0, 1024.0]),
    ("g0", [0.0, 9216.0, 19200.0, 1536.0]),
    ("g1", [18695.0, 9216.0, 505.0, 1024.0]),
    ("g2", [18196.0, 9216.0, 499.0, 1024.0]),
    ("h0", [0.0, 10752.0, 19200.0, 1536.0]),
    ("h3", [448.0, 10752.0, 1004.0, 1024.0]),
    ("h1", [947.0, 10752.0, 505.0, 1024.0]),
    ("h2", [448.0, 10752.0, 499.0, 1024.0]),
    ("j0", [0.0, 12288.0, 19200.0, 1600.0]),
    ("j1", [16256.0, 12288.0, 2944.0, 1408.0]),
    ("j2", [15751.0, 12864.0, 505.0, 1024.0]),
    ("j3", [15252.0, 12864.0, 499.0, 1024.0]),
    ("k0", [0.0, 13888.0, 19200.0, 1536.0]),
    ("k1", [0.0, 13888.0, 505.0, 1024.0]),
    ("k2", [505.0, 13888.0, 499.0, 1024.0]),
    ("m0", [0.0, 15424.0, 19200.0, 1536.0]),
    ("m1", [0.0, 15424.0, 1529.0, 1024.0]),
    ("m2", [1529.0, 15424.0, 499.0, 1024.0]),
    ("m3", [2028.0, 15424.0, 448.0, 1024.0]),
    ("n0", [0.0, 16960.0, 19200.0, 1536.0]),
    ("n3", [997.0, 16960.0, 953.0, 1024.0]),
    ("n1", [1445.0, 16960.0, 505.0, 1024.0]),
    ("n2", [997.0, 16960.0, 448.0, 1024.0]),
    ("o0", [0.0, 18496.0, 19200.0, 1536.0]),
    ("o1", [19200.0, 18496.0, 0.0, 1024.0]),
    ("o2", [18253.0, 18496.0, 499.0, 1024.0]),
    ("o3", [18752.0, 18496.0, 448.0, 1024.0]),
    ("q0", [0.0, 20032.0, 19200.0, 1536.0]),
    ("q1", [0.0, 20032.0, 3072.0, 1344.0]),
    ("q2", [3072.0, 20160.0, 499.0, 1024.0]),
    ("r0", [0.0, 21568.0, 19200.0, 1024.0]),
    ("r1", [18695.0, 21568.0, 505.0, 1024.0]),
    ("r2", [18196.0, 21568.0, 499.0, 1024.0]),
];

const STATIC_2X: [(&str, [f64; 4]); 52] = [
    ("a0", [0.0, 0.0, 19200.0, 1536.0]),
    ("a1", [18695.5, 0.0, 504.5, 992.0]),
    ("a2", [18197.0, 0.0, 498.5, 992.0]),
    ("b0", [0.0, 1536.0, 19200.0, 1536.0]),
    ("b1", [18701.5, 1536.0, 498.5, 992.0]),
    ("b2", [17948.0, 1536.0, 504.5, 992.0]),
    ("c0", [0.0, 3072.0, 19200.0, 1536.0]),
    ("c1", [0.0, 3072.0, 498.5, 992.0]),
    ("c2", [498.5, 3072.0, 504.5, 992.0]),
    ("d0", [0.0, 4608.0, 19200.0, 1536.0]),
    ("d1", [0.0, 4608.0, 498.5, 992.0]),
    ("d2", [498.5, 4608.0, 498.5, 992.0]),
    ("e0", [0.0, 6144.0, 19200.0, 1536.0]),
    ("e1", [18701.5, 6144.0, 498.5, 992.0]),
    ("e2", [18197.0, 6144.0, 504.5, 992.0]),
    ("e3", [17749.0, 6144.0, 448.0, 992.0]),
    ("f0", [0.0, 7680.0, 19200.0, 1536.0]),
    ("f1", [18701.5, 7680.0, 498.5, 992.0]),
    ("f2", [18197.0, 7680.0, 504.5, 992.0]),
    ("f3", [17749.0, 7680.0, 448.0, 992.0]),
    ("g0", [0.0, 9216.0, 19200.0, 1536.0]),
    ("g1", [18695.5, 9216.0, 504.5, 992.0]),
    ("g2", [18197.0, 9216.0, 498.5, 992.0]),
    ("h0", [0.0, 10752.0, 19200.0, 1536.0]),
    ("h3", [448.0, 10752.0, 1003.0, 992.0]),
    ("h1", [946.5, 10752.0, 504.5, 992.0]),
    ("h2", [448.0, 10752.0, 498.5, 992.0]),
    ("j0", [0.0, 12288.0, 19200.0, 1632.0]),
    ("j1", [16256.0, 12288.0, 2944.0, 1408.0]),
    ("j2", [15751.5, 12896.0, 504.5, 992.0]),
    ("j3", [15253.0, 12896.0, 498.5, 992.0]),
    ("k0", [0.0, 13920.0, 19200.0, 1536.0]),
    ("k1", [0.0, 13920.0, 504.5, 992.0]),
    ("k2", [504.5, 13920.0, 498.5, 992.0]),
    ("m0", [0.0, 15456.0, 19200.0, 1536.0]),
    ("m1", [0.0, 15456.0, 1528.5, 1024.0]),
    ("m2", [1528.5, 15456.0, 498.5, 992.0]),
    ("m3", [2027.0, 15456.0, 448.0, 992.0]),
    ("n0", [0.0, 16992.0, 19200.0, 1536.0]),
    ("n3", [997.0, 16992.0, 952.5, 992.0]),
    ("n1", [1445.0, 16992.0, 504.5, 992.0]),
    ("n2", [997.0, 16992.0, 448.0, 992.0]),
    ("o0", [0.0, 18528.0, 19200.0, 1536.0]),
    ("o1", [19200.0, 18528.0, 0.0, 992.0]),
    ("o2", [18253.5, 18528.0, 498.5, 992.0]),
    ("o3", [18752.0, 18528.0, 448.0, 992.0]),
    ("q0", [0.0, 20064.0, 19200.0, 1536.0]),
    ("q1", [0.0, 20064.0, 3072.0, 1376.0]),
    ("q2", [3072.0, 20224.0, 498.5, 992.0]),
    ("r0", [0.0, 21600.0, 19200.0, 1024.0]),
    ("r1", [18695.5, 21600.0, 504.5, 992.0]),
    ("r2", [18197.0, 21600.0, 498.5, 992.0]),
];

const DYNAMIC: &str = "\
<!doctype html><style>html,body{margin:0;background:#fff;font:14px Arial}body{width:300px}div{min-height:24px}</style>\n\
<div id=\"t0\" dir=\"auto\"><i id=\"t1\">b</i><i id=\"t2\">c</i></div>\n\
<div id=\"u0\" dir=\"auto\"><i id=\"u1\">1</i><i id=\"u2\">c</i></div>\n\
<div id=\"v0\" dir=\"auto\"><span id=\"v3\" dir=\"ltr\"><i id=\"v1\">b</i></span><i id=\"v2\">&#1488;</i></div>\n\
<div id=\"w0\" dir=\"auto\"><i id=\"w1\">b</i><i id=\"w2\">&#1488;</i></div>\n\
<div id=\"x0\" dir=\"auto\"><i id=\"x1\">&#1488;</i><i id=\"x2\">c</i></div>";

const DYNAMIC_1X: [(&str, [f64; 4]); 16] = [
    ("t0", [0.0, 0.0, 19200.0, 1536.0]),
    ("t1", [18695.0, 0.0, 505.0, 1024.0]),
    ("t2", [18247.0, 0.0, 448.0, 1024.0]),
    ("u0", [0.0, 1536.0, 19200.0, 1536.0]),
    ("u3", [18695.0, 1536.0, 505.0, 1024.0]),
    ("u1", [17748.0, 1536.0, 499.0, 1024.0]),
    ("u2", [18247.0, 1536.0, 448.0, 1024.0]),
    ("v0", [0.0, 3072.0, 19200.0, 1536.0]),
    ("v3", [0.0, 3072.0, 499.0, 1024.0]),
    ("v1", [0.0, 3072.0, 499.0, 1024.0]),
    ("v2", [499.0, 3072.0, 505.0, 1024.0]),
    ("w0", [0.0, 4608.0, 19200.0, 1536.0]),
    ("w2", [18695.0, 4608.0, 505.0, 1024.0]),
    ("x0", [0.0, 6144.0, 19200.0, 1536.0]),
    ("x1", [0.0, 6144.0, 505.0, 1024.0]),
    ("x2", [505.0, 6144.0, 448.0, 1024.0]),
];

const DYNAMIC_2X: [(&str, [f64; 4]); 16] = [
    ("t0", [0.0, 0.0, 19200.0, 1536.0]),
    ("t1", [18695.5, 0.0, 504.5, 992.0]),
    ("t2", [18247.5, 0.0, 448.0, 992.0]),
    ("u0", [0.0, 1536.0, 19200.0, 1536.0]),
    ("u3", [18695.5, 1536.0, 504.5, 992.0]),
    ("u1", [17749.0, 1536.0, 498.5, 992.0]),
    ("u2", [18247.5, 1536.0, 448.0, 992.0]),
    ("v0", [0.0, 3072.0, 19200.0, 1536.0]),
    ("v3", [0.0, 3072.0, 498.5, 992.0]),
    ("v1", [0.0, 3072.0, 498.5, 992.0]),
    ("v2", [498.5, 3072.0, 504.5, 992.0]),
    ("w0", [0.0, 4608.0, 19200.0, 1536.0]),
    ("w2", [18695.5, 4608.0, 504.5, 992.0]),
    ("x0", [0.0, 6144.0, 19200.0, 1536.0]),
    ("x1", [0.0, 6144.0, 504.5, 992.0]),
    ("x2", [504.5, 6144.0, 448.0, 992.0]),
];

fn open_at(page: &'static str, scale: u16) -> Harness<Page<()>> {
    let page = App::new((), page)
        .page(320, 480)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    {
        let mut doc = harness.base_mut();
        let color_scheme = doc.get_viewport().color_scheme;
        let size = |css: u32| css * u32::from(scale);
        doc.set_viewport(Viewport::new(
            size(320),
            size(480),
            f32::from(scale),
            color_scheme,
        ));
    }
    harness.pump();
    harness
}

fn node_id(harness: &Harness<Page<()>>, id: &str) -> NodeId {
    harness
        .base()
        .query_selector(&format!("#{id}"))
        .expect("valid selector")
        .expect("every probed id exists")
}

fn compare(harness: &Harness<Page<()>>, scale: u16, chrome: &[(&'static str, [f64; 4])]) {
    let doc = harness.base();
    let found: Vec<(&str, [f64; 4])> = chrome
        .iter()
        .flat_map(|(id, _)| {
            let node = node_id(harness, id);
            doc.node_client_rects(node).into_iter().map(move |rect| {
                let values = [rect.x, rect.y, rect.width, rect.height];
                (*id, values.map(|value| value * 64.0))
            })
        })
        .collect();
    let ids: Vec<&str> = found.iter().map(|(id, _)| *id).collect();
    let expected: Vec<&str> = chrome.iter().map(|(id, _)| *id).collect();
    assert_eq!(ids, expected, "one rect per id at {scale}x");
    let wrong: Vec<String> = found
        .iter()
        .zip(chrome)
        .filter(|((_, have), (_, want))| {
            have.iter()
                .zip(want)
                .any(|(have, want)| (have - want).abs() >= 0.01)
        })
        .map(|((id, have), (_, want))| format!("{scale}x #{id}: {have:?}, Chrome {want:?}"))
        .collect();
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

fn html_name(name: LocalName) -> QualName {
    QualName::new(None, ns!(html), name)
}

fn attr_name(name: LocalName) -> QualName {
    QualName::new(None, ns!(), name)
}

fn mutate_dynamic_page(harness: &mut Harness<Page<()>>) {
    let t1 = node_id(harness, "t1");
    let u1 = node_id(harness, "u1");
    let v3 = node_id(harness, "v3");
    let w1 = node_id(harness, "w1");
    let x0 = node_id(harness, "x0");
    let t1_text = harness.base().get_node(t1).expect("t1 exists").children[0];
    let mut doc = harness.base_mut();
    let mut mutator = doc.mutate();
    mutator.set_node_text(t1_text, "\u{5d0}");
    let u3 = mutator.create_element(html_name(local_name!("i")), Vec::new());
    mutator.set_attribute(u3, attr_name(local_name!("id")), "u3");
    let u3_text = mutator.create_text_node("\u{5d0}");
    mutator.append_children(u3, &[u3_text]);
    mutator.insert_nodes_before(u1, &[u3]);
    mutator.clear_attribute(v3, attr_name(local_name!("dir")));
    mutator.remove_node(w1);
    mutator.set_attribute(x0, attr_name(local_name!("dir")), "ltr");
    drop(mutator);
    drop(doc);
    harness.pump();
}

fn direction(harness: &Harness<Page<()>>, id: &str) -> Direction {
    harness
        .base()
        .get_node(node_id(harness, id))
        .expect("the node exists")
        .primary_styles()
        .expect("the node is styled")
        .get_inherited_box()
        .direction
}

#[test]
fn dir_auto_takes_the_first_strong_character_like_chrome() {
    compare(&open_at(STATIC, 1), 1, &STATIC_1X);
}

#[test]
fn dir_auto_takes_the_first_strong_character_at_2x_like_chrome() {
    compare(&open_at(STATIC, 2), 2, &STATIC_2X);
}

#[test]
fn dir_auto_follows_text_child_and_dir_changes_like_chrome() {
    for (scale, chrome) in [(1, &DYNAMIC_1X), (2, &DYNAMIC_2X)] {
        let mut harness = open_at(DYNAMIC, scale);
        mutate_dynamic_page(&mut harness);
        compare(&harness, scale, chrome);
    }
}

#[test]
fn dir_auto_input_follows_what_is_typed() {
    let mut harness = open_at(
        "<!doctype html><input id=q dir=auto><input id=r><input id=s dir=auto value=b>",
        1,
    );
    assert_eq!(direction(&harness, "q"), Direction::Ltr);
    harness.click("#q");
    harness.type_text("\u{5d0}b");
    assert_eq!(direction(&harness, "q"), Direction::Rtl);
    harness.click("#r");
    harness.type_text("\u{5d0}");
    assert_eq!(direction(&harness, "r"), Direction::Ltr);
    assert_eq!(direction(&harness, "s"), Direction::Ltr);
}
