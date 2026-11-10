use std::collections::HashMap;

use markup5ever::local_name;
use style::Atom;
use style::counter_style::CounterStyle;
use style::properties::ComputedValues;
use style::values::computed::{Content, ContentItem, Quotes};
use style::values::generics::counters::CounterPair;

use super::{counter_style, damage::ALL_DAMAGE};
use crate::{BaseDocument, NodeId};

pub(crate) const BEFORE: usize = 1;
pub(crate) const AFTER: usize = 0;

pub(crate) type GeneratedContent = HashMap<NodeId, [Option<String>; 2]>;

const LIST_ITEM: &str = "list-item";
const DEFAULT_QUOTES: [&str; 4] = ["\u{201c}", "\u{201d}", "\u{2018}", "\u{2019}"];

struct Counter {
    name: Atom,
    value: i32,
    depth: usize,
}

struct Walker<'doc> {
    doc: &'doc BaseDocument,
    counters: Vec<Counter>,
    quote_depth: usize,
    generated: GeneratedContent,
}

impl BaseDocument {
    pub(crate) fn resolve_generated_content(&mut self) {
        let mut walker = Walker {
            doc: self,
            counters: Vec::new(),
            quote_depth: 0,
            generated: GeneratedContent::new(),
        };
        walker.visit(self.root_element().id, 0, None);
        let generated = walker.generated;

        let stale = self
            .generated_content
            .keys()
            .filter(|node_id| !generated.contains_key(*node_id));
        let changed: Vec<NodeId> = generated
            .iter()
            .filter(|(node_id, texts)| self.generated_content.get(*node_id) != Some(*texts))
            .map(|(node_id, _)| node_id)
            .chain(stale)
            .copied()
            .collect();

        self.generated_content = generated;
        for node_id in changed {
            if let Some(node) = self.nodes.get_mut(node_id) {
                node.insert_damage(ALL_DAMAGE);
            }
        }
    }
}

impl<'doc> Walker<'doc> {
    fn visit(&mut self, node_id: NodeId, depth: usize, inherited_lang: Option<&'doc str>) {
        let node = &self.doc.nodes[node_id];
        let Some(element) = node.element_data() else {
            return;
        };
        let Some(styles) = node.primary_styles() else {
            return;
        };
        if styles.get_box().display.is_none() {
            return;
        }
        let lang = element.attr(local_name!("lang")).or(inherited_lang);

        let is_list = matches!(
            element.name.local,
            local_name!("ol") | local_name!("ul") | local_name!("menu")
        );
        if is_list {
            self.reset(&Atom::from(LIST_ITEM), 0, depth);
        }
        self.apply_counters(&styles, depth);
        let counts_itself = styles
            .get_counters()
            .counter_increment
            .iter()
            .any(|pair| &*pair.name.0 == LIST_ITEM);
        if styles.get_box().display.is_list_item() && !counts_itself {
            self.add(&Atom::from(LIST_ITEM), 1, depth);
        }

        let pseudos = node
            .try_stylo_element_data()
            .and_then(|data| data.get())
            .map(|data| {
                let pseudos = data.styles.pseudos.as_array();
                (pseudos[BEFORE].clone(), pseudos[AFTER].clone())
            });
        let (before, after) = pseudos.unwrap_or((None, None));

        if let Some(style) = before {
            self.pseudo(node_id, BEFORE, &style, depth + 1, lang);
        }
        for child_id in &node.children {
            self.visit(*child_id, depth + 1, lang);
        }
        if let Some(style) = after {
            self.pseudo(node_id, AFTER, &style, depth + 1, lang);
        }
        self.counters.retain(|counter| counter.depth <= depth);
    }

