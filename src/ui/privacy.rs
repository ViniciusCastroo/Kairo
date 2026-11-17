use super::*;

impl BrowserWindow {
    pub(super) fn open_private_window(&self) {
        let Some(app) = self
            .window
            .upgrade()
            .and_then(|window| window.application())
        else {
            return;
        };
        let (window, _) = build_window_in_mode(
            &app,
            None,
            None,
            Some(self.preferences.borrow().clone()),
            webkit6::NetworkSession::new_ephemeral(),
        );
        window.present();
    }

    pub(super) fn show_clear_data(self: &Rc<Self>) {
        if self.private || self.clearing_data.get() {
            return;
        }
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Limpar dados de navegação")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .modal(true)
            .default_width(480)
            .build();
        data::dialog_controls(&dialog, "Cancelar");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.append(&gtk4::Label::builder()
            .label("As abas desta janela serão fechadas. Serão apagados todo o histórico, as abas salvas, cookies, cache e armazenamento dos sites. Você sairá das contas dos sites.\n\nFavoritos, preferências e arquivos baixados serão preservados. Janelas privadas não serão alteradas.")
            .wrap(true).xalign(0.0).build());
        let error = gtk4::Label::builder().wrap(true).visible(false).build();
        error.add_css_class("error");
        content.append(&error);
        let clear = gtk4::Button::with_label("Fechar abas e limpar dados");
        clear.add_css_class("destructive-action");
        content.append(&clear);
        let browser = self.clone();
        clear.connect_clicked(glib::clone!(
            #[weak]
            browser,
            #[weak]
            dialog,
            #[weak]
            error,
            move |_| {
                match browser.clear_browsing_data() {
                    Ok(()) => dialog.close(),
                    Err(message) => {
                        error.set_text(&message);
                        error.set_visible(true);
                    }
                }
            }
        ));
        dialog.set_child(Some(&content));
        dialog.present();
    }

    pub(super) fn clear_browsing_data(self: &Rc<Self>) -> Result<(), String> {
        if self.private || self.clearing_data.get() {
            return Err("A limpeza não está disponível nesta janela.".into());
        }
        if self.has_active_downloads() {
            return Err("Conclua ou cancele os downloads antes de limpar os dados.".into());
        }
        let store = self.store.as_ref().ok_or("O perfil está indisponível.")?;
        let manager = self
            .network_session
            .website_data_manager()
            .ok_or("O armazenamento dos sites está indisponível.")?;
        store
            .borrow_mut()
            .clear_browsing_records()
            .map_err(|error| error.to_string())?;
        self.clearing_data.set(true);
        self.storage_status.remove_css_class("error");
        self.storage_status.set_text("Limpando dados de navegação…");
        self.storage_status.set_visible(true);
        if let Some(pending) = self.pending_save.borrow_mut().take() {
            pending.remove();
        }
        self.cancel_all_permissions();
        // Fecha também telas que guardam cópias antigas de histórico ou preferências.
        if let Some(window) = self.window.upgrade() {
            window.set_sensitive(false);
            for dialog in gtk4::Window::list_toplevels()
                .into_iter()
                .filter_map(|widget| widget.downcast::<gtk4::Window>().ok())
            {
                if dialog.transient_for().as_ref() == Some(window.upcast_ref()) {
                    dialog.close();
                }
            }
        }
        *self.tabs.borrow_mut() = Tabs::default();
        let views = std::mem::take(&mut *self.views.borrow_mut());
        for (id, tab) in views {
            tab.webview.stop_loading();
            self.metrics.borrow_mut().forget(id);
            self.tab_strip.remove(&tab.header);
        }
        while self.notebook.n_pages() > 0 {
            self.notebook.remove_page(Some(0));
        }
        self.downloads.borrow_mut().clear();
        // A API do binding exige Send; WebKit entrega o resultado no contexto GTK.
        let browser = glib::thread_guard::ThreadGuard::new(self.clone());
        manager.clear(
            webkit6::WebsiteDataTypes::ALL,
            glib::TimeSpan(0),
            None::<&gio::Cancellable>,
            move |result| {
                let browser = browser.into_inner();
                browser.clearing_data.set(false);
                browser.add_view(browser.new_view(), Some(start_page::URI));
                browser.flush_session();
                if let Some(window) = browser.window.upgrade() {
                    window.set_sensitive(true);
                }
                match result {
                    Ok(()) => {
                        browser.storage_status.remove_css_class("error");
                        browser.storage_status.set_text("Dados de navegação limpos.");
                        browser.storage_status.set_visible(true);
                    }
                    Err(error) => browser.report_storage_error(format!(
                        "Histórico e abas salvas foram apagados, mas os dados dos sites não foram completamente limpos. Tente novamente: {error}"
                    )),
                }
            },
        );
        Ok(())
    }
}
