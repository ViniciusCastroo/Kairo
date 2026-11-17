use super::*;
use crate::browser::downloads::{DownloadState, Transfer, suggested_filename};

pub(super) struct DownloadTask {
    pub(super) transfer: Transfer,
    object: Option<webkit6::Download>,
    file: Option<gio::File>,
    chooser: Option<gio::Cancellable>,
    widgets: Option<DownloadWidgets>,
}

struct DownloadWidgets {
    name: glib::WeakRef<gtk4::Label>,
    status: glib::WeakRef<gtk4::Label>,
    progress: glib::WeakRef<gtk4::ProgressBar>,
    cancel: glib::WeakRef<gtk4::Button>,
    folder: glib::WeakRef<gtk4::Button>,
}

impl BrowserWindow {
    pub(super) fn connect_downloads(self: &Rc<Self>) {
        let browser = self.clone();
        self.network_session.connect_download_started(glib::clone!(
            #[weak]
            browser,
            move |_, download| {
                let owned = download
                    .web_view()
                    .is_some_and(|view| browser.id_for_page(view.upcast_ref()).is_some());
                if owned {
                    if browser.closing.get() {
                        download.cancel();
                    } else {
                        browser.add_download(download);
                    }
                }
            }
        ));
    }

    fn add_download(self: &Rc<Self>, download: &webkit6::Download) -> u64 {
        let id = self.next_download.get();
        self.next_download.set(id + 1);
        download.set_allow_overwrite(false);
        self.downloads.borrow_mut().insert(
            id,
            DownloadTask {
                transfer: Transfer::new(),
                object: Some(download.clone()),
                file: None,
                chooser: None,
                widgets: None,
            },
        );
        let weak = Rc::downgrade(self);
        download.connect_decide_destination(move |download, name| {
            if let Some(browser) = weak.upgrade() {
                browser.choose_download_destination(id, name);
            } else {
                download.cancel();
            }
            // Sempre impede o salvamento automático no destino padrão do motor.
            true
        });
        let browser = self.clone();
        download.connect_received_data(glib::clone!(
            #[weak]
            browser,
            move |download, _| {
                browser.update_download_data(id, download);
            }
        ));
        download.connect_failed(glib::clone!(
            #[weak]
            browser,
            move |_, error| {
                browser.fail_download(
                    id,
                    error.to_string(),
                    error.matches(webkit6::DownloadError::CancelledByUser),
                    false,
                );
            }
        ));
        download.connect_finished(glib::clone!(
            #[weak]
            browser,
            move |download| {
                browser.update_download_data(id, download);
                if let Some(task) = browser.downloads.borrow_mut().get_mut(&id) {
                    task.transfer.finish();
                    task.object.take();
                    task.chooser.take();
                }
                browser.update_download_widgets(id);
            }
        ));
        self.show_downloads();
        id
    }

    fn choose_download_destination(self: &Rc<Self>, id: u64, suggested: &str) {
        let cancellable = gio::Cancellable::new();
        let name = suggested_filename(suggested);
        {
            let mut tasks = self.downloads.borrow_mut();
            let Some(task) = tasks.get_mut(&id).filter(|task| task.transfer.active()) else {
                return;
            };
            task.transfer.name = name.clone();
            task.chooser = Some(cancellable.clone());
        }
        self.update_download_widgets(id);
        #[cfg(test)]
        {
            let destination = self.download_test_destination.borrow_mut().take();
            if let Some(file) = destination {
                if let Err(error) = self.assign_download_destination(id, file, false) {
                    self.fail_download(id, error.to_string(), false, true);
                }
                return;
            }
            if self.download_test_cancel.replace(false) {
                self.cancel_download(id);
                return;
            }
        }
        let parent = self
            .downloads_window
            .upgrade()
            .filter(|window| window.is_visible())
            .or_else(|| self.window.upgrade().map(|window| window.upcast()));
        let Some(parent) = parent else {
            self.cancel_download(id);
            return;
        };
        let chooser = gtk4::FileDialog::builder()
            .title("Salvar download")
            .initial_name(&name)
            .build();
        if let Some(folder) = glib::user_special_dir(glib::UserDirectory::Downloads) {
            chooser.set_initial_folder(Some(&gio::File::for_path(folder)));
        }
        let browser = self.clone();
        chooser.save(
            Some(&parent),
            Some(&cancellable),
            glib::clone!(
                #[weak]
                browser,
                move |result| {
                    match result {
                        Ok(file) => {
                            // O seletor nativo confirma a substituição quando o usuário escolhe um arquivo existente.
                            let replace = file.query_exists(None::<&gio::Cancellable>);
                            if let Err(error) =
                                browser.assign_download_destination(id, file, replace)
                            {
                                browser.fail_download(id, error.to_string(), false, true);
                            }
                        }
                        Err(error) => {
                            let cancelled = error.matches(gio::IOErrorEnum::Cancelled)
                                || error.matches(gtk4::DialogError::Dismissed)
                                || error.matches(gtk4::DialogError::Cancelled);
                            browser.fail_download(id, error.to_string(), cancelled, true);
                        }
                    }
                }
            ),
        );
    }