    fn pseudo(
        &mut self,
        owner: NodeId,
        slot: usize,
        style: &ComputedValues,
        depth: usize,
        lang: Option<&str>,
    ) {
        if style.get_box().display.is_none() {
            return;
        }
        let Content::Items(item_data) = &style.get_counters().content else {
            return;
        };
        self.apply_counters(style, depth);

        let mut text = String::new();
        let mut is_generated = false;
        for item in &item_data.items[..item_data.alt_start] {
            match item {
                ContentItem::String(string) => text.push_str(string),
                ContentItem::Counter(name, counter_style) => {
                    is_generated = true;
                    let value = self.innermost(&name.0).unwrap_or(0);
                    text.push_str(&counter_text(counter_style, value));
                }
                ContentItem::Counters(name, separator, counter_style) => {
                    is_generated = true;
                    let mut values: Vec<i32> = self
                        .counters
                        .iter()
                        .filter(|counter| counter.name == name.0)
                        .map(|counter| counter.value)
                        .collect();
                    if values.is_empty() {
                        values.push(0);
                    }
                    let parts: Vec<String> = values
                        .into_iter()
                        .map(|value| counter_text(counter_style, value))
                        .collect();
                    text.push_str(&parts.join(separator));
                }
                ContentItem::OpenQuote => {
                    is_generated = true;
                    text.push_str(&quote(style, lang, self.quote_depth, QuoteSide::Open));
                    self.quote_depth += 1;
                }
                ContentItem::CloseQuote => {
                    is_generated = true;
                    if self.quote_depth > 0 {
                        self.quote_depth -= 1;
                        text.push_str(&quote(style, lang, self.quote_depth, QuoteSide::Close));
                    }
                }
                ContentItem::NoOpenQuote => {
                    is_generated = true;
                    self.quote_depth += 1;
                }
                ContentItem::NoCloseQuote => {
                    is_generated = true;
                    self.quote_depth = self.quote_depth.saturating_sub(1);
                }
                ContentItem::Image(_) => {}
            }
        }
        if is_generated {
            self.generated.entry(owner).or_default()[slot] = Some(text);
        }
    }

    fn apply_counters(&mut self, style: &ComputedValues, depth: usize) {
        let counters = style.get_counters();
        for CounterPair { name, value, .. } in counters.counter_reset.iter() {
            self.reset(&name.0, *value, depth);
        }
        for CounterPair { name, value, .. } in counters.counter_increment.iter() {
            self.add(&name.0, *value, depth);
        }
        for CounterPair { name, value, .. } in counters.counter_set.iter() {
            self.innermost_or_new(&name.0, depth).value = *value;
        }
    }

    fn reset(&mut self, name: &Atom, value: i32, depth: usize) {
        let sibling = self
            .counters
            .iter()
            .rposition(|counter| &counter.name == name && counter.depth == depth);
        if let Some(index) = sibling {
            self.counters.remove(index);
        }
        self.counters.push(Counter {
            name: name.clone(),
            value,
            depth,
        });
    }

    fn add(&mut self, name: &Atom, amount: i32, depth: usize) {
        let counter = self.innermost_or_new(name, depth);
        counter.value = counter.value.saturating_add(amount);
    }

    fn innermost(&self, name: &Atom) -> Option<i32> {
        self.counters
            .iter()
            .rev()
            .find(|counter| &counter.name == name)
            .map(|counter| counter.value)
    }

    fn innermost_or_new(&mut self, name: &Atom, depth: usize) -> &mut Counter {
        let index = self
            .counters
            .iter()
            .rposition(|counter| &counter.name == name)
            .unwrap_or_else(|| {
                self.counters.push(Counter {
                    name: name.clone(),
                    value: 0,
                    depth,
                });
                self.counters.len() - 1
            });
        &mut self.counters[index]
    }
}

fn counter_text(counter_style: &CounterStyle, value: i32) -> String {
    match counter_style {
        CounterStyle::None => String::new(),
        CounterStyle::Name(name) => match &*name.0 {
            "disc" => "\u{2022}".to_string(),
            "circle" => "\u{25e6}".to_string(),
            "square" => "\u{25aa}".to_string(),
            name => counter_style::text(name, value),
        },
        CounterStyle::String(_) | CounterStyle::Symbols { .. } => value.to_string(),
    }
}

#[derive(Clone, Copy)]
enum QuoteSide {
    Open,
    Close,
}

