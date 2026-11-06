mod app;

fn main() -> Result<(), pageframe::Error> {
    app::app().run()
}
