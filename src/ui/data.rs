use super::*;

impl BrowserWindow {
    pub(super) fn report_storage_error(&self, error: impl std::fmt::Display) {
        self.storage_status.add_css_class("error");
        self.storage_status
            .set_text(&format!("Não foi possível salvar ou ler os dados: {error}"));
        self.storage_status.set_visible(true);
    }

    pub(super) fn queue_session(self: &Rc<Self>) {
        if self.private
            || self.clearing_data.get()
            || self.restoring.get()
            || self.closing.get()
            || self.store.is_none()
        {
            return;
        }
        if let Some(pending) = self.pending_save.borrow_mut().take() {
            pending.remove();
        }
        let browser = Rc::downgrade(self);
        let source =
            glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
                if let Some(browser) = browser.upgrade() {
                    browser.pending_save.borrow_mut().take();
                    browser.flush_session();
                }
            });
        *self.pending_save.borrow_mut() = Some(source);
    }

    pub(super) fn flush_session(&self) {
        if let Some(pending) = self.pending_save.borrow_mut().take() {
            pending.remove();
        }
        if self.private
            || self.clearing_data.get()
            || self.restoring.get()
            || self.closing.get()
            || !self.preferences.borrow().restore_session
        {
            return;
        }
        let Some(store) = &self.store else {
            return;
        };
        let session = {
            let tabs = self.tabs.borrow();
            let urls = tabs
                .iter()
                .map(|tab| {
                    if restorable_uri(&tab.uri) {
                        tab.uri.clone()
                    } else {
                        "about:blank".into()
                    }
                })
                .collect();
            let active = tabs
                .iter()
                .position(|tab| Some(tab.id) == tabs.active())
                .unwrap_or(0);
            Session { urls, active }
        };
        if let Err(error) = store.borrow_mut().save_session(&session) {
            self.report_storage_error(error);
        }
    }

    pub(super) fn record_visit(&self, id: TabId) {
        if self.private || self.clearing_data.get() {
            return;
        }
        let Some(store) = &self.store else {
            return;
        };
        let visit = self
            .tabs
            .borrow()
            .get(id)
            .filter(|tab| tab.error.is_none())
            .map(|tab| (tab.uri.clone(), tab.title.clone()));
        if let Some((uri, title)) = visit
            && let Err(error) = store.borrow().record_visit(&uri, &title)
        {
            self.report_storage_error(error);
        }
    }

    pub(super) fn show_preferences(self: &Rc<Self>) {
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Preferências do Kairo")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .modal(true)
            .default_width(460)
            .build();
        dialog_controls(&dialog, "Cancelar");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        for side in [
            gtk4::PositionType::Top,
            gtk4::PositionType::Bottom,
            gtk4::PositionType::Left,
            gtk4::PositionType::Right,
        ] {
            match side {
                gtk4::PositionType::Top => content.set_margin_top(20),
                gtk4::PositionType::Bottom => content.set_margin_bottom(20),
                gtk4::PositionType::Left => content.set_margin_start(20),
                _ => content.set_margin_end(20),
            }
        }
        let preferences = self.preferences.borrow().clone();
        let restore = gtk4::CheckButton::with_label("Restaurar as abas ao abrir");
        restore.set_active(preferences.restore_session);
        content.append(&restore);
        content.append(
            &gtk4::Label::builder()
                .label("Buscador padrão")
                .xalign(0.0)
                .build(),
        );
        let engines = crate::browser::search::SearchEngine::ALL;
        let search = gtk4::DropDown::from_strings(&engines.map(|engine| engine.name()));
        search.set_selected(
            engines
                .iter()
                .position(|engine| *engine == preferences.search_engine)
                .unwrap_or(0) as u32,
        );
        content.append(&search);
        content.append(
            &gtk4::Label::builder()
                .label("Página inicial")
                .xalign(0.0)
                .build(),
        );
        let home = gtk4::Entry::builder()
            .text(&preferences.home_page)
            .hexpand(true)
            .build();
        content.append(&home);
        let use_start = gtk4::Button::with_label("Usar tela inicial do Kairo");
        use_start.connect_clicked(glib::clone!(
            #[weak]
            home,
            move |_| home.set_text(start_page::URI)
        ));
        content.append(&use_start);
        let validation = gtk4::Label::builder()
            .wrap(true)
            .xalign(0.0)
            .visible(false)
            .build();
        validation.add_css_class("error");
        content.append(&validation);
        let save = gtk4::Button::with_label("Salvar");
        save.add_css_class("suggested-action");
        content.append(&save);
        let browser = self.clone();
        save.connect_clicked(glib::clone!(
            #[weak]
            browser,
            #[weak]
            dialog,
            #[weak]
            home,
            #[weak]
            restore,
            #[weak]
            search,
            #[weak]
            validation,
            move |_| {
                let preferences = Preferences {
                    restore_session: restore.is_active(),
                    home_page: home.text().trim().to_owned(),
                    search_engine: crate::browser::search::SearchEngine::ALL
                        .get(search.selected() as usize)
                        .copied()
                        .unwrap_or_default(),
                };
                if !restorable_uri(&preferences.home_page) {
                    validation.set_text(
                        "Use a tela inicial do Kairo ou um endereço HTTP/HTTPS completo.",
                    );
                    validation.set_visible(true);
                    return;
                }
                let Some(store) = &browser.store else {
                    validation.set_text(
                        "O perfil está indisponível. As preferências não podem ser salvas.",
                    );
                    validation.set_visible(true);
                    return;
                };
                let result = store.borrow_mut().save_preferences(&preferences);
                match result {
                    Ok(()) => {
                        *browser.preferences.borrow_mut() = preferences;
                        browser.flush_session();
                        let start_pages: Vec<_> = browser
                            .views
                            .borrow()
                            .values()
                            .filter(|tab| tab.webview.uri().as_deref() == Some(start_page::URI))
                            .map(|tab| tab.webview.clone())
                            .collect();
                        for page in start_pages {
                            browser.sync_start_page(&page);
                        }
                        dialog.close();
                    }
                    Err(error) => {
                        validation.set_text(&format!("Não foi possível salvar: {error}"));
                        validation.set_visible(true);
                    }
                }
            }
        ));
        dialog.set_child(Some(&content));
        dialog.present();
    }

    pub(super) fn show_history(self: &Rc<Self>) {
        let Some(store) = &self.store else {
            self.report_storage_error("O perfil está indisponível.");
            return;
        };
        let entries = match store.borrow().history() {
            Ok(entries) => entries,
            Err(error) => {
                self.report_storage_error(error);
                return;
            }
        };
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Histórico do Kairo")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .default_width(650)
            .default_height(480)
            .build();
        dialog_controls(&dialog, "Voltar à navegação");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 8);
        let clear = gtk4::Button::with_label("Limpar histórico");
        content.append(&clear);
        let list = gtk4::ListBox::new();
        list.set_placeholder(Some(&gtk4::Label::new(Some("Histórico vazio"))));
        for entry in &entries {
            let row = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            row.set_margin_top(8);
            row.set_margin_bottom(8);
            row.set_margin_start(12);
            row.set_margin_end(12);
            row.append(
                &gtk4::Label::builder()
                    .label(&entry.title)
                    .xalign(0.0)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build(),
            );
            let date = glib::DateTime::from_unix_local(entry.visited_at / 1000)
                .and_then(|date| date.format("%d/%m/%Y %H:%M"))
                .map(|date| date.to_string())
                .unwrap_or_default();
            row.append(
                &gtk4::Label::builder()
                    .label(format!("{}\n{date}", entry.uri))
                    .xalign(0.0)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build(),
            );
            list.append(&row);
        }
        let browser = self.clone();
        list.connect_row_activated(glib::clone!(
            #[weak]
            browser,
            #[weak]
            dialog,
            move |_, row| {
                if let Some(entry) = entries
                    .get(row.index() as usize)
                    .filter(|entry| restorable_uri(&entry.uri))
                {
                    browser.add_view(browser.new_view(), Some(&entry.uri));
                    dialog.close();
                }
            }
        ));
        clear.connect_clicked(glib::clone!(
            #[weak]
            browser,
            #[weak]
            list,
            move |_| {
                let Some(store) = &browser.store else {
                    return;
                };
                match store.borrow().clear_history() {
                    Ok(()) => {
                        while let Some(child) = list.first_child() {
                            list.remove(&child);
                        }
                    }
                    Err(error) => browser.report_storage_error(error),
                }
            }
        ));
        let scroll = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .child(&list)
            .build();
        content.append(&scroll);
        dialog.set_child(Some(&content));
        dialog.present();
    }
}

pub(super) fn dialog_controls(dialog: &gtk4::Window, close_label: &str) {
    let header = gtk4::HeaderBar::builder()
        .show_title_buttons(true)
        .decoration_layout(":close")
        .title_widget(&gtk4::Label::new(dialog.title().as_deref()))
        .build();
    let close = gtk4::Button::with_label(close_label);
    close.connect_clicked(glib::clone!(
        #[weak]
        dialog,
        move |_| dialog.close()
    ));
    header.pack_start(&close);
    dialog.set_titlebar(Some(&header));

    let keys = gtk4::EventControllerKey::new();
    keys.set_propagation_phase(gtk4::PropagationPhase::Capture);
    keys.connect_key_pressed(glib::clone!(
        #[weak]
        dialog,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                dialog.close();
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        }
    ));
    dialog.add_controller(keys);
}
