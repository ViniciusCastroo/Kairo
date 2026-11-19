use gtk4::prelude::*;

// Identificador provisório para desenvolvimento local.
const APP_ID: &str = "dev.example.Kairo";

pub fn run() -> gtk4::glib::ExitCode {
    let app = gtk4::Application::builder()
        .application_id(APP_ID)
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
            return;
        }

        crate::ui::present_window(app);
    });

    app.connect_open(|app, files, _| {
        let window = app
            .active_window()
            .unwrap_or_else(|| crate::ui::present_window(app).upcast());
        window.present();
        for file in files {
            let _ = window.activate_action("win.open-uri", Some(&file.uri().to_variant()));
        }
    });

    app.run()
}
