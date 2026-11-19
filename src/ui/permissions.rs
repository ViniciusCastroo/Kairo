use super::*;
use crate::browser::permissions::secure_page_origin;

pub(super) struct PermissionPrompt {
    request: webkit6::PermissionRequest,
    document: String,
    dialog: glib::WeakRef<gtk4::Window>,
}

impl BrowserWindow {
    pub(super) fn connect_permissions(self: &Rc<Self>, id: TabId, view: &webkit6::WebView) {
        let browser = self.clone();
        view.connect_permission_request(glib::clone!(
            #[weak]
            browser,
            #[upgrade_or]
            false,
            move |view, request| {
                browser.request_permission(id, view, request);
                true
            }
        ));
        view.connect_query_permission_state(|_, query| {
            let supported = query.name().is_some_and(|name| {
                matches!(
                    name.as_str(),
                    "geolocation"
                        | "camera"
                        | "microphone"
                        | "notifications"
                        | "clipboard-read"
                        | "clipboard-write"
                )
            });
            query.finish(if supported {
                webkit6::PermissionState::Prompt
            } else {
                webkit6::PermissionState::Denied
            });
            true
        });
        view.connect_load_failed_with_tls_errors(glib::clone!(
            #[weak]
            browser,
            #[upgrade_or]
            false,
            move |view, uri, certificate, errors| {
                browser.block_tls(id, view, uri, certificate, errors);
                true
            }
        ));
        if let Some(manager) = view.user_content_manager() {
            manager.register_script_message_handler("kairoTls", None);
            manager.connect_script_message_received(
                Some("kairoTls"),
                glib::clone!(
                    #[weak]
                    browser,
                    #[weak]
                    view,
                    move |_, value| browser.answer_tls_warning(id, &view, &value.to_str()),
                ),
            );
        }
        view.connect_show_notification(glib::clone!(
            #[weak]
            browser,
            #[upgrade_or]
            false,
            move |_, notification| {
                let Some(app) = browser
                    .window
                    .upgrade()
                    .and_then(|window| window.application())
                else {
                    return false;
                };
                let message =
                    gio::Notification::new(notification.title().as_deref().unwrap_or("Kairo"));
                message.set_body(notification.body().as_deref());
                message.set_icon(&gio::ThemedIcon::new(kairo::branding::ICON_NAME));
                let key = format!("kairo-notification-{}", notification.id());
                app.send_notification(Some(&key), &message);
                notification.connect_closed(glib::clone!(
                    #[weak]
                    app,
                    move |notification| app.withdraw_notification(&format!(
                        "kairo-notification-{}",
                        notification.id()
                    ))
                ));
                true
            }
        ));
    }

    /// Recusa a página e mostra o aviso. Também chamado quando uma página com erro
    /// de certificado chega a carregar sem o sinal do WebKit e sem exceção do usuário.
    pub(super) fn block_tls(
        self: &Rc<Self>,
        id: TabId,
        view: &webkit6::WebView,
        uri: &str,
        certificate: &gio::TlsCertificate,
        errors: gio::TlsCertificateFlags,
    ) {
        let host = tls_host(uri);
        if let Some(tab) = self.views.borrow().get(&id) {
            tab.tls_failed.set(true);
            tab.history_pending.set(false);
            tab.is_start_page.set(false);
            tab.tls_block.replace(host.clone().map(|host| TlsBlock {
                uri: uri.to_owned(),
                host,
                certificate: certificate.clone(),
            }));
        }
        self.metrics.borrow_mut().fail(id, false);
        let Some(host) = host else {
            self.set_error(
                id,
                Some("O certificado deste site não é válido. A conexão foi bloqueada.".into()),
            );
            return;
        };
        // O WebKit ainda emite o fim da carga recusada; o aviso entra depois dele.
        let html = kairo::browser::tls_error::page(&host, errors);
        let uri = uri.to_owned();
        let view = view.downgrade();
        let browser = self.clone();
        glib::idle_add_local_once(glib::clone!(
            #[weak]
            browser,
            move || {
                let Some(view) = view.upgrade() else { return };
                let pending = browser.views.borrow().get(&id).is_some_and(|tab| {
                    let pending = tab
                        .tls_block
                        .borrow()
                        .as_ref()
                        .is_some_and(|block| block.uri == uri);
                    tab.interstitial_loading.set(pending);
                    tab.interstitial_committed.set(false);
                    pending
                });
                if pending {
                    view.load_alternate_html(&html, &uri, None);
                }
            }
        ));
    }

