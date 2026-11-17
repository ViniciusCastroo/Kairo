pub const ICON_NAME: &str = "dev.example.Kairo";

pub fn register() {
    static REGISTERED: std::sync::Once = std::sync::Once::new();
    REGISTERED.call_once(|| {
        gtk4::gio::resources_register_include!("kairo.gresource")
            .expect("Recursos embarcados válidos");
    });
}

pub fn white_logo_data_uri() -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        gtk4::glib::base64_encode(include_bytes!("../assets/branding/kairo-white-compact.svg"))
    )
}
