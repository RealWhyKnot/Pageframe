use pageframe::{App, Event, EventKind, Key, NamedKey, NodeId, Ui};

pub(crate) const MAX_TITLE: usize = 200;

pub(crate) struct Todo {
    pub(crate) id: u32,
    pub(crate) done: bool,
}

#[derive(Default)]
pub(crate) struct Todos {
    next_id: u32,
    pub(crate) items: Vec<Todo>,
}

pub(crate) fn app() -> App<Todos> {
    App::new(Todos::default(), include_str!("todo.html"))
        .stylesheet(include_str!("todo.css"))
        .title("To-do")
        .size(420, 560)
        .on("#add", EventKind::Click, |todos, ui, _| add(todos, ui))
        .on("#new", EventKind::KeyDown, |todos, ui, event| {
            if event.key() == Some(&Key::Named(NamedKey::Enter)) {
                add(todos, ui);
            }
        })
        .on(".toggle", EventKind::Click, toggle)
        .on(".remove", EventKind::Click, remove)
}

fn add(todos: &mut Todos, ui: &mut Ui<'_>) {
    let typed = ui.select("#new").value().unwrap_or_default();
    let title: String = typed.trim().chars().take(MAX_TITLE).collect();
    ui.select("#new").set_value("");
    if title.is_empty() {
        return;
    }
    let Some(row) = ui
        .instantiate("#row-template")
        .into_iter()
        .find(|&node| ui.node(node).has_class("row"))
    else {
        return;
    };
    let id = todos.next_id;
    todos.next_id = id.wrapping_add(1);
    todos.items.push(Todo { id, done: false });
    ui.node(row).set_attr("data-id", &id.to_string());
    if let Some(label) = ui.find_in(row, ".title") {
        ui.node(label).set_text(&title);
    }
    ui.select("#list").append(&[row]);
    refresh(todos, ui);
}

fn find_row(todos: &Todos, ui: &mut Ui<'_>, event: &Event<'_>) -> Option<(NodeId, usize)> {
    let row = ui.closest(event.target(), ".row")?;
    let id: u32 = ui.node(row).attr("data-id")?.parse().ok()?;
    let index = todos.items.iter().position(|todo| todo.id == id)?;
    Some((row, index))
}

fn toggle(todos: &mut Todos, ui: &mut Ui<'_>, event: &mut Event<'_>) {
    let Some((row, index)) = find_row(todos, ui, event) else {
        return;
    };
    let Some(todo) = todos.items.get_mut(index) else {
        return;
    };
    todo.done = !todo.done;
    let done = if todo.done { "true" } else { "false" };
    ui.node(row).set_attr("data-done", done);
    ui.node(event.current_target())
        .set_attr("aria-pressed", done);
    refresh(todos, ui);
}

fn remove(todos: &mut Todos, ui: &mut Ui<'_>, event: &mut Event<'_>) {
    let Some((row, index)) = find_row(todos, ui, event) else {
        return;
    };
    todos.items.remove(index);
    ui.node(row).remove();
    refresh(todos, ui);
}

fn refresh(todos: &Todos, ui: &mut Ui<'_>) {
    let left = todos.items.iter().filter(|todo| !todo.done).count();
    let label = if left == 1 {
        "1 item left".to_owned()
    } else {
        format!("{left} items left")
    };
    ui.select("#left").set_text(&label);
    if todos.items.is_empty() {
        ui.select("#empty").remove_attr("hidden");
    } else {
        ui.select("#empty").set_attr("hidden", "");
    }
}
