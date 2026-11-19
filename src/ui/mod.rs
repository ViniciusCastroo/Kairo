use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

use gtk4::{gio, glib, prelude::*};
use webkit6::prelude::*;

use crate::browser::{
    navigation_target, start_page,
    tabs::{TabId, Tabs},
};
use crate::storage::{Preferences, Session, Store, restorable_uri};

mod bookmarks;
mod data;
mod downloads;
mod metrics;
mod permissions;
mod privacy;
mod style;

struct TabView {
    webview: webkit6::WebView,
    label: gtk4::Label,
    history_pending: Cell<bool>,
    header: gtk4::Box,
    is_start_page: Cell<bool>,
    tls_failed: Cell<bool>,
    /// Certificado recusado enquanto a página de aviso está aberta.
    tls_block: RefCell<Option<TlsBlock>>,
    /// Eventos de carregamento da própria página de aviso, ignorados.
    interstitial_loading: Cell<bool>,
    /// O aviso já foi exibido; só então o fim de carga é dele, e não da conexão recusada.
    interstitial_committed: Cell<bool>,
}

struct TlsBlock {
    uri: String,
    host: String,
    certificate: gio::TlsCertificate,
}

struct BrowserWindow {
    window: glib::WeakRef<gtk4::ApplicationWindow>,
    notebook: gtk4::Notebook,
    address: gtk4::Entry,
    security: gtk4::Label,
    status: gtk4::Label,
    storage_status: gtk4::Label,
    spinner: gtk4::Spinner,
    back: gio::SimpleAction,
    forward: gio::SimpleAction,
    stop: gio::SimpleAction,
    tabs: RefCell<Tabs>,
    views: RefCell<HashMap<TabId, TabView>>,
    store: Option<Rc<RefCell<Store>>>,
    preferences: RefCell<Preferences>,
    restoring: Cell<bool>,
    closing: Cell<bool>,
    pending_save: RefCell<Option<glib::SourceId>>,
    metrics: RefCell<crate::performance::NavigationMetrics>,
    tab_strip: gtk4::Box,
    tab_scroll: gtk4::ScrolledWindow,
    reload_button: gtk4::Button,
    web_context: webkit6::WebContext,
    bookmarks_window: glib::WeakRef<gtk4::Window>,
    bookmarks_list: glib::WeakRef<gtk4::ListBox>,
    network_session: webkit6::NetworkSession,
    private: bool,
    clearing_data: Cell<bool>,
    downloads: RefCell<std::collections::BTreeMap<u64, downloads::DownloadTask>>,
    next_download: Cell<u64>,
    downloads_window: glib::WeakRef<gtk4::Window>,
    downloads_list: glib::WeakRef<gtk4::ListBox>,
    download_exit_prompt: glib::WeakRef<gtk4::Window>,
    exit_confirmed: Cell<bool>,
    permission_prompts: RefCell<HashMap<TabId, permissions::PermissionPrompt>>,
    #[cfg(test)]
    download_test_destination: RefCell<Option<gio::File>>,
    #[cfg(test)]
    download_test_cancel: Cell<bool>,
}

pub fn present_window(app: &gtk4::Application) -> gtk4::ApplicationWindow {
    let (store, warning) = match Store::open_default() {
        Ok(store) => (Some(Rc::new(RefCell::new(store))), None),
        Err(error) => (
            None,
            Some(format!(
                "Não foi possível abrir o perfil. Os dados não serão salvos: {error}"
            )),
        ),
    };
    let (window, _) = build_window(app, store, warning);
    window.present();
    window
}

fn build_window(
    app: &gtk4::Application,
    store: Option<Rc<RefCell<Store>>>,
    warning: Option<String>,
) -> (gtk4::ApplicationWindow, Rc<BrowserWindow>) {
    // Ensaios gráficos nunca usam cookies ou cache do perfil pessoal.
    #[cfg(test)]
    let network_session = webkit6::NetworkSession::new_ephemeral();
    #[cfg(not(test))]
    let network_session = webkit6::NetworkSession::default().expect("Sessão de rede do WebKit");
    build_window_in_mode(app, store, warning, None, network_session)
}

