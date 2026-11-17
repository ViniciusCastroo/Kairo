use gtk4::prelude::*;

// Identificador provisório para desenvolvimento local.
const APP_ID: &str = "dev.example.Kairo";

pub fn run() -> gtk4::glib::ExitCode {
    let app = gtk4::Application::builder().application_id(APP_ID).build();

    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }

        crate::ui::present_window(app);
    });

    app.run()
}