    pub(super) fn answer_tls_warning(&self, id: TabId, view: &webkit6::WebView, answer: &str) {
        let block = {
            let views = self.views.borrow();
            let Some(tab) = views.get(&id) else { return };
            let shown = !tab.interstitial_loading.get()
                && tab
                    .tls_block
                    .borrow()
                    .as_ref()
                    .is_some_and(|block| view.uri().as_deref() == Some(block.uri.as_str()));
            if !shown || self.closing.get() {
                return;
            }
            tab.tls_block.take()
        };
        let Some(block) = block else { return };
        match answer {
            // Exceção só para esta sessão de rede; some ao fechar o Kairo.
            "proceed" => {
                self.network_session
                    .allow_tls_certificate_for_host(&block.certificate, &block.host);
                self.tls_exceptions.borrow_mut().insert(block.host.clone());
                view.load_uri(&block.uri);
            }
            "back" if view.can_go_back() => view.go_back(),
            "back" => view.load_uri(start_page::URI),
            _ => {
                self.views
                    .borrow()
                    .get(&id)
                    .map(|tab| tab.tls_block.replace(Some(block)));
            }
        }
    }

    fn request_permission(
        self: &Rc<Self>,
        id: TabId,
        view: &webkit6::WebView,
        request: &webkit6::PermissionRequest,
    ) {
        let capability =
            if let Some(media) = request.downcast_ref::<webkit6::UserMediaPermissionRequest>() {
                match (media.is_for_audio_device(), media.is_for_video_device()) {
                    (true, true) => Some("câmera e microfone"),
                    (true, false) => Some("microfone"),
                    (false, true) => Some("câmera"),
                    _ => None,
                }
            } else if request.is::<webkit6::GeolocationPermissionRequest>() {
                Some("localização")
            } else if request.is::<webkit6::NotificationPermissionRequest>() {
                Some("notificações")
            } else if request.is::<webkit6::ClipboardPermissionRequest>() {
                Some("área de transferência")
            } else {
                None
            };
        let document = view.uri().map(|uri| uri.to_string()).unwrap_or_default();
        let eligible = self.tabs.borrow().active() == Some(id)
            && !self.closing.get()
            && !self.permission_prompts.borrow().contains_key(&id);
        let Some((capability, origin)) = capability
            .zip(secure_page_origin(&document))
            .filter(|_| eligible)
        else {
            request.deny();
            return;
        };
        let Some(parent) = self.window.upgrade().filter(|window| window.is_visible()) else {
            request.deny();
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Permissão do site")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .modal(true)
            .default_width(470)
            .build();
        data::dialog_controls(&dialog, "Negar");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 16);
        content.set_margin_top(20);
        content.set_margin_bottom(20);
        content.set_margin_start(20);
        content.set_margin_end(20);
        content.append(
            &gtk4::Label::builder()
                .label(&origin)
                .xalign(0.0)
                .selectable(true)
                .wrap(true)
                .build(),
        );
        content.append(&gtk4::Label::builder().label(format!("O conteúdo desta página, incluindo conteúdo incorporado, solicita acesso a {capability}.\n\nEsta escolha não será salva no perfil.")).wrap(true).xalign(0.0).build());
        let allow = gtk4::Button::with_label("Permitir agora");
        allow.add_css_class("suggested-action");
        let browser = self.clone();
        allow.connect_clicked(glib::clone!(
            #[weak]
            browser,
            move |_| browser.finish_permission(id, true)
        ));
        content.append(&allow);
        let deny = gtk4::Button::with_label("Não permitir");
        deny.connect_clicked(glib::clone!(
            #[weak]
            browser,
            move |_| browser.cancel_permission(id)
        ));
        content.append(&deny);
        dialog.set_child(Some(&content));
        self.permission_prompts.borrow_mut().insert(
            id,
            PermissionPrompt {
                request: request.clone(),
                document,
                dialog: dialog.downgrade(),
            },
        );
        dialog.connect_close_request(glib::clone!(
            #[weak]
            browser,
            #[upgrade_or]
            glib::Propagation::Proceed,
            move |_| {
                if let Some(prompt) = browser.permission_prompts.borrow_mut().remove(&id) {
                    prompt.request.deny();
                }
                glib::Propagation::Proceed
            }
        ));
        dialog.present();
    }

    fn finish_permission(&self, id: TabId, allow: bool) {
        let pending = self.permission_prompts.borrow_mut().remove(&id);
        let Some(pending) = pending else {
            return;
        };
        let same_document = self
            .views
            .borrow()
            .get(&id)
            .is_some_and(|tab| tab.webview.uri().as_deref() == Some(pending.document.as_str()));
        let selected = self.tabs.borrow().active() == Some(id);
        if allow && same_document && selected && !self.closing.get() {
            pending.request.allow();
        } else {
            pending.request.deny();
        }
        if let Some(dialog) = pending.dialog.upgrade() {
            dialog.close();
        }
    }

    pub(super) fn cancel_permission(&self, id: TabId) {
        self.finish_permission(id, false);
    }

    pub(super) fn cancel_all_permissions(&self) {
        let ids: Vec<_> = self.permission_prompts.borrow().keys().copied().collect();
        for id in ids {
            self.cancel_permission(id);
        }
    }
}

pub(super) fn tls_host(uri: &str) -> Option<String> {
    url::Url::parse(uri).ok().and_then(|url| match url.host()? {
        url::Host::Ipv6(address) => Some(address.to_string()),
        host => Some(host.to_string()),
    })
}