fn build_window_in_mode(
    app: &gtk4::Application,
    mut store: Option<Rc<RefCell<Store>>>,
    mut warning: Option<String>,
    private_preferences: Option<Preferences>,
    network_session: webkit6::NetworkSession,
) -> (gtk4::ApplicationWindow, Rc<BrowserWindow>) {
    style::install();
    let private = private_preferences.is_some();
    if private {
        assert!(network_session.is_ephemeral());
        store = None;
    }
    let saved = store.as_ref().map(|store| {
        let store = store.borrow();
        store.preferences().and_then(|preferences| {
            let session = if preferences.restore_session {
                store.session()?
            } else {
                Session::default()
            };
            Ok((preferences, session))
        })
    });
    let (mut preferences, session) = match saved {
        Some(Ok(saved)) => saved,
        Some(Err(error)) => {
            warning = Some(format!(
                "Não foi possível ler o perfil. Os dados não serão alterados: {error}"
            ));
            store = None;
            (Preferences::default(), Session::default())
        }
        None => (Preferences::default(), Session::default()),
    };
    if let Some(selected) = private_preferences {
        preferences = Preferences {
            restore_session: false,
            home_page: start_page::URI.into(),
            ..selected
        };
    }
    let window = gtk4::ApplicationWindow::builder()
        .application(app)
        .title("Kairo")
        .default_width(1100)
        .default_height(740)
        .build();
    window.add_css_class("kairo-window");
    let address = gtk4::Entry::builder()
        .hexpand(true)
        .placeholder_text("Pesquisar ou digitar endereço")
        .build();
    address.add_css_class("kairo-address");
    address.set_icon_from_icon_name(gtk4::EntryIconPosition::Primary, Some(kairo::icons::SEARCH));
    let security = gtk4::Label::builder()
        .label("Não seguro")
        .tooltip_text("A conexão com este site não é segura")
        .visible(false)
        .build();
    security.add_css_class("kairo-insecure");
    let status = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .selectable(true)
        .visible(false)
        .margin_start(12)
        .margin_end(12)
        .build();
    status.add_css_class("error");
    let storage_status = gtk4::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .margin_start(12)
        .margin_end(12)
        .visible(warning.is_some())
        .label(warning.as_deref().unwrap_or(""))
        .build();
    storage_status.add_css_class("error");
    let spinner = gtk4::Spinner::new();
    let toolbar = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
    toolbar.add_css_class("kairo-toolbar");
    for (icon, action, tooltip) in [
        (kairo::icons::BACK, "win.back", "Voltar (Alt+←)"),
        (kairo::icons::FORWARD, "win.forward", "Avançar (Alt+→)"),
    ] {
        let button = style::icon_button(icon, tooltip, 18);
        button.add_css_class("flat");
        button.set_action_name(Some(action));
        button.set_tooltip_text(Some(tooltip));
        toolbar.append(&button);
    }
    let reload_button = style::icon_button(kairo::icons::RELOAD, "Recarregar (Ctrl+R / F5)", 18);
    reload_button.set_action_name(Some("win.reload"));
    reload_button.set_tooltip_text(Some("Recarregar (Ctrl+R / F5)"));
    reload_button.add_css_class("flat");
    toolbar.append(&reload_button);
    toolbar.append(&security);
    toolbar.append(&address);
    let bookmark = style::icon_button(kairo::icons::STAR, "Adicionar favorito (Ctrl+D)", 18);
    bookmark.add_css_class("flat");
    bookmark.set_action_name(Some("win.bookmark-page"));
    bookmark.set_tooltip_text(Some("Adicionar favorito (Ctrl+D)"));
    toolbar.append(&bookmark);
    toolbar.append(&spinner);
    let menu = gio::Menu::new();
    menu.append(Some("Histórico (Ctrl+H)"), Some("win.history"));
    menu.append(Some("Favoritos (Ctrl+Shift+O)"), Some("win.bookmarks"));
    menu.append(Some("Tela inicial do Kairo"), Some("win.start-page"));
    menu.append(Some("Preferências"), Some("win.preferences"));
    menu.append(
        Some("Nova janela privada (Ctrl+Shift+N)"),
        Some("win.private-window"),
    );
    menu.append(Some("Limpar dados de navegação…"), Some("win.clear-data"));
    menu.append(Some("Desempenho"), Some("win.performance"));
    menu.append(Some("Downloads (Ctrl+J)"), Some("win.downloads"));
    toolbar.append(
        &gtk4::MenuButton::builder()
            .child(
                &gtk4::Image::builder()
                    .icon_name(kairo::icons::MENU)
                    .pixel_size(18)
                    .build(),
            )
            .menu_model(&menu)
            .tooltip_text("Menu do Kairo")
            .build(),
    );
    let notebook = gtk4::Notebook::builder()
        .show_tabs(false)
        .show_border(false)
        .vexpand(true)
        .build();
    notebook.add_css_class("kairo-pages");
    let tab_strip = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    tab_strip.add_css_class("kairo-tabs");
    let tab_scroll = gtk4::ScrolledWindow::builder()
        .hexpand(false)
        .hscrollbar_policy(gtk4::PolicyType::External)
        .vscrollbar_policy(gtk4::PolicyType::Never)
        .propagate_natural_width(true)
        .min_content_width(180)
        .max_content_width(1600)
        .child(&tab_strip)
        .build();
    let new_tab = style::icon_button(kairo::icons::PLUS, "Nova aba (Ctrl+T)", 16);
    new_tab.add_css_class("flat");
    new_tab.add_css_class("kairo-new-tab");
    new_tab.set_action_name(Some("win.new-tab"));
    new_tab.set_tooltip_text(Some("Nova aba (Ctrl+T)"));
    let titlebar = gtk4::Box::new(gtk4::Orientation::Horizontal, 4);
    titlebar.add_css_class("kairo-titlebar");
    titlebar.append(&tab_scroll);
    titlebar.append(&new_tab);
    if private {
        let badge = gtk4::Label::new(Some("Privado"));
        badge.add_css_class("private-badge");
        titlebar.append(&badge);
    }
    titlebar.append(&gtk4::Box::builder().hexpand(true).build());
    let controls = gtk4::WindowControls::new(gtk4::PackType::End);
    controls.set_decoration_layout(Some(":minimize,maximize,close"));
    titlebar.append(&controls);
    let handle = gtk4::WindowHandle::new();
    handle.set_child(Some(&titlebar));
    window.set_titlebar(Some(&handle));
    let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
    content.append(&toolbar);
    if private {
        let notice = gtk4::Label::builder()
            .label("Janela privada · sem histórico ou sessão salva · arquivos baixados permanecem no computador")
            .wrap(true)
            .margin_top(6)
            .margin_bottom(6)
            .build();
        content.append(&notice);
    }
    content.append(&status);
    content.append(&storage_status);
    content.append(&notebook);
    window.set_child(Some(&content));
    if let Some(font_map) = address.pango_context().font_map()
        && let Err(error) = kairo::fonts::register_native(&font_map)
    {
        glib::g_warning!("Kairo", "{error} Usando a fonte alternativa do sistema.");
    }

    let browser = Rc::new(BrowserWindow {
        window: window.downgrade(),
        notebook,
        address,
        security,
        status,
        storage_status,
        spinner,
        back: gio::SimpleAction::new("back", None),
        forward: gio::SimpleAction::new("forward", None),
        stop: gio::SimpleAction::new("stop", None),
        tabs: RefCell::new(Tabs::default()),
        views: RefCell::new(HashMap::new()),
        store,
        preferences: RefCell::new(preferences),
        restoring: Cell::new(true),
        closing: Cell::new(false),
        pending_save: RefCell::new(None),
        metrics: RefCell::new(crate::performance::NavigationMetrics::default()),
        tab_strip,
        tab_scroll,
        reload_button,
        web_context: webkit6::WebContext::new(),
        bookmarks_window: glib::WeakRef::new(),
        bookmarks_list: glib::WeakRef::new(),
        network_session,
        private,
        clearing_data: Cell::new(false),
        downloads: RefCell::new(std::collections::BTreeMap::new()),
        next_download: Cell::new(0),
        downloads_window: glib::WeakRef::new(),
        downloads_list: glib::WeakRef::new(),
        download_exit_prompt: glib::WeakRef::new(),
        exit_confirmed: Cell::new(false),
        permission_prompts: RefCell::new(HashMap::new()),
        #[cfg(test)]
        download_test_destination: RefCell::new(None),
        #[cfg(test)]
        download_test_cancel: Cell::new(false),
    });
    browser.connect_downloads();
    browser.register_start_page();
    browser.bind_actions(&window, app);
    window.connect_map(glib::clone!(
        #[weak]
        browser,
        move |_| browser.metrics.borrow_mut().window_mapped()
    ));
    browser.address.connect_activate(glib::clone!(
        #[weak]
        browser,
        move |entry| {
            let Some((id, view)) = browser.active_view() else {
                return;
            };
            let engine = browser.preferences.borrow().search_engine;
            match navigation_target(entry.text().as_str(), engine) {
                Ok(target) => {
                    browser.set_error(id, None);
                    view.load_uri(target.as_str());
                    view.grab_focus();
                }
                Err(message) => browser.set_error(id, Some(message.into())),
            }
        }
    ));
    browser
        .address
        .connect_changed(|entry| entry.remove_css_class("error"));
    browser.notebook.connect_switch_page(glib::clone!(
        #[weak]
        browser,
        move |_, page, _| {
            if let Some(id) = browser.id_for_page(page) {
                if let Some(previous) = browser
                    .tabs
                    .borrow()
                    .active()
                    .filter(|previous| *previous != id)
                {
                    browser.cancel_permission(previous);
                }
                browser.tabs.borrow_mut().select(id);
                browser.sync(true);
                browser.queue_session();
            }
        }
    ));
    if session.urls.is_empty() {
        let home = browser.preferences.borrow().home_page.clone();
        browser.add_view(browser.new_view(), Some(&home));
    } else {
        for uri in &session.urls {
            browser.add_view(browser.new_view(), Some(uri));
        }
        browser
            .notebook
            .set_current_page(Some(session.active as u32));
    }
    browser.restoring.set(false);
    browser.queue_session();
    window.connect_close_request(glib::clone!(
        #[weak]
        browser,
        #[upgrade_or]
        glib::Propagation::Proceed,
        move |_| {
            if browser.clearing_data.get() {
                return glib::Propagation::Stop;
            }
            if browser.has_active_downloads() && !browser.exit_confirmed.get() {
                browser.confirm_download_exit();
                return glib::Propagation::Stop;
            }
            browser.cancel_all_permissions();
            browser.flush_session();
            browser.closing.set(true);
            glib::Propagation::Proceed
        }
    ));
    // A janela mantém o controlador vivo; o controlador só guarda uma referência fraca à janela.
    let keep_alive = browser.clone();
    window.connect_destroy(move |_| {
        let _ = &keep_alive;
    });
    (window, browser)
}