fn quote(style: &ComputedValues, lang: Option<&str>, level: usize, side: QuoteSide) -> String {
    match &style.get_list().quotes {
        Quotes::QuoteList(list) => {
            let pairs = &list.0;
            let Some(pair) = pairs.get(level.min(pairs.len().saturating_sub(1))) else {
                return String::new();
            };
            match side {
                QuoteSide::Open => pair.opening.to_string(),
                QuoteSide::Close => pair.closing.to_string(),
            }
        }
        Quotes::Auto => {
            let marks = language_quotes(lang);
            let pair = level.min(1) * 2;
            match side {
                QuoteSide::Open => marks[pair].to_string(),
                QuoteSide::Close => marks[pair + 1].to_string(),
            }
        }
    }
}

fn language_quotes(lang: Option<&str>) -> [&'static str; 4] {
    let primary = lang
        .and_then(|lang| lang.split('-').next())
        .unwrap_or_default()
        .to_ascii_lowercase();
    LANGUAGE_QUOTES
        .iter()
        .find(|(code, _)| *code == primary)
        .map_or(DEFAULT_QUOTES, |(_, marks)| *marks)
}

const LANGUAGE_QUOTES: &[(&str, [&str; 4])] = &[
    ("ar", ["\u{201d}", "\u{201c}", "\u{2019}", "\u{2018}"]),
    ("bg", ["\u{201e}", "\u{201c}", "\u{201e}", "\u{201c}"]),
    ("ca", ["\u{00ab}", "\u{00bb}", "\u{201c}", "\u{201d}"]),
    ("cs", ["\u{201e}", "\u{201c}", "\u{201a}", "\u{2018}"]),
    ("de", ["\u{201e}", "\u{201c}", "\u{201a}", "\u{2018}"]),
    ("el", ["\u{00ab}", "\u{00bb}", "\u{201c}", "\u{201d}"]),
    ("et", ["\u{201e}", "\u{201c}", "\u{201a}", "\u{2018}"]),
    ("fa", ["\u{00ab}", "\u{00bb}", "\u{2039}", "\u{203a}"]),
    ("fi", ["\u{201d}", "\u{201d}", "\u{2019}", "\u{2019}"]),
    ("fr", ["\u{00ab}", "\u{00bb}", "\u{00ab}", "\u{00bb}"]),
    ("he", ["\u{201d}", "\u{201d}", "\u{2019}", "\u{2019}"]),
    ("hu", ["\u{201e}", "\u{201d}", "\u{00bb}", "\u{00ab}"]),
    ("it", ["\u{00ab}", "\u{00bb}", "\u{201c}", "\u{201d}"]),
    ("nb", ["\u{00ab}", "\u{00bb}", "\u{2018}", "\u{2019}"]),
    ("no", ["\u{00ab}", "\u{00bb}", "\u{2018}", "\u{2019}"]),
    ("pl", ["\u{201e}", "\u{201d}", "\u{00ab}", "\u{00bb}"]),
    ("ro", ["\u{201e}", "\u{201d}", "\u{00ab}", "\u{00bb}"]),
    ("ru", ["\u{00ab}", "\u{00bb}", "\u{201e}", "\u{201c}"]),
    ("sk", ["\u{201e}", "\u{201c}", "\u{201a}", "\u{2018}"]),
    ("sl", ["\u{201e}", "\u{201c}", "\u{201a}", "\u{2018}"]),
    ("sr", ["\u{201e}", "\u{201d}", "\u{2019}", "\u{2019}"]),
    ("sv", ["\u{201d}", "\u{201d}", "\u{2019}", "\u{2019}"]),
    ("uk", ["\u{00ab}", "\u{00bb}", "\u{201e}", "\u{201c}"]),
];

#[cfg(test)]
mod tests {
    use super::{DEFAULT_QUOTES, language_quotes};

    #[test]
    fn quotes_follow_the_primary_language() {
        assert_eq!(language_quotes(None), DEFAULT_QUOTES);
        assert_eq!(language_quotes(Some("en-GB")), DEFAULT_QUOTES);
        assert_eq!(language_quotes(Some("de-AT"))[0], "\u{201e}");
        assert_eq!(language_quotes(Some("DE"))[3], "\u{2018}");
    }
}
