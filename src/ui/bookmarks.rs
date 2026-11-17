use super::*;

impl BrowserWindow {
    pub(super) fn bookmark_current_page(self: &Rc<Self>) {
        let current = self.active_view().and_then(|(id, _)| {
            self.tabs
                .borrow()
                .get(id)
                .map(|tab| (tab.title.clone(), tab.uri.clone()))
        });
        let Some((title, uri)) = current.filter(|(_, uri)| {
            url::Url::parse(uri).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
        }) else {
            self.edit_bookmark(None, "", "https://");
            return;
        };
        let existing = self
            .store
            .as_ref()
            .and_then(|store| store.borrow().bookmarks().ok())
            .and_then(|items| items.into_iter().find(|bookmark| bookmark.uri == uri));
        if let Some(bookmark) = existing {
            self.edit_bookmark(Some(bookmark.id), &bookmark.title, &bookmark.uri);
        } else {
            self.edit_bookmark(None, &title, &uri);
        }
    }

    pub(super) fn show_bookmarks(self: &Rc<Self>) {
        if let Some(window) = self
            .bookmarks_window
            .upgrade()
            .filter(|window| window.is_visible())
        {
            window.present();
            return;
        }
        if self.store.is_none() {
            self.report_storage_error("O perfil está indisponível.");
            return;
        }
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Favoritos do Kairo")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .default_width(700)
            .default_height(480)
            .build();
        data::dialog_controls(&dialog, "Voltar à navegação");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        let add = gtk4::Button::with_label("Adicionar favorito");
        add.add_css_class("suggested-action");
        let browser = self.clone();
        add.connect_clicked(glib::clone!(
            #[weak]
            browser,
            move |_| browser.edit_bookmark(None, "", "https://")
        ));
        content.append(&add);
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.set_placeholder(Some(&gtk4::Label::new(Some(
            "Seus sites favoritos aparecerão aqui.",
        ))));
        self.bookmarks_window.set(Some(&dialog));
        self.bookmarks_list.set(Some(&list));
        self.populate_bookmarks(&list);
        content.append(
            &gtk4::ScrolledWindow::builder()
                .vexpand(true)
                .child(&list)
                .build(),
        );
        content.append(
            &gtk4::Label::builder()
                .label("Os 12 primeiros favoritos aparecem na tela inicial.")
                .xalign(0.0)
                .build(),
        );
        dialog.set_child(Some(&content));
        dialog.present();
    }

    fn populate_bookmarks(self: &Rc<Self>, list: &gtk4::ListBox) {
        let Some(store) = &self.store else {
            return;
        };
        let items = match store.borrow().bookmarks() {
            Ok(items) => items,
            Err(error) => {
                self.report_storage_error(error);
                return;
            }
        };
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let count = items.len();
        for (position, bookmark) in items.into_iter().enumerate() {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
            row.set_margin_top(6);
            row.set_margin_bottom(6);
            let labels = gtk4::Box::new(gtk4::Orientation::Vertical, 4);
            labels.append(
                &gtk4::Label::builder()
                    .label(&bookmark.title)
                    .xalign(0.0)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build(),
            );
            labels.append(
                &gtk4::Label::builder()
                    .label(&bookmark.uri)
                    .xalign(0.0)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .build(),
            );
            let open = gtk4::Button::builder().child(&labels).hexpand(true).build();
            open.add_css_class("flat");
            open.set_tooltip_text(Some("Abrir favorito em nova aba"));
            let uri = bookmark.uri.clone();
            let browser = self.clone();
            open.connect_clicked(glib::clone!(
                #[weak]
                browser,
                move |_| {
                    if url::Url::parse(&uri)
                        .is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
                        && restorable_uri(&uri)
                    {
                        browser.add_view(browser.new_view(), Some(&uri));
                        if let Some(window) = browser.bookmarks_window.upgrade() {
                            window.close();
                        }
                    }
                }
            ));
            row.append(&open);
            let edit = style::icon_button(kairo::icons::EDIT, "Editar favorito", 18);
            edit.set_tooltip_text(Some("Editar favorito"));
            let copy = bookmark.clone();
            edit.connect_clicked(glib::clone!(
                #[weak]
                browser,
                move |_| browser.edit_bookmark(Some(copy.id), &copy.title, &copy.uri)
            ));
            row.append(&edit);
            for (direction, icon, label, enabled) in [
                (-1, kairo::icons::UP, "Mover para cima", position > 0),
                (
                    1,
                    kairo::icons::DOWN,
                    "Mover para baixo",
                    position + 1 < count,
                ),
            ] {
                let button = style::icon_button(icon, label, 18);
                button.set_tooltip_text(Some(label));
                button.set_sensitive(enabled);
                let id = bookmark.id;
                button.connect_clicked(glib::clone!(
                    #[weak]
                    browser,
                    move |_| {
                        let result = browser
                            .store
                            .as_ref()
                            .unwrap()
                            .borrow_mut()
                            .move_bookmark(id, direction);
                        match result {
                            Ok(()) => browser.refresh_bookmarks(),
                            Err(error) => browser.report_storage_error(error),
                        }
                    }
                ));
                row.append(&button);
            }
            let remove = style::icon_button(kairo::icons::TRASH, "Remover favorito", 18);
            remove.set_tooltip_text(Some("Remover favorito"));
            let id = bookmark.id;
            remove.connect_clicked(glib::clone!(
                #[weak]
                browser,
                move |_| {
                    let result = browser.store.as_ref().unwrap().borrow().remove_bookmark(id);
                    match result {
                        Ok(()) => browser.refresh_bookmarks(),
                        Err(error) => browser.report_storage_error(error),
                    }
                }
            ));
            row.append(&remove);
            list.append(&row);
        }
    }

