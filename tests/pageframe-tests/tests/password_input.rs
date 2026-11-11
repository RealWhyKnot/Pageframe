use keyboard_types::{Key, Modifiers, NamedKey};
use pageframe_dom::node::TextInputData;
use pageframe_dom::{QualName, local_name, ns};
use pageframe_test_harness::Harness;
use pageframe_traits::events::{PageframeImeEvent, UiEvent};

const BULLET: char = '\u{2022}';

#[cfg(target_os = "macos")]
const ACTION_MOD: Modifiers = Modifiers::META;
#[cfg(not(target_os = "macos"))]
const ACTION_MOD: Modifiers = Modifiers::CONTROL;

fn with_input<R>(harness: &Harness, f: impl FnOnce(&TextInputData) -> R) -> R {
    let doc = harness.base();
    let input = doc
        .get_node(harness.node("input"))
        .unwrap()
        .element_data()
        .unwrap()
        .text_input_data()
        .unwrap();
    f(input)
}

fn shown(harness: &Harness) -> String {
    with_input(harness, |input| input.editor.raw_text().to_string())
}

fn value(harness: &Harness) -> String {
    with_input(harness, |input| input.value().to_string())
}

fn bullets(count: usize) -> String {
    std::iter::repeat_n(BULLET, count).collect()
}

fn type_attr() -> QualName {
    QualName::new(None, ns!(), local_name!("type"))
}

#[test]
fn password_input_shows_bullets_for_its_value() {
    let harness =
        Harness::from_html(r#"<html><body><input type="password" value="hunter2"></body></html>"#);
    assert_eq!(shown(&harness), bullets(7));
    assert_eq!(value(&harness), "hunter2");
}

#[test]
fn typing_and_deleting_edit_the_real_value() {
    let mut harness =
        Harness::from_html(r#"<html><body><input type="password" autofocus></body></html>"#);
    harness.type_text("päss");
    assert_eq!(shown(&harness), bullets(4));
    assert_eq!(value(&harness), "päss");

    harness.press(Key::Named(NamedKey::ArrowLeft));
    harness.press(Key::Named(NamedKey::Backspace));
    assert_eq!(value(&harness), "päs");
    harness.press(Key::Named(NamedKey::Home));
    harness.press(Key::Named(NamedKey::Delete));
    assert_eq!(value(&harness), "äs");
    harness.type_text(&BULLET.to_string());
    assert_eq!(value(&harness), format!("{BULLET}äs"));
    assert_eq!(shown(&harness), bullets(3));
}

#[test]
fn replacing_a_selection_and_pasting_edit_the_real_value() {
    let mut harness = Harness::from_html(
        r#"<html><body><input type="password" value="secret" autofocus></body></html>"#,
    );
    harness.press_with(Key::Character("a".into()), ACTION_MOD);
    harness.type_text("x");
    assert_eq!(value(&harness), "x");
    harness.dispatch(UiEvent::Paste("yz".into()));
    harness.pump();
    assert_eq!(value(&harness), "xyz");
    assert_eq!(shown(&harness), bullets(3));
}

#[test]
fn word_and_selection_deletes_edit_the_real_value() {
    let mut harness = Harness::from_html(
        r#"<html><body><input type="PASSWORD" value="one 😀two" autofocus></body></html>"#,
    );
    assert_eq!(shown(&harness), bullets(8));
    harness.press(Key::Named(NamedKey::End));
    harness.press_with(Key::Named(NamedKey::Backspace), ACTION_MOD);
    assert_eq!(
        shown(&harness).chars().count(),
        value(&harness).chars().count()
    );
    assert_eq!(value(&harness), "");
    assert_ne!(value(&harness), "one 😀two");

    harness.press_with(Key::Character("a".into()), ACTION_MOD);
    harness.type_text("😀b");
    harness.press(Key::Named(NamedKey::ArrowLeft));
    harness.press_with(Key::Named(NamedKey::ArrowLeft), Modifiers::SHIFT);
    harness.press(Key::Named(NamedKey::Backspace));
    assert_eq!(value(&harness), "b");
    assert_eq!(shown(&harness), bullets(1));

    harness.press(Key::Named(NamedKey::End));
    harness.type_text("c d");
    harness.press_with(Key::Named(NamedKey::ArrowLeft), ACTION_MOD);
    harness.type_text("a");
    assert_eq!(value(&harness), "abc d");
    harness.press_with(Key::Named(NamedKey::Delete), ACTION_MOD);
    assert_eq!(value(&harness), "a");
}

#[test]
fn word_selection_extends_an_existing_selection() {
    let mut harness = Harness::from_html(
        r#"<html><body><input type="password" value="abc def" autofocus></body></html>"#,
    );
    harness.press(Key::Named(NamedKey::End));
    harness.press_with(Key::Named(NamedKey::ArrowLeft), Modifiers::SHIFT);
    harness.press_with(
        Key::Named(NamedKey::ArrowLeft),
        ACTION_MOD | Modifiers::SHIFT,
    );
    harness.type_text("z");
    assert_eq!(value(&harness), "z");

    harness.type_text("ab");
    harness.press(Key::Named(NamedKey::Home));
    harness.press(Key::Named(NamedKey::ArrowRight));
    harness.press_with(Key::Named(NamedKey::ArrowRight), Modifiers::SHIFT);
    harness.press_with(
        Key::Named(NamedKey::ArrowRight),
        ACTION_MOD | Modifiers::SHIFT,
    );
    harness.type_text("y");
    assert_eq!(value(&harness), "zy");
}

#[test]
fn ime_composition_is_not_shown_in_password_input() {
    let mut harness =
        Harness::from_html(r#"<html><body><input type="password" autofocus></body></html>"#);
    harness.ime(PageframeImeEvent::Preedit("ka".into(), Some((2, 2))));
    assert_eq!(shown(&harness), "");
    harness.ime(PageframeImeEvent::Commit("か".into()));
    assert_eq!(value(&harness), "か");
    assert_eq!(shown(&harness), bullets(1));
}

#[test]
fn changing_the_type_toggles_masking() {
    let mut harness = Harness::from_html(
        r#"<html><body><input type="password" value="open sesame"></body></html>"#,
    );
    let input = harness.node("input");
    harness
        .base_mut()
        .mutate()
        .set_attribute(input, type_attr(), "text");
    harness.pump();
    assert_eq!(shown(&harness), "open sesame");
    assert_eq!(value(&harness), "open sesame");

    harness
        .base_mut()
        .mutate()
        .set_attribute(input, type_attr(), "password");
    harness.pump();
    assert_eq!(shown(&harness), bullets(11));
    assert_eq!(value(&harness), "open sesame");
}
