use pageframe::{App, Page};
use pageframe_test_harness::Harness;

const STYLE: &str = "html,body{margin:0;font:14px Arial}.s{width:200px;height:100px;\
border:2px solid;padding:3px;margin-bottom:4px}.t{height:300px}.c{height:10px}";

struct Case {
    id: &'static str,
    client: [f64; 2],
    offset: [f64; 2],
    child: [f64; 4],
}

const STABLE: &[&str] = &[
    "<div id=a class=s style=\"overflow:auto;scrollbar-gutter:stable;scrollbar-width:thin\"><div id=ak class=c></div></div>",
    "<div id=b class=s style=\"overflow:auto;scrollbar-gutter:stable;scrollbar-width:none\"><div id=bk class=c></div></div>",
    "<div id=c class=s style=\"overflow:visible;scrollbar-gutter:stable\"><div id=ck class=c></div></div>",
    "<div id=d class=s style=\"overflow:clip;scrollbar-gutter:stable\"><div id=dk class=c></div></div>",
    "<div id=e class=s style=\"overflow:auto;scrollbar-gutter:stable;direction:rtl\"><div id=ek class=c></div></div>",
    "<div id=f class=s style=\"overflow:auto;scrollbar-gutter:stable;writing-mode:vertical-rl\"><div id=fk style=\"width:10px\"></div></div>",
    "<div id=g class=s style=\"overflow-x:auto;overflow-y:hidden;scrollbar-gutter:stable\"><div id=gk class=c></div></div>",
    "<div id=h class=s style=\"overflow:auto;scrollbar-gutter:stable\"><div id=hk class=t></div></div>",
    "<div id=i class=s style=\"overflow:scroll;scrollbar-gutter:stable\"><div id=ik class=c></div></div>",
    "<div id=j class=s style=\"display:flex;overflow:auto;scrollbar-gutter:stable\"><div id=jk style=\"flex:1;height:10px\"></div></div>",
    "<div id=k class=s style=\"display:grid;overflow:auto;scrollbar-gutter:stable\"><div id=kk style=\"height:10px\"></div></div>",
    "<div id=l class=s style=\"overflow:auto;scrollbar-gutter:stable;width:auto;display:inline-block\"><div id=lk style=\"width:50px;height:10px\"></div></div>",
    "<div id=m class=s style=\"overflow:auto;scrollbar-gutter:stable\">text <span id=mk>text</span> text text text text text text text text text text</div>",
    "<div id=n class=s style=\"overflow:hidden;scrollbar-gutter:stable both-edges;direction:rtl\"><div id=nk class=c></div></div>",
    "<div id=o class=s style=\"overflow:auto;scrollbar-gutter:stable;writing-mode:vertical-rl;direction:rtl\"><div id=ok style=\"width:10px\"></div></div>",
    "<div id=p class=s style=\"overflow:auto;scrollbar-gutter:stable;writing-mode:vertical-lr;direction:rtl\"><div id=pk style=\"width:10px\"></div></div>",
];

const STABLE_CASES: &[Case] = &[
    Case {
        id: "a",
        client: [196.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 5.0, 190.0, 10.0],
    },
    Case {
        id: "b",
        client: [206.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 119.0, 200.0, 10.0],
    },
    Case {
        id: "c",
        client: [206.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 233.0, 200.0, 10.0],
    },
    Case {
        id: "d",
        client: [206.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 347.0, 200.0, 10.0],
    },
    Case {
        id: "e",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [20.0, 461.0, 185.0, 10.0],
    },
    Case {
        id: "f",
        client: [206.0, 91.0],
        offset: [210.0, 110.0],
        child: [195.0, 575.0, 10.0, 85.0],
    },
    Case {
        id: "g",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 689.0, 185.0, 10.0],
    },
    Case {
        id: "h",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 803.0, 185.0, 300.0],
    },
    Case {
        id: "i",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 917.0, 185.0, 10.0],
    },
    Case {
        id: "j",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 1031.0, 185.0, 10.0],
    },
    Case {
        id: "k",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 1145.0, 185.0, 10.0],
    },
    Case {
        id: "l",
        client: [56.0, 106.0],
        offset: [75.0, 110.0],
        child: [5.0, 1259.0, 50.0, 10.0],
    },
    Case {
        id: "m",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [31.46875, 1376.0, 22.578125, 16.0],
    },
    Case {
        id: "n",
        client: [176.0, 106.0],
        offset: [210.0, 110.0],
        child: [20.0, 1490.0, 170.0, 10.0],
    },
    Case {
        id: "o",
        client: [206.0, 91.0],
        offset: [210.0, 110.0],
        child: [195.0, 1604.0, 10.0, 85.0],
    },
    Case {
        id: "p",
        client: [206.0, 91.0],
        offset: [210.0, 110.0],
        child: [5.0, 1718.0, 10.0, 85.0],
    },
];

