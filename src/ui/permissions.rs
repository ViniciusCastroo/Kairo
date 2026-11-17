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
            move |_, _, _, _| {
                if let Some(tab) = browser.views.borrow().get(&id) {
                    tab.tls_failed.set(true);
                    tab.history_pending.set(false);
                }
                browser.metrics.borrow_mut().fail(id, false);
                browser.set_error(
                    id,
                    Some("O certificado deste site não é válido. A conexão foi bloqueada.".into()),
                );
                true
            }
        ));
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