impl BrowserWindow {
    fn new_view(&self) -> webkit6::WebView {
        webkit6::WebView::builder()
            .web_context(&self.web_context)
            .network_session(&self.network_session)
            .user_content_manager(&webkit6::UserContentManager::new())
            .build()
    }

    fn register_start_page(self: &Rc<Self>) {
        if let Some(security) = self.web_context.security_manager() {
            security.register_uri_scheme_as_local("kairo");
            security.register_uri_scheme_as_display_isolated("kairo");
            security.register_uri_scheme_as_no_access("kairo");
        }
        let browser = self.clone();
        self.web_context.register_uri_scheme(
            "kairo",
            glib::clone!(
                #[weak]
                browser,
                move |request| {
                    if request.uri().as_deref() != Some(start_page::URI) {
                        request.finish_error(&mut glib::Error::new(
                            gio::IOErrorEnum::NotFound,
                            "Página interna não encontrada",
                        ));
                        return;
                    }
                    let history = browser
                        .store
                        .as_ref()
                        .and_then(|store| store.borrow().history().ok())
                        .unwrap_or_default();
                    let engine = browser.preferences.borrow().search_engine;
                    let bookmarks = browser
                        .store
                        .as_ref()
                        .and_then(|store| store.borrow().bookmarks().ok())
                        .unwrap_or_default();
                    let mut html = start_page::render(&history, &bookmarks, engine);
                    if browser.private {
                        html = html
                            .replace("Para onde vamos?", "Navegação privada")
                            .replace(
                                "class=\"manage-shortcuts\"",
                                "class=\"manage-shortcuts\" hidden",
                            );
                    }
                    let bytes = glib::Bytes::from_owned(html.into_bytes());
                    let stream = gio::MemoryInputStream::from_bytes(&bytes);
                    request.finish(
                        &stream,
                        bytes.len() as i64,
                        Some("text/html; charset=utf-8"),
                    );
                }
            ),
        );
    }