    fn assign_download_destination(
        &self,
        id: u64,
        file: gio::File,
        replace: bool,
    ) -> std::io::Result<()> {
        let path = file
            .path()
            .ok_or_else(|| std::io::Error::other("Selecione um destino local."))?;
        if !path.is_absolute() {
            return Err(std::io::Error::other(
                "O destino precisa ser um caminho absoluto.",
            ));
        }
        let destination = path
            .to_str()
            .ok_or_else(|| std::io::Error::other("O nome do arquivo precisa ser UTF-8 válido."))?;
        if path.is_dir() {
            return Err(std::io::Error::other(
                "Selecione um arquivo, não uma pasta.",
            ));
        }
        if path.exists() && !replace {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "O arquivo já existe.",
            ));
        }
        let object = {
            let mut tasks = self.downloads.borrow_mut();
            let Some(task) = tasks.get_mut(&id).filter(|task| task.transfer.active()) else {
                return Ok(());
            };
            task.transfer.name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "download".into());
            task.transfer.state = DownloadState::Transferring;
            task.file = Some(file.clone());
            task.chooser.take();
            task.object.clone()
        };
        if let Some(download) = object {
            download.set_allow_overwrite(replace);
            download.set_destination(destination);
        }
        self.update_download_widgets(id);
        Ok(())
    }

    fn update_download_data(&self, id: u64, download: &webkit6::Download) {
        if let Some(task) = self
            .downloads
            .borrow_mut()
            .get_mut(&id)
            .filter(|task| task.transfer.active())
        {
            task.transfer.received = download.received_data_length();
            task.transfer.total = download
                .response()
                .map(|response| {
                    let compressed = response
                        .http_headers()
                        .and_then(|headers| headers.one("Content-Encoding"))
                        .is_some_and(|value| !value.eq_ignore_ascii_case("identity"));
                    if compressed {
                        0
                    } else {
                        response.content_length()
                    }
                })
                .unwrap_or(0);
            let progress = download.estimated_progress();
            task.transfer.progress = if progress.is_finite() {
                progress.clamp(0.0, 1.0)
            } else {
                0.0
            };
        }
        self.update_download_widgets(id);
    }

    fn fail_download(&self, id: u64, message: String, cancelled: bool, stop_backend: bool) {
        let (object, chooser) = {
            let mut tasks = self.downloads.borrow_mut();
            let Some(task) = tasks.get_mut(&id).filter(|task| task.transfer.active()) else {
                return;
            };
            task.transfer.fail(message, cancelled);
            (task.object.take(), task.chooser.take())
        };
        if let Some(chooser) = chooser {
            chooser.cancel();
        }
        if stop_backend && let Some(object) = object {
            object.cancel();
        }
        self.update_download_widgets(id);
    }

    fn cancel_download(&self, id: u64) {
        self.fail_download(id, String::new(), true, true);
    }

    pub(super) fn has_active_downloads(&self) -> bool {
        self.downloads
            .borrow()
            .values()
            .any(|task| task.transfer.active())
    }

    fn update_download_widgets(&self, id: u64) {
        let tasks = self.downloads.borrow();
        let Some(task) = tasks.get(&id) else {
            return;
        };
        let Some(widgets) = &task.widgets else {
            return;
        };
        if let Some(name) = widgets.name.upgrade() {
            name.set_text(&task.transfer.name);
        }
        if let Some(status) = widgets.status.upgrade() {
            status.set_text(&task.transfer.status());
        }
        if let Some(progress) = widgets.progress.upgrade() {
            if task.transfer.state == DownloadState::Transferring && task.transfer.total == 0 {
                progress.pulse();
            } else {
                progress.set_fraction(task.transfer.progress);
            }
        }
        if let Some(cancel) = widgets.cancel.upgrade() {
            cancel.set_sensitive(task.transfer.active());
        }
        if let Some(folder) = widgets.folder.upgrade() {
            folder.set_sensitive(task.transfer.state == DownloadState::Complete);
        }
    }

    pub(super) fn show_downloads(self: &Rc<Self>) {
        if let Some(window) = self
            .downloads_window
            .upgrade()
            .filter(|window| window.is_visible())
        {
            if let Some(list) = self.downloads_list.upgrade() {
                self.populate_downloads(&list);
            }
            window.present();
            return;
        }
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Downloads do Kairo")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .default_width(680)
            .default_height(440)
            .build();
        data::dialog_controls(&dialog, "Voltar à navegação");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        let clear = gtk4::Button::with_label("Limpar concluídos");
        let browser = self.clone();
        clear.connect_clicked(glib::clone!(
            #[weak]
            browser,
            move |_| {
                browser
                    .downloads
                    .borrow_mut()
                    .retain(|_, task| task.transfer.active());
                if let Some(list) = browser.downloads_list.upgrade() {
                    browser.populate_downloads(&list);
                }
            }
        ));
        content.append(&clear);
        let list = gtk4::ListBox::new();
        list.set_selection_mode(gtk4::SelectionMode::None);
        list.set_placeholder(Some(&gtk4::Label::new(Some(
            "Nenhum download nesta sessão.",
        ))));
        self.downloads_window.set(Some(&dialog));
        self.downloads_list.set(Some(&list));
        self.populate_downloads(&list);
        content.append(
            &gtk4::ScrolledWindow::builder()
                .vexpand(true)
                .child(&list)
                .build(),
        );
        dialog.set_child(Some(&content));
        dialog.present();
    }

    fn populate_downloads(self: &Rc<Self>, list: &gtk4::ListBox) {
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        let ids: Vec<_> = self.downloads.borrow().keys().copied().collect();
        for id in ids {
            let row = gtk4::Box::new(gtk4::Orientation::Horizontal, 12);
            row.set_margin_top(10);
            row.set_margin_bottom(10);
            row.set_margin_start(10);
            row.set_margin_end(10);
            let detail = gtk4::Box::new(gtk4::Orientation::Vertical, 6);
            detail.set_hexpand(true);
            let name = gtk4::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk4::pango::EllipsizeMode::End)
                .build();
            let status = gtk4::Label::builder().xalign(0.0).wrap(true).build();
            let progress = gtk4::ProgressBar::new();
            detail.append(&name);
            detail.append(&status);
            detail.append(&progress);
            row.append(&detail);
            let cancel = gtk4::Button::with_label("Cancelar");
            cancel.set_tooltip_text(Some("Cancelar download"));
            let browser = self.clone();
            cancel.connect_clicked(glib::clone!(
                #[weak]
                browser,
                move |_| browser.cancel_download(id)
            ));
            row.append(&cancel);
            let folder = style::icon_button(kairo::icons::FOLDER, "Mostrar pasta", 18);
            folder.set_tooltip_text(Some("Mostrar pasta"));
            folder.connect_clicked(glib::clone!(
                #[weak]
                browser,
                move |_| {
                    let parent = browser
                        .downloads
                        .borrow()
                        .get(&id)
                        .and_then(|task| task.file.as_ref())
                        .and_then(|file| file.parent());
                    if let Some(parent) = parent
                        && let Err(error) = gio::AppInfo::launch_default_for_uri(
                            &parent.uri(),
                            None::<&gio::AppLaunchContext>,
                        )
                    {
                        browser.report_storage_error(error);
                    }
                }
            ));
            row.append(&folder);
            if let Some(task) = self.downloads.borrow_mut().get_mut(&id) {
                task.widgets = Some(DownloadWidgets {
                    name: name.downgrade(),
                    status: status.downgrade(),
                    progress: progress.downgrade(),
                    cancel: cancel.downgrade(),
                    folder: folder.downgrade(),
                });
            }
            list.append(&row);
            self.update_download_widgets(id);
        }
    }

    pub(super) fn confirm_download_exit(self: &Rc<Self>) {
        if let Some(prompt) = self
            .download_exit_prompt
            .upgrade()
            .filter(|window| window.is_visible())
        {
            prompt.present();
            return;
        }
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Downloads em andamento")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .modal(true)
            .default_width(430)
            .build();
        data::dialog_controls(&dialog, "Continuar baixando");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.append(
            &gtk4::Label::builder()
                .label("Fechar o Kairo agora cancelará os downloads em andamento.")
                .wrap(true)
                .build(),
        );
        let exit = gtk4::Button::with_label("Cancelar downloads e sair");
        exit.add_css_class("destructive-action");
        let browser = self.clone();
        exit.connect_clicked(glib::clone!(
            #[weak]
            browser,
            #[weak]
            dialog,
            move |_| {
                let ids: Vec<_> = browser.downloads.borrow().keys().copied().collect();
                for id in ids {
                    browser.cancel_download(id);
                }
                browser.exit_confirmed.set(true);
                dialog.close();
                if let Some(window) = browser.window.upgrade() {
                    window.close();
                }
            }
        ));
        content.append(&exit);
        dialog.set_child(Some(&content));
        self.download_exit_prompt.set(Some(&dialog));
        dialog.present();
    }
}