    fn edit_bookmark(self: &Rc<Self>, id: Option<i64>, title: &str, uri: &str) {
        let parent = self
            .bookmarks_window
            .upgrade()
            .filter(|window| window.is_visible())
            .or_else(|| self.window.upgrade().map(|window| window.upcast()));
        let Some(parent) = parent else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title(if id.is_some() {
                "Editar favorito"
            } else {
                "Adicionar favorito"
            })
            .transient_for(&parent)
            .destroy_with_parent(true)
            .modal(true)
            .default_width(470)
            .build();
        data::dialog_controls(&dialog, "Cancelar");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 10);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.append(&gtk4::Label::builder().label("Nome").xalign(0.0).build());
        let name = gtk4::Entry::builder().text(title).max_length(200).build();
        name.set_widget_name("bookmark-name");
        content.append(&name);
        content.append(&gtk4::Label::builder().label("Endereço").xalign(0.0).build());
        let address = gtk4::Entry::builder()
            .text(uri)
            .placeholder_text("https://exemplo.com")
            .build();
        address.set_widget_name("bookmark-address");
        content.append(&address);
        let error = gtk4::Label::builder()
            .xalign(0.0)
            .wrap(true)
            .visible(false)
            .build();
        error.add_css_class("error");
        content.append(&error);
        let save = gtk4::Button::with_label("Salvar favorito");
        save.add_css_class("suggested-action");
        let browser = self.clone();
        save.connect_clicked(glib::clone!(
            #[weak]
            browser,
            #[weak]
            dialog,
            #[weak]
            name,
            #[weak]
            address,
            #[weak]
            error,
            move |_| {
                let Some(store) = &browser.store else {
                    error.set_text("O perfil está indisponível.");
                    error.set_visible(true);
                    return;
                };
                let result = store
                    .borrow_mut()
                    .save_bookmark(id, &name.text(), &address.text());
                match result {
                    Ok(_) => {
                        browser.refresh_bookmarks();
                        dialog.close();
                    }
                    Err(problem) => {
                        error.set_text(&problem.to_string());
                        error.set_visible(true);
                    }
                }
            }
        ));
        content.append(&save);
        dialog.set_child(Some(&content));
        dialog.present();
    }

    pub(super) fn refresh_bookmarks(self: &Rc<Self>) {
        if let Some(list) = self
            .bookmarks_list
            .upgrade()
            .filter(|list| list.is_visible())
        {
            self.populate_bookmarks(&list);
        }
        let pages: Vec<_> = self
            .views
            .borrow()
            .values()
            .filter(|tab| tab.is_start_page.get())
            .map(|tab| tab.webview.clone())
            .collect();
        for page in pages {
            self.sync_start_page(&page);
        }
    }
}