    fn sync_start_page(&self, view: &webkit6::WebView) {
        let engine = self.preferences.borrow().search_engine;
        let (history, bookmarks) = self
            .store
            .as_ref()
            .map(|store| {
                let store = store.borrow();
                (
                    store.history().unwrap_or_default(),
                    store.bookmarks().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        let shortcuts = start_page::shortcuts(&history, &bookmarks, engine);
        let encoded = glib::base64_encode(shortcuts.html.as_bytes());
        // Somente constantes dos buscadores entram no script; páginas externas não são alteradas.
        let script = format!(
            "(() => {{ if (location.href !== 'kairo://start/') return; const form = document.querySelector('form.search'); const query = document.getElementById('query'); const label = document.querySelector('label[for=query]'); if (form && query && label) {{ form.action = '{}'; query.placeholder = 'Pesquisar com {}'; label.textContent = query.placeholder; }} const grid = document.querySelector('.shortcut-grid'); const heading = document.getElementById('shortcut-heading'); if (grid && heading) {{ grid.innerHTML = new TextDecoder().decode(Uint8Array.from(atob('{}'), c => c.charCodeAt(0))); heading.textContent = '{}'; }} }})()",
            engine.endpoint(),
            engine.name(),
            encoded,
            shortcuts.title
        );
        view.evaluate_javascript(&script, None, None, None::<&gio::Cancellable>, |_| {});
    }

    fn bind_actions(self: &Rc<Self>, window: &gtk4::ApplicationWindow, app: &gtk4::Application) {
        for (action, callback) in [
            (
                &self.back,
                webkit6::WebView::go_back as fn(&webkit6::WebView),
            ),
            (&self.forward, webkit6::WebView::go_forward),
            (&self.stop, webkit6::WebView::stop_loading),
        ] {
            let browser = self.clone();
            action.connect_activate(glib::clone!(
                #[weak]
                browser,
                move |_, _| {
                    if let Some((_, view)) = browser.active_view() {
                        callback(&view);
                    }
                }
            ));
            window.add_action(action);
        }
        // Endereços da linha de comando: `kairo https://a https://b`.
        let open_uri = gio::SimpleAction::new("open-uri", Some(glib::VariantTy::STRING));
        let browser = self.clone();
        open_uri.connect_activate(glib::clone!(
            #[weak]
            browser,
            move |_, parameter| {
                if let Some(input) = parameter.and_then(|value| value.str()) {
                    browser.open_uri(input);
                }
            }
        ));
        window.add_action(&open_uri);
        self.action(window, "reload", |browser| {
            if let Some((_, view)) = browser.active_view() {
                view.reload();
            }
        });
        self.action(window, "home", |browser| {
            if let Some((_, view)) = browser.active_view() {
                let uri = browser.preferences.borrow().home_page.clone();
                view.load_uri(&uri);
            }
        });
        self.action(window, "history", |browser| browser.show_history());
        self.action(window, "preferences", |browser| browser.show_preferences());
        self.action(window, "private-window", |browser| {
            browser.open_private_window()
        });
        self.action(window, "clear-data", |browser| browser.show_clear_data());
        self.action(window, "performance", |browser| browser.show_performance());
        self.action(window, "downloads", |browser| browser.show_downloads());
        self.action(window, "bookmarks", |browser| browser.show_bookmarks());
        self.action(window, "bookmark-page", |browser| {
            browser.bookmark_current_page()
        });
        self.action(window, "start-page", |browser| {
            if let Some((_, view)) = browser.active_view() {
                view.load_uri(start_page::URI);
            }
        });
        self.action(window, "focus-address", |browser| browser.focus_address());
        self.action(window, "new-tab", |browser| {
            browser.add_view(browser.new_view(), Some(start_page::URI));
            browser.focus_address();
        });
        self.action(window, "close-tab", |browser| {
            if let Some((_, view)) = browser.active_view() {
                view.try_close();
            }
        });
        self.action(window, "next-tab", |browser| browser.switch_tab(1));
        self.action(window, "previous-tab", |browser| browser.switch_tab(-1));
        for (action, shortcuts) in [
            ("win.focus-address", &["<Control>l"][..]),
            ("win.back", &["<Alt>Left"][..]),
            ("win.forward", &["<Alt>Right"][..]),
            ("win.reload", &["<Control>r", "F5"][..]),
            ("win.stop", &["Escape"][..]),
            ("win.home", &["<Alt>Home"][..]),
            ("win.history", &["<Control>h"][..]),
            ("win.downloads", &["<Control>j"][..]),
            ("win.bookmark-page", &["<Control>d"][..]),
            ("win.bookmarks", &["<Control><Shift>o"][..]),
            ("win.new-tab", &["<Control>t"][..]),
            ("win.private-window", &["<Control><Shift>n"][..]),
            ("win.close-tab", &["<Control>w"][..]),
            ("win.next-tab", &["<Control>Tab", "<Control>Page_Down"][..]),
            (
                "win.previous-tab",
                &["<Control><Shift>Tab", "<Control>Page_Up"][..],
            ),
        ] {
            app.set_accels_for_action(action, shortcuts);
        }
        if self.private {
            for name in [
                "history",
                "preferences",
                "bookmarks",
                "bookmark-page",
                "clear-data",
            ] {
                if let Some(action) = window
                    .lookup_action(name)
                    .and_downcast::<gio::SimpleAction>()
                {
                    action.set_enabled(false);
                }
            }
        }
    }

    fn action(
        self: &Rc<Self>,
        window: &gtk4::ApplicationWindow,
        name: &str,
        callback: impl Fn(&Rc<Self>) + 'static,
    ) {
        let action = gio::SimpleAction::new(name, None);
        let browser = self.clone();
        action.connect_activate(glib::clone!(
            #[weak]
            browser,
            move |_, _| {
                if !browser.clearing_data.get() {
                    callback(&browser);
                }
            }
        ));
        window.add_action(&action);
    }

    fn active_view(&self) -> Option<(TabId, webkit6::WebView)> {
        let id = self.tabs.borrow().active()?;
        self.views
            .borrow()
            .get(&id)
            .map(|tab| (id, tab.webview.clone()))
    }

    fn id_for_page(&self, page: &gtk4::Widget) -> Option<TabId> {
        self.views
            .borrow()
            .iter()
            .find_map(|(id, tab)| (tab.webview.upcast_ref::<gtk4::Widget>() == page).then_some(*id))
    }

    fn focus_address(&self) {
        self.address.grab_focus();
        self.address.select_region(0, -1);
    }

    fn select(&self, id: TabId) {
        let page = self
            .views
            .borrow()
            .get(&id)
            .and_then(|tab| self.notebook.page_num(&tab.webview));
        if let Some(page) = page {
            self.notebook.set_current_page(Some(page));
        }
    }

    fn switch_tab(&self, delta: isize) {
        let next = self.tabs.borrow().adjacent(delta);
        if let Some(id) = next {
            self.select(id);
        }
    }

    fn open_uri(self: &Rc<Self>, input: &str) {
        if self.clearing_data.get() || self.closing.get() {
            return;
        }
        let engine = self.preferences.borrow().search_engine;
        let Ok(target) = navigation_target(input, engine) else {
            return;
        };
        // Uma janela recém-aberta só com a página inicial é reaproveitada.
        let untouched = (self.notebook.n_pages() == 1)
            .then(|| self.active_view())
            .flatten()
            .filter(|(_, view)| {
                !view.can_go_back()
                    && matches!(
                        view.uri().as_deref(),
                        None | Some("") | Some("about:blank") | Some(start_page::URI)
                    )
            });
        match untouched {
            Some((_, view)) => view.load_uri(target.as_str()),
            None => self.add_view(self.new_view(), Some(target.as_str())),
        }
    }

    fn set_error(&self, id: TabId, message: Option<String>) {
        if let Some(tab) = self.tabs.borrow_mut().get_mut(id) {
            tab.error = message;
        }
        self.sync(false);
    }

    fn sync(&self, update_address: bool) {
        let Some((id, view)) = self.active_view() else {
            return;
        };
        let tabs = self.tabs.borrow();
        let Some(tab) = tabs.get(id) else {
            return;
        };
        if update_address {
            self.address
                .set_text(if tab.uri == "about:blank" || tab.uri == start_page::URI {
                    ""
                } else {
                    &tab.uri
                });
        }
        self.back.set_enabled(view.can_go_back());
        self.forward.set_enabled(view.can_go_forward());
        self.stop.set_enabled(view.is_loading());
        self.spinner.set_spinning(view.is_loading());
        self.spinner.set_visible(view.is_loading());
        style::set_button_icon(
            &self.reload_button,
            if view.is_loading() {
                kairo::icons::CLOSE
            } else {
                kairo::icons::RELOAD
            },
            18,
        );
        self.reload_button
            .update_property(&[gtk4::accessible::Property::Label(if view.is_loading() {
                "Parar carregamento"
            } else {
                "Recarregar"
            })]);
        self.reload_button
            .set_action_name(Some(if view.is_loading() {
                "win.stop"
            } else {
                "win.reload"
            }));
        self.reload_button
            .set_tooltip_text(Some(if view.is_loading() {
                "Parar carregamento (Esc)"
            } else {
                "Recarregar (Ctrl+R / F5)"
            }));
        for (candidate_id, candidate) in self.views.borrow().iter() {
            if *candidate_id == id {
                candidate.header.add_css_class("selected");
            } else {
                candidate.header.remove_css_class("selected");
            }
        }
        if let Some(bounds) = self
            .views
            .borrow()
            .get(&id)
            .and_then(|tab| tab.header.compute_bounds(&self.tab_strip))
        {
            let adjustment = self.tab_scroll.hadjustment();
            let left = f64::from(bounds.x());
            let right = left + f64::from(bounds.width());
            if left < adjustment.value() {
                adjustment.set_value(left);
            } else if right > adjustment.value() + adjustment.page_size() {
                adjustment.set_value(right - adjustment.page_size());
            }
        }
        self.status.set_text(tab.error.as_deref().unwrap_or(""));
        self.status.set_visible(tab.error.is_some());
        let insecure = self.views.borrow().get(&id).is_some_and(|candidate| {
            candidate.tls_block.borrow().is_some()
                || tab.uri.starts_with("http://")
                || (tab.uri.starts_with("https://")
                    && view
                        .tls_info()
                        .is_some_and(|(_, errors)| !errors.is_empty()))
        });
        self.security.set_visible(insecure);
        if tab.error.is_some() {
            self.address.add_css_class("error");
        } else {
            self.address.remove_css_class("error");
        }
        if let Some(window) = self.window.upgrade() {
            let suffix = if self.private {
                "Kairo — Privado"
            } else {
                "Kairo"
            };
            window.set_title(Some(&format!("{} — {suffix}", tab.title)));
        }
    }

    fn refresh(self: &Rc<Self>, id: TabId, update_address: bool) {
        {
            let views = self.views.borrow();
            let Some(view) = views.get(&id) else {
                return;
            };
            if let Some(tab) = self.tabs.borrow_mut().get_mut(id) {
                tab.uri = view
                    .webview
                    .uri()
                    .map(|uri| uri.to_string())
                    .unwrap_or_default();
                tab.title = view
                    .webview
                    .title()
                    .filter(|title| !title.is_empty())
                    .map(|title| title.to_string())
                    .unwrap_or_else(|| {
                        if tab.uri.is_empty()
                            || tab.uri == "about:blank"
                            || tab.uri == start_page::URI
                        {
                            "Nova aba".into()
                        } else {
                            tab.uri.clone()
                        }
                    });
                view.label.set_text(&tab.title);
                view.label.set_tooltip_text(Some(&tab.title));
            }
        }
        if self.tabs.borrow().active() == Some(id) {
            self.sync(update_address);
        }
        if update_address {
            self.queue_session();
        }
    }

    fn add_view(self: &Rc<Self>, view: webkit6::WebView, uri: Option<&str>) {
        if self.clearing_data.get() {
            view.stop_loading();
            return;
        }
        let id = self.tabs.borrow_mut().open();
        let label = gtk4::Label::builder()
            .label("Nova aba")
            .max_width_chars(20)
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .build();
        let close = style::icon_button(kairo::icons::CLOSE, "Fechar aba", 14);
        close.add_css_class("flat");
        close.add_css_class("kairo-tab-close");
        close.set_tooltip_text(Some("Fechar aba"));
        close.connect_clicked(glib::clone!(
            #[weak]
            view,
            move |_| view.try_close()
        ));
        let tab_header = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        tab_header.add_css_class("kairo-tab");
        tab_header.set_width_request(220);
        tab_header.set_hexpand(false);
        let select = gtk4::Button::builder().child(&label).hexpand(true).build();
        select.add_css_class("flat");
        select.add_css_class("kairo-tab-select");
        let browser = self.clone();
        select.connect_clicked(glib::clone!(
            #[weak]
            browser,
            move |_| browser.select(id)
        ));
        tab_header.append(&select);
        tab_header.append(&close);
        self.tab_strip.append(&tab_header);
        self.views.borrow_mut().insert(
            id,
            TabView {
                webview: view.clone(),
                label,
                history_pending: Cell::new(false),
                header: tab_header,
                is_start_page: Cell::new(false),
                tls_failed: Cell::new(false),
                tls_block: RefCell::new(None),
                interstitial_loading: Cell::new(false),
                interstitial_committed: Cell::new(false),
            },
        );
        self.connect_view(id, &view);
        let page = self.notebook.append_page(&view, None::<&gtk4::Widget>);
        self.notebook.set_current_page(Some(page));
        if let Some(uri) = uri {
            view.load_uri(uri);
        }
        self.refresh(id, true);
    }

    fn remove_tab(self: &Rc<Self>, id: TabId) {
        self.cancel_permission(id);
        self.metrics.borrow_mut().forget(id);
        let view = self.views.borrow_mut().remove(&id);
        let Some(view) = view else {
            return;
        };
        self.tab_strip.remove(&view.header);
        self.tabs.borrow_mut().close(id);
        let active = self.tabs.borrow().active();
        if let Some(page) = self.notebook.page_num(&view.webview) {
            self.notebook.remove_page(Some(page));
        }
        view.webview.stop_loading();
        if let Some(active) = active {
            self.select(active);
            self.sync(true);
        } else if self.has_active_downloads() {
            self.add_view(self.new_view(), Some(start_page::URI));
            self.show_downloads();
        } else if let Some(window) = self.window.upgrade() {
            window.close();
        }
        self.queue_session();
    }

    fn connect_view(self: &Rc<Self>, id: TabId, view: &webkit6::WebView) {
        let browser = self.clone();
        self.connect_permissions(id, view);
        view.connect_decide_policy(|_, decision, kind| {
            if kind == webkit6::PolicyDecisionType::Response
                && let Some(response) = decision.downcast_ref::<webkit6::ResponsePolicyDecision>()
            {
                let attachment = response
                    .response()
                    .and_then(|response| response.http_headers())
                    .and_then(|headers| headers.one("Content-Disposition"))
                    .is_some_and(|value| {
                        value
                            .split(';')
                            .next()
                            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("attachment"))
                    });
                if attachment || !response.is_mime_type_supported() {
                    decision.download();
                    return true;
                }
            }
            false
        });
        if let Some(manager) = view.user_content_manager() {
            manager.register_script_message_handler("kairoBookmarks", None);
            manager.connect_script_message_received(
                Some("kairoBookmarks"),
                glib::clone!(
                    #[weak]
                    browser,
                    move |_, value| {
                        let trusted = browser
                            .views
                            .borrow()
                            .get(&id)
                            .is_some_and(|tab| tab.is_start_page.get());
                        if !browser.private
                            && !browser.clearing_data.get()
                            && trusted
                            && value.is_string()
                            && value.to_str() == "manage"
                        {
                            browser.show_bookmarks();
                        }
                    }
                ),
            );
        }
        view.connect_uri_notify(glib::clone!(
            #[weak]
            browser,
            move |_| browser.refresh(id, true)
        ));
        view.connect_title_notify(glib::clone!(
            #[weak]
            browser,
            move |_| browser.refresh(id, false)
        ));
        view.connect_is_loading_notify(glib::clone!(
            #[weak]
            browser,
            move |_| browser.refresh(id, false)
        ));
        if let Some(history) = view.back_forward_list() {
            history.connect_local(
                "changed",
                false,
                glib::clone!(
                    #[weak]
                    browser,
                    #[upgrade_or]
                    None,
                    move |_| {
                        browser.refresh(id, false);
                        None
                    }
                ),
            );
        }
        view.connect_load_changed(glib::clone!(
            #[weak]
            browser,
            move |view, event| {
                let interstitial = browser.views.borrow().get(&id).is_some_and(|tab| {
                    if !tab.interstitial_loading.get() {
                        return false;
                    }
                    // Outra navegação começou antes de o aviso terminar de carregar.
                    let same = tab
                        .tls_block
                        .borrow()
                        .as_ref()
                        .is_some_and(|block| view.uri().as_deref() == Some(block.uri.as_str()));
                    if event == webkit6::LoadEvent::Committed {
                        tab.interstitial_committed.set(true);
                    }
                    if !same
                        || (event == webkit6::LoadEvent::Finished
                            && tab.interstitial_committed.get())
                    {
                        tab.interstitial_loading.set(false);
                        tab.interstitial_committed.set(false);
                    }
                    same
                });
                if interstitial {
                    browser.refresh(id, false);
                    return;
                }
                let record = browser.views.borrow().get(&id).is_some_and(|tab| {
                    match event {
                        webkit6::LoadEvent::Started => {
                            tab.history_pending.set(false);
                            tab.tls_failed.set(false);
                            tab.tls_block.replace(None);
                        }
                        webkit6::LoadEvent::Committed => {
                            tab.history_pending.set(true);
                            tab.is_start_page
                                .set(view.uri().as_deref() == Some(start_page::URI));
                        }
                        webkit6::LoadEvent::Finished => return tab.history_pending.replace(false),
                        _ => (),
                    }
                    false
                });
                if event == webkit6::LoadEvent::Started {
                    browser.cancel_permission(id);
                    browser.metrics.borrow_mut().begin(id);
                    browser.set_error(id, None);
                }
                browser.refresh(id, false);
                if event == webkit6::LoadEvent::Finished
                    && view.uri().as_deref() == Some(start_page::URI)
                {
                    browser.sync_start_page(view);
                }
                if record {
                    browser.metrics.borrow_mut().complete(id);
                    browser.record_visit(id);
                }
            }
        ));
        view.connect_load_failed(glib::clone!(
            #[weak]
            browser,
            #[upgrade_or]
            false,
            move |_, _, _, error| {
                browser.metrics.borrow_mut().fail(
                    id,
                    error.matches(webkit6::NetworkError::Cancelled)
                        || error.matches(webkit6::PolicyError::FrameLoadInterruptedByPolicyChange),
                );
                if let Some(tab) = browser.views.borrow().get(&id) {
                    tab.history_pending.set(false);
                }
                let tls_failed = browser
                    .views
                    .borrow()
                    .get(&id)
                    .is_some_and(|tab| tab.tls_failed.get());
                if !tls_failed
                    && !error.matches(webkit6::NetworkError::Cancelled)
                    && !error.matches(webkit6::PolicyError::FrameLoadInterruptedByPolicyChange)
                {
                    browser.set_error(
                        id,
                        Some(format!("Não foi possível carregar a página: {error}")),
                    );
                }
                false
            }
        ));
        view.connect_close(glib::clone!(
            #[weak]
            browser,
            move |_| browser.remove_tab(id)
        ));
        view.connect_create(glib::clone!(
            #[weak]
            browser,
            #[upgrade_or]
            None,
            move |parent, action| {
                // Pop-ups sem interação direta são recusados. O WebKit carrega o destino.
                if browser.clearing_data.get() || !action.is_user_gesture() {
                    return None;
                }
                let child = webkit6::WebView::builder()
                    .related_view(parent)
                    .user_content_manager(&webkit6::UserContentManager::new())
                    .build();
                child.connect_ready_to_show(glib::clone!(
                    #[weak]
                    browser,
                    move |child| {
                        if browser.id_for_page(child.upcast_ref()).is_none() {
                            browser.add_view(child.clone(), None);
                        }
                    }
                ));
                Some(child.upcast())
            }
        ));
    }
}

// Este módulo é compartilhado com o teste GTK de thread principal.
#[cfg(test)]
#[allow(dead_code)]
pub(crate) mod tests;