const BOTH_EDGES: &[&str] = &[
    "<div id=a class=s style=\"overflow:auto\"><div id=ak class=c></div></div>",
    "<div id=b class=s style=\"overflow:scroll\"><div id=bk class=c></div></div>",
    "<div id=c class=s style=\"overflow:hidden;scrollbar-gutter:stable\"><div id=ck class=c></div></div>",
    "<div id=d class=s style=\"overflow:auto;scrollbar-gutter:stable both-edges\"><div id=dk class=c></div></div>",
    "<div id=e class=s style=\"overflow:auto;scrollbar-gutter:stable both-edges;scrollbar-width:thin\"><div id=ek class=c></div></div>",
    "<div id=f class=s style=\"overflow:auto;scrollbar-gutter:stable both-edges;writing-mode:vertical-rl\"><div id=fk style=\"width:10px\"></div></div>",
    "<div id=g class=s style=\"overflow:auto;scrollbar-gutter:stable both-edges;width:auto;display:inline-block\"><div id=gk style=\"width:50px;height:10px\"></div></div>",
    "<div id=h class=s style=\"display:flex;overflow:auto;scrollbar-gutter:stable both-edges\"><div id=hk style=\"flex:1;height:10px\"></div></div>",
    "<div id=i class=s style=\"display:grid;overflow:auto;scrollbar-gutter:stable both-edges\"><div id=ik style=\"height:10px\"></div></div>",
    "<div id=j class=s style=\"overflow:auto;scrollbar-gutter:stable;box-sizing:border-box\"><div id=jk class=c></div></div>",
    "<div id=k class=s style=\"overflow:auto;scrollbar-gutter:stable;width:auto;height:auto;float:left\"><div id=kk style=\"width:50px;height:10px\"></div></div>",
    "<div id=l class=s style=\"overflow:auto;scrollbar-gutter:stable both-edges\">text <span id=lk>text</span> text text text text text text text text text text</div>",
    "<div id=m class=s style=\"overflow:auto;scrollbar-gutter:stable;writing-mode:vertical-lr\"><div id=mk style=\"width:10px\"></div></div>",
    "<div id=n class=s style=\"overflow:auto;scrollbar-gutter:stable;position:absolute;left:300px;top:0;width:auto;height:auto\"><div id=nk style=\"width:50px;height:10px\"></div></div>",
];

const BOTH_EDGES_CASES: &[Case] = &[
    Case {
        id: "a",
        client: [206.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 5.0, 200.0, 10.0],
    },
    Case {
        id: "b",
        client: [206.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 119.0, 200.0, 10.0],
    },
    Case {
        id: "c",
        client: [191.0, 106.0],
        offset: [210.0, 110.0],
        child: [5.0, 233.0, 185.0, 10.0],
    },
    Case {
        id: "d",
        client: [176.0, 106.0],
        offset: [210.0, 110.0],
        child: [20.0, 347.0, 170.0, 10.0],
    },
    Case {
        id: "e",
        client: [186.0, 106.0],
        offset: [210.0, 110.0],
        child: [15.0, 461.0, 180.0, 10.0],
    },
    Case {
        id: "f",
        client: [206.0, 76.0],
        offset: [210.0, 110.0],
        child: [195.0, 590.0, 10.0, 70.0],
    },
    Case {
        id: "g",
        client: [56.0, 106.0],
        offset: [90.0, 110.0],
        child: [20.0, 689.0, 50.0, 10.0],
    },
    Case {
        id: "h",
        client: [176.0, 106.0],
        offset: [210.0, 110.0],
        child: [20.0, 806.0, 170.0, 10.0],
    },
    Case {
        id: "i",
        client: [176.0, 106.0],
        offset: [210.0, 110.0],
        child: [20.0, 920.0, 170.0, 10.0],
    },
    Case {
        id: "j",
        client: [181.0, 96.0],
        offset: [200.0, 100.0],
        child: [5.0, 1034.0, 175.0, 10.0],
    },
    Case {
        id: "k",
        client: [56.0, 16.0],
        offset: [75.0, 20.0],
        child: [5.0, 1138.0, 50.0, 10.0],
    },
    Case {
        id: "l",
        client: [176.0, 106.0],
        offset: [210.0, 110.0],
        child: [121.46875, 1138.0, 22.578125, 16.0],
    },
    Case {
        id: "m",
        client: [206.0, 91.0],
        offset: [210.0, 110.0],
        child: [5.0, 1252.0, 10.0, 85.0],
    },
    Case {
        id: "n",
        client: [56.0, 16.0],
        offset: [75.0, 20.0],
        child: [305.0, 5.0, 50.0, 10.0],
    },
];

fn check(rows: &[&str], cases: &[Case]) {
    let html = format!(
        "<!doctype html><html><head><style>{STYLE}</style></head><body>{}</body></html>",
        rows.join("\n")
    );
    let page: Page<()> = App::new((), &html)
        .page(640, 400)
        .expect("test pages have valid selectors");
    let mut harness = Harness::wrap(page);
    harness.pump();
    let doc = harness.base();
    let find = |id: &str| {
        doc.query_selector_all(&format!("#{id}"))
            .expect("selector is valid")[0]
    };
    for case in cases {
        let id = case.id;
        let scroller = find(id);
        let node = doc.get_node(scroller).expect("scroller exists");
        let client = [
            f64::from(node.client_width()),
            f64::from(node.client_height()),
        ];
        assert_eq!(client, case.client, "{id} clientWidth/clientHeight");
        let offset = doc.offset_rect(scroller).expect("scroller has a box");
        let offset = [offset.width.round(), offset.height.round()];
        assert_eq!(offset, case.offset, "{id} offsetWidth/offsetHeight");
        let child = doc
            .get_client_bounding_rect(find(&format!("{id}k")))
            .expect("child has a box");
        let child = [child.x, child.y, child.width, child.height];
        assert_eq!(child, case.child, "{id} child rect");
    }
    drop(doc);
}

#[test]
fn stable_gutter_reserves_space_on_the_inline_end() {
    check(STABLE, STABLE_CASES);
}

#[test]
fn both_edges_reserves_space_on_both_sides() {
    check(BOTH_EDGES, BOTH_EDGES_CASES);
}
