mod app;
use kairo::{browser, performance, storage};
mod ui;

fn main() -> gtk4::glib::ExitCode {
    performance::start_clock();
    app::run()
}
