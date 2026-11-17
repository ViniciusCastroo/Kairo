use gtk4::prelude::*;

pub(super) fn icon_button(icon: &str, label: &str, size: i32) -> gtk4::Button {
    let button = gtk4::Button::from_icon_name(icon);
    set_button_icon(&button, icon, size);
    button.set_tooltip_text(Some(label));
    button.update_property(&[gtk4::accessible::Property::Label(label)]);
    button
}

pub(super) fn set_button_icon(button: &gtk4::Button, icon: &str, size: i32) {
    if let Some(image) = button.child().and_downcast::<gtk4::Image>() {
        image.set_icon_name(Some(icon));
        image.set_pixel_size(size);
    } else {
        button.set_child(Some(
            &gtk4::Image::builder()
                .icon_name(icon)
                .pixel_size(size)
                .build(),
        ));
    }
}

pub(super) fn install() {
    thread_local! { static INSTALLED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
    if INSTALLED.with(|installed| installed.replace(true)) {
        return;
    }
    kairo::branding::register();
    gtk4::Window::set_default_icon_name(kairo::branding::ICON_NAME);
    if let Some(settings) = gtk4::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(true);
    }
    let provider = gtk4::CssProvider::new();
    provider.load_from_data(include_str!("style.css"));
    if let Some(display) = gtk4::gdk::Display::default() {
        gtk4::IconTheme::for_display(&display).add_resource_path("/dev/example/Kairo/icons");
        gtk4::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}
