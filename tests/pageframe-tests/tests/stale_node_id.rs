use std::sync::Arc;

use pageframe_dom::{DocumentConfig, LocalName, QualName, ns};
use pageframe_html::{HtmlDocument, HtmlProvider};

fn qname(local: &str) -> QualName {
    QualName {
        prefix: None,
        ns: ns!(html),
        local: LocalName::from(local),
    }
}

#[test]
fn a_dropped_node_id_does_not_resolve_to_the_node_that_took_its_slot() {
    let mut doc = HtmlDocument::from_html(
        "<!doctype html><html><body></body></html>",
        DocumentConfig {
            html_parser_provider: Some(Arc::new(HtmlProvider)),
            ..Default::default()
        },
    );
    let body = doc
        .query_selector("body")
        .expect("selector parses")
        .expect("body exists");

    let mut mutator = doc.mutate();
    let dropped = mutator.create_element(qname("header"), Vec::new());
    mutator.append_children(body, &[dropped]);
    drop(mutator);
    doc.resolve(0.0);

    let mut mutator = doc.mutate();
    mutator.remove_and_drop_node(dropped);
    let replacements: Vec<_> = (0..8)
        .map(|_| mutator.create_element(qname("footer"), Vec::new()))
        .collect();
    mutator.append_children(body, &replacements);
    drop(mutator);
    doc.resolve(0.0);

    assert!(doc.get_node(dropped).is_none());
    for replacement in replacements {
        assert_ne!(replacement, dropped);
        let name = doc
            .get_node(replacement)
            .and_then(|node| node.element_data())
            .map(|element| element.name.local.to_string());
        assert_eq!(name.as_deref(), Some("footer"));
    }
}
