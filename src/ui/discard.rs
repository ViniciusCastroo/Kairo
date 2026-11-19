//! Abas que não ocupam processo de página: restauradas sob demanda e descartadas
//! depois de muito tempo inativas. O histórico da aba fica no processo da interface.
use super::*;
use std::time::{Duration, Instant};

const DISCARD_AFTER: Duration = Duration::from_secs(15 * 60);

/// Campos alterados e não enviados, ou documento em modo de edição.
const EDITED_FORM: &str = "[...document.querySelectorAll('input, textarea, select')].some(e => \
    e.type === 'checkbox' || e.type === 'radio' ? e.checked !== e.defaultChecked \
    : e.tagName === 'SELECT' ? [...e.options].some(o => o.selected !== o.defaultSelected) \
    : e.value !== e.defaultValue) || document.designMode === 'on'";

impl BrowserWindow {
    pub(super) fn add_pending_view(self: &Rc<Self>, uri: &str) {
        let Some(id) = self.add_view(self.new_view(), None) else {
            return;
        };
        if let Some(tab) = self.views.borrow().get(&id) {
            tab.pending_uri.replace(Some(uri.to_owned()));
        }
        self.refresh(id, true);
    }

    pub(super) fn wake(&self, id: TabId) {
        let views = self.views.borrow();
        let Some(tab) = views.get(&id) else { return };
        if let Some(uri) = tab.pending_uri.take() {
            tab.webview.load_uri(&uri);
        } else if tab.discarded.replace(false) {
            // Volta à mesma entrada do histórico, sem criar uma nova.
            match tab
                .webview
                .back_forward_list()
                .and_then(|list| list.current_item())
            {
                Some(item) => tab.webview.go_to_back_forward_list_item(&item),
                None => tab.webview.reload(),
            }
        }
    }

    pub(super) fn left_tab(&self, id: TabId) {
        if let Some(tab) = self.views.borrow().get(&id) {
            tab.last_active.set(Instant::now());
        }
    }

    pub(super) fn schedule_discard(self: &Rc<Self>) {
        let browser = Rc::downgrade(self);
        glib::timeout_add_seconds_local(60, move || {
            let Some(browser) = browser.upgrade() else {
                return glib::ControlFlow::Break;
            };
            browser.discard_inactive(DISCARD_AFTER);
            glib::ControlFlow::Continue
        });
    }

    /// Encerra o processo das abas inativas há `idle` sem áudio, captura, carga,
    /// pedido de permissão, aviso de certificado ou formulário editado.
    pub(super) fn discard_inactive(self: &Rc<Self>, idle: Duration) {
        if self.clearing_data.get() || self.closing.get() {
            return;
        }
        let active = self.tabs.borrow().active();
        let prompts = self.permission_prompts.borrow();
        let candidates: Vec<_> = self
            .views
            .borrow()
            .iter()
            .filter(|(id, tab)| {
                let view = &tab.webview;
                Some(**id) != active
                    && !prompts.contains_key(id)
                    && tab.last_active.get().elapsed() >= idle
                    && !tab.discarded.get()
                    && tab.pending_uri.borrow().is_none()
                    && tab.tls_block.borrow().is_none()
                    && !view.is_loading()
                    && !view.is_playing_audio()
                    && view.camera_capture_state() == webkit6::MediaCaptureState::None
                    && view.microphone_capture_state() == webkit6::MediaCaptureState::None
                    && view.display_capture_state() == webkit6::MediaCaptureState::None
                    && view
                        .uri()
                        .is_some_and(|uri| !uri.is_empty() && uri != "about:blank")
            })
            .map(|(id, tab)| (*id, tab.webview.clone()))
            .collect();
        drop(prompts);
        for (id, view) in candidates {
            let browser = Rc::downgrade(self);
            view.evaluate_javascript(
                EDITED_FORM,
                Some("kairo-discard"),
                None,
                None::<&gio::Cancellable>,
                move |edited| {
                    // Em caso de erro, a aba fica como está.
                    if !matches!(edited, Ok(ref value) if value.is_boolean() && !value.to_boolean())
                    {
                        return;
                    }
                    let Some(browser) = browser.upgrade() else {
                        return;
                    };
                    if browser.tabs.borrow().active() == Some(id) {
                        return;
                    }
                    if let Some(tab) = browser.views.borrow().get(&id)
                        && !tab.webview.is_loading()
                        && !tab.discarded.replace(true)
                    {
                        tab.webview.terminate_web_process();
                    }
                    browser.refresh(id, true);
                },
            );
        }
    }

    /// Avisa quando o processo de uma página termina sem ter sido descartado pelo Kairo.
    pub(super) fn connect_crash(self: &Rc<Self>, id: TabId, view: &webkit6::WebView) {
        let browser = Rc::downgrade(self);
        view.connect_web_process_terminated(move |_, reason| {
            let Some(browser) = browser.upgrade() else {
                return;
            };
            if reason == webkit6::WebProcessTerminationReason::TerminatedByApi {
                return;
            }
            let message = if reason == webkit6::WebProcessTerminationReason::ExceededMemoryLimit {
                "A página usou memória demais e foi encerrada. Recarregue para abrir de novo."
            } else {
                "A página parou de funcionar. Recarregue para abrir de novo."
            };
            browser.set_error(id, Some(message.into()));
        });
    }
}
