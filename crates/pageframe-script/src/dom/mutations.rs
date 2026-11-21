use boa_engine::job::{Job, PromiseJob};
use boa_engine::object::builtins::JsArray;
use boa_engine::object::{JsObject, ObjectInitializer};
use boa_engine::property::Attribute;
use boa_engine::{Context, JsResult, JsString, JsValue, js_string};
use pageframe_dom::node::NodeData;
use pageframe_dom::{BaseDocument, NodeId};

use super::{dom_ctx, js_str, node_id_of_value, node_or_null, node_wrapper};
use crate::state::DomCtx;

enum Kind {
    ChildList,
    Attributes,
    CharacterData,
}

struct Record {
    kind: Kind,
    target: NodeId,
    path: Vec<NodeId>,
    name: Option<String>,
    old_value: Option<String>,
    added: Vec<NodeId>,
    removed: Vec<NodeId>,
    previous: Option<NodeId>,
    next: Option<NodeId>,
}

impl Record {
    fn new(kind: Kind, target: NodeId, path: Vec<NodeId>) -> Self {
        Self {
            kind,
            target,
            path,
            name: None,
            old_value: None,
            added: Vec::new(),
            removed: Vec::new(),
            previous: None,
            next: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct MutationQueue {
    observing: bool,
    scheduled: bool,
    depth: usize,
    records: Vec<Record>,
    children: Vec<(NodeId, Vec<NodeId>)>,
    texts: Vec<(NodeId, String)>,
}

fn path_of(doc: &BaseDocument, node_id: NodeId) -> Vec<NodeId> {
    let mut path = Vec::new();
    let mut current = Some(node_id);
    while let Some(id) = current {
        path.push(id);
        current = doc.get_node(id).and_then(|node| node.parent);
    }
    path
}

fn text_of(doc: &BaseDocument, node_id: NodeId) -> Option<String> {
    match &doc.get_node(node_id)?.data {
        NodeData::Text(text) => Some(text.content.clone()),
        NodeData::Comment { contents } => Some(contents.clone()),
        _ => None,
    }
}

fn children_of(doc: &BaseDocument, node_id: NodeId) -> Option<Vec<NodeId>> {
    doc.get_node(node_id).map(|node| node.children.to_vec())
}

pub(crate) fn is_observing(ctx: &DomCtx) -> bool {
    ctx.state.borrow().mutations.observing
}

fn schedule(ctx: &DomCtx, context: &mut Context) {
    {
        let mut state = ctx.state.borrow_mut();
        let queue = &mut state.mutations;
        if queue.scheduled || queue.records.is_empty() {
            return;
        }
        queue.scheduled = true;
    }
    context.enqueue_job(Job::PromiseJob(PromiseJob::new(|context| {
        dom_ctx(context)?.state.borrow_mut().mutations.scheduled = false;
        let deliver = context
            .global_object()
            .get(js_string!("__pageframe_mutation_deliver"), context)?;
        match deliver.as_object().filter(|deliver| deliver.is_callable()) {
            Some(deliver) => deliver.call(&JsValue::undefined(), &[], context),
            None => Ok(JsValue::undefined()),
        }
    })));
}

pub(crate) fn record_attribute(
    context: &mut Context,
    ctx: &DomCtx,
    node_id: NodeId,
    name: &str,
    old_value: Option<String>,
) {
    if !is_observing(ctx) {
        return;
    }
    let path = path_of(&ctx.doc.borrow(), node_id);
    let mut record = Record::new(Kind::Attributes, node_id, path);
    record.name = Some(name.to_string());
    record.old_value = old_value;
    ctx.state.borrow_mut().mutations.records.push(record);
    schedule(ctx, context);
}

pub(crate) fn set_observing(
    _: &JsValue,
    args: &[JsValue],
    context: &mut Context,
) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let observing = args.first().is_some_and(JsValue::to_boolean);
    let mut state = ctx.state.borrow_mut();
    state.mutations.observing = observing;
    if !observing {
        state.mutations.depth = 0;
        state.mutations.children.clear();
        state.mutations.texts.clear();
    }
    Ok(JsValue::undefined())
}

pub(crate) fn begin(_: &JsValue, args: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let undefined = JsValue::undefined();
    let (this, args) = args.split_first().unwrap_or((&undefined, &[]));
    let ctx = dom_ctx(context)?;
    if !is_observing(&ctx) {
        return Ok(JsValue::undefined());
    }
    let doc = ctx.doc.borrow();
    let mut state = ctx.state.borrow_mut();
    let queue = &mut state.mutations;
    queue.depth += 1;
    let nodes: Vec<NodeId> = args
        .iter()
        .filter_map(node_id_of_value)
        .chain(node_id_of_value(this))
        .collect();
    let parent = |id: NodeId| doc.get_node(id).and_then(|node| node.parent);
    let mut parents: Vec<NodeId> = nodes[..nodes.len().saturating_sub(1)]
        .iter()
        .filter_map(|&id| parent(id))
        .collect();
    parents.extend(nodes.iter().copied());
    parents.extend(node_id_of_value(this).and_then(parent));
    for id in parents {
        if queue.children.iter().any(|(known, _)| *known == id) {
            continue;
        }
        if let Some(children) = children_of(&doc, id) {
            queue.children.push((id, children));
        }
    }
    for id in nodes {
        if queue.texts.iter().any(|(known, _)| *known == id) {
            continue;
        }
        if let Some(text) = text_of(&doc, id) {
            queue.texts.push((id, text));
        }
    }
    Ok(JsValue::undefined())
}

fn single_move(before: &[NodeId], after: &[NodeId]) -> Option<NodeId> {
    if before.len() != after.len() {
        return None;
    }
    before.iter().copied().find(|moved| {
        before
            .iter()
            .filter(|id| *id != moved)
            .eq(after.iter().filter(|id| *id != moved))
    })
}

fn neighbours(list: &[NodeId], first: usize, len: usize) -> (Option<NodeId>, Option<NodeId>) {
    let previous = first.checked_sub(1).map(|index| list[index]);
    (previous, list.get(first + len).copied())
}

fn child_records(doc: &BaseDocument, parent: NodeId, before: &[NodeId]) -> Vec<Record> {
    let Some(after) = children_of(doc, parent) else {
        return Vec::new();
    };
    if before == after.as_slice() {
        return Vec::new();
    }
    let path = path_of(doc, parent);
    if let Some(moved) = single_move(before, &after) {
        let mut removal = Record::new(Kind::ChildList, parent, path.clone());
        let from = before.iter().position(|id| *id == moved).unwrap_or(0);
        (removal.previous, removal.next) = neighbours(before, from, 1);
        removal.removed.push(moved);
        let mut addition = Record::new(Kind::ChildList, parent, path);
        let to = after.iter().position(|id| *id == moved).unwrap_or(0);
        (addition.previous, addition.next) = neighbours(&after, to, 1);
        addition.added.push(moved);
        return vec![removal, addition];
    }
    let prefix = before
        .iter()
        .zip(&after)
        .take_while(|(a, b)| a == b)
        .count();
    let suffix = before[prefix..]
        .iter()
        .rev()
        .zip(after[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut record = Record::new(Kind::ChildList, parent, path);
    record.removed = before[prefix..before.len() - suffix].to_vec();
    record.added = after[prefix..after.len() - suffix].to_vec();
    let (previous, next) = neighbours(&after, prefix, record.added.len());
    record.previous = previous;
    record.next = next;
    vec![record]
}

pub(crate) fn end(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let (children, texts) = {
        let mut state = ctx.state.borrow_mut();
        let queue = &mut state.mutations;
        if queue.depth == 0 {
            return Ok(JsValue::undefined());
        }
        queue.depth -= 1;
        if queue.depth > 0 {
            return Ok(JsValue::undefined());
        }
        (
            std::mem::take(&mut queue.children),
            std::mem::take(&mut queue.texts),
        )
    };
    let mut records = Vec::new();
    {
        let doc = ctx.doc.borrow();
        for (node_id, old) in texts {
            if text_of(&doc, node_id).is_some_and(|text| text != old) {
                let mut record = Record::new(Kind::CharacterData, node_id, path_of(&doc, node_id));
                record.old_value = Some(old);
                records.push(record);
            }
        }
        for (parent, before) in children {
            records.extend(child_records(&doc, parent, &before));
        }
    }
    ctx.state.borrow_mut().mutations.records.extend(records);
    schedule(&ctx, context);
    Ok(JsValue::undefined())
}

fn nodes_array(ctx: &DomCtx, ids: &[NodeId], context: &mut Context) -> JsValue {
    let values: Vec<JsValue> = ids
        .iter()
        .map(|&id| node_wrapper(ctx, id, context).into())
        .collect();
    JsArray::from_iter(values, context).into()
}

fn optional_str(value: Option<&str>) -> JsValue {
    value.map_or_else(JsValue::null, js_str)
}

fn record_object(ctx: &DomCtx, record: &Record, context: &mut Context) -> JsObject {
    let kind = match record.kind {
        Kind::ChildList => "childList",
        Kind::Attributes => "attributes",
        Kind::CharacterData => "characterData",
    };
    let target = node_wrapper(ctx, record.target, context);
    let path = nodes_array(ctx, &record.path, context);
    let added = nodes_array(ctx, &record.added, context);
    let removed = nodes_array(ctx, &record.removed, context);
    let previous = node_or_null(ctx, record.previous, context);
    let next = node_or_null(ctx, record.next, context);
    let fields: [(&str, JsValue); 9] = [
        ("type", js_str(kind)),
        ("target", target.into()),
        ("path", path),
        ("attributeName", optional_str(record.name.as_deref())),
        ("oldValue", optional_str(record.old_value.as_deref())),
        ("addedNodes", added),
        ("removedNodes", removed),
        ("previousSibling", previous),
        ("nextSibling", next),
    ];
    let mut object = ObjectInitializer::new(context);
    for (key, value) in fields {
        object.property(JsString::from(key), value, Attribute::all());
    }
    object.build()
}

pub(crate) fn take(_: &JsValue, _: &[JsValue], context: &mut Context) -> JsResult<JsValue> {
    let ctx = dom_ctx(context)?;
    let records = std::mem::take(&mut ctx.state.borrow_mut().mutations.records);
    let values: Vec<JsValue> = records
        .iter()
        .map(|record| record_object(&ctx, record, context).into())
        .collect();
    Ok(JsArray::from_iter(values, context).into())
}
