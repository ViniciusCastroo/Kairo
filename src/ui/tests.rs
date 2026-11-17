use super::*;

pub fn run() {
    gtk4::init().expect("GTK deve inicializar");
    let app = gtk4::Application::builder()
        .application_id("dev.example.Kairo.Test")
        .flags(gio::ApplicationFlags::NON_UNIQUE)
        .build();
    app.register(None::<&gio::Cancellable>).unwrap();
    if std::env::args().any(|arg| arg == "--only-fonts") {
        start_page_works(&app);
        return;
    }
    if std::env::args().any(|arg| arg == "--only-tls") {
        tls_errors_are_blocked(&app);
        return;
    }
    if std::env::args().any(|arg| arg == "--only-privacy") {
        privacy_works(&app);
        return;
    }
    let mut store = Store::memory().unwrap();
    store
        .save_preferences(&Preferences {
            home_page: "about:blank".into(),
            ..Default::default()
        })
        .unwrap();
    let store = Rc::new(RefCell::new(store));
    let (window, browser) = build_window(&app, Some(store.clone()), None);
    let first = browser.tabs.borrow().active().unwrap();
    for _ in 0..9 {
        browser.add_view(webkit6::WebView::new(), Some("about:blank"));
    }
    assert_eq!(browser.views.borrow().len(), 10);
    assert_eq!(browser.notebook.n_pages(), 10);
    wait_until(|| {
        browser
            .views
            .borrow()
            .values()
            .all(|tab| !tab.webview.is_loading())
    });
    window.present();
    collect_smoke_baseline_if_requested(&browser);
    let last = browser.tabs.borrow().active().unwrap();
    assert_ne!(first, last);
    browser.switch_tab(1);
    assert_eq!(browser.tabs.borrow().active(), Some(first));
    assert_eq!(browser.address.text(), "");

    // Erros e eventos de uma aba em segundo plano não substituem a aba atual.
    browser.set_error(last, Some("Erro de outra aba".into()));
    assert!(!browser.status.is_visible());
    browser.select(last);
    assert_eq!(browser.status.text(), "Erro de outra aba");
    browser.select(first);
    assert!(!browser.status.is_visible());
    let view = browser.views.borrow().get(&last).unwrap().webview.clone();
    view.try_close();
    wait_until(|| !browser.views.borrow().contains_key(&last));
    assert_eq!(browser.tabs.borrow().active(), Some(first));
    assert_eq!(browser.notebook.n_pages(), 9);
    window.present();
    history_can_be_closed(&window, &browser);

    let ids: Vec<_> = browser.views.borrow().keys().copied().collect();
    for id in ids {
        browser.remove_tab(id);
    }
    assert_eq!(browser.notebook.n_pages(), 0);
    assert!(browser.tabs.borrow().active().is_none());
    assert!(browser.views.borrow().is_empty());
    window.destroy();
    persistence_survives_window_restart(&app);
    start_page_works(&app);
    bookmarks_work(&app);
    downloads_work(&app);
    permissions_work(&app);
    tls_errors_are_blocked(&app);
    privacy_works(&app);
}

fn privacy_works(app: &gtk4::Application) {
    let server = LocalServer::start();
    let directory = std::env::temp_dir().join(format!(
        "kairo-privacy-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&directory).unwrap();
    let network = webkit6::NetworkSession::new(
        Some(directory.join("data").to_str().unwrap()),
        Some(directory.join("cache").to_str().unwrap()),
    );
    let mut profile = Store::memory().unwrap();
    profile
        .save_preferences(&Preferences {
            home_page: "about:blank".into(),
            ..Default::default()
        })
        .unwrap();
    profile
        .save_bookmark(None, "Preservado", "https://example.org/bookmark")
        .unwrap();
    let profile = Rc::new(RefCell::new(profile));
    let preferences = profile.borrow().preferences().unwrap();
    let (normal_window, normal) =
        build_window_in_mode(app, Some(profile.clone()), None, None, network);
    normal_window.present();
    let (_, normal_view) = normal.active_view().unwrap();
    normal_view.load_uri(&format!("{}/normal", server.url));
    wait_until(|| {
        normal_view.title().as_deref() == Some("Local Kairo test") && !normal_view.is_loading()
    });
    javascript(
        &normal_view,
        "document.cookie = 'kairo=normal; path=/; max-age=3600'; localStorage.setItem('kairo', 'normal'); sessionStorage.setItem('kairo', 'normal'); 'seeded'",
    );
    javascript(
        &normal_view,
        "window.seedReady = 'pending'; const db = indexedDB.open('kairo-test', 1); db.onupgradeneeded = () => db.result.createObjectStore('items'); db.onerror = () => window.seedReady = 'error'; db.onsuccess = () => { db.result.close(); caches.open('kairo-test').then(cache => cache.put('/cache-fixture', new Response('cached'))).then(() => window.seedReady = 'ready').catch(() => window.seedReady = 'error'); }; 'started'",
    );
    wait_until(|| javascript(&normal_view, "window.seedReady") != "pending");
    assert_eq!(javascript(&normal_view, "window.seedReady"), "ready");
    normal.flush_session();
    let saved_session = profile.borrow().session().unwrap();
    let saved_history = profile.borrow().history().unwrap().len();

    let (private_window, private) = build_window_in_mode(
        app,
        Some(profile.clone()),
        None,
        Some(preferences.clone()),
        webkit6::NetworkSession::new_ephemeral(),
    );
    private_window.present();
    let (_, private_view) = private.active_view().unwrap();
    wait_until(|| {
        private_view.title().as_deref() == Some("Nova aba") && !private_view.is_loading()
    });
    assert!(private.private && private.store.is_none());
    assert!(private.network_session.is_ephemeral());
    assert_eq!(
        javascript(
            &private_view,
            "document.getElementById('heading').textContent"
        ),
        "Navegação privada"
    );
    assert_eq!(
        javascript(
            &private_view,
            "document.querySelector('.manage-shortcuts').hidden"
        ),
        "true"
    );
    assert_eq!(
        javascript(
            &private_view,
            "getComputedStyle(document.querySelector('.manage-shortcuts')).display"
        ),
        "none"
    );
    save_preview_if_requested(&private_window);
    assert!(private_window.title().unwrap().contains("Privado"));
    for name in [
        "history",
        "preferences",
        "bookmarks",
        "bookmark-page",
        "clear-data",
    ] {
        assert!(!private_window.lookup_action(name).unwrap().is_enabled());
    }
    private_view.load_uri(&format!("{}/private", server.url));
    wait_until(|| {
        private_view.title().as_deref() == Some("Local Kairo test") && !private_view.is_loading()
    });
    assert_eq!(javascript(&private_view, "document.cookie"), "");
    assert_eq!(
        javascript(&private_view, "String(localStorage.getItem('kairo'))"),
        "null"
    );
    javascript(
        &private_view,
        "document.cookie = 'kairo=private; path=/'; localStorage.setItem('kairo', 'private'); 'seeded'",
    );
    let sibling = private.new_view();
    assert_eq!(
        sibling.network_session(),
        Some(private.network_session.clone())
    );
    private.add_view(
        sibling.clone(),
        Some(&format!("{}/private-tab", server.url)),
    );
    wait_until(|| sibling.title().as_deref() == Some("Local Kairo test") && !sibling.is_loading());
    assert_eq!(javascript(&sibling, "document.cookie"), "kairo=private");
    let related = webkit6::WebView::builder()
        .related_view(&private_view)
        .build();
    assert_eq!(
        related.network_session(),
        Some(private.network_session.clone())
    );
    private.flush_session();
    assert_eq!(profile.borrow().session().unwrap(), saved_session);
    assert_eq!(profile.borrow().history().unwrap().len(), saved_history);
    assert_eq!(javascript(&normal_view, "document.cookie"), "kairo=normal");
    assert_eq!(
        javascript(&normal_view, "localStorage.getItem('kairo')"),
        "normal"
    );

    // Uma segunda janela privada também começa sem os dados da primeira.
    let (fresh_window, fresh) = build_window_in_mode(
        app,
        None,
        None,
        Some(preferences.clone()),
        webkit6::NetworkSession::new_ephemeral(),
    );
    let (_, fresh_view) = fresh.active_view().unwrap();
    fresh_view.load_uri(&format!("{}/fresh", server.url));
    wait_until(|| {
        fresh_view.title().as_deref() == Some("Local Kairo test") && !fresh_view.is_loading()
    });
    assert_eq!(javascript(&fresh_view, "document.cookie"), "");
    assert_eq!(
        javascript(&fresh_view, "String(localStorage.getItem('kairo'))"),
        "null"
    );
    fresh_window.destroy();

    // Cancelar a confirmação não modifica o perfil ou os dados do site.
    normal.show_clear_data();
    let confirmation = gtk4::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<gtk4::Window>().ok())
        .find(|window| window.title().as_deref() == Some("Limpar dados de navegação"))
        .unwrap();
    confirmation.close();
    assert_eq!(profile.borrow().session().unwrap(), saved_session);
    assert_eq!(javascript(&normal_view, "document.cookie"), "kairo=normal");

    // Não reutiliza a referência antiga após a limpeza: abre uma WebView nova.
    normal.show_clear_data();
    let confirmation = gtk4::Window::list_toplevels()
        .into_iter()
        .filter_map(|window| window.downcast::<gtk4::Window>().ok())
        .find(|window| window.title().as_deref() == Some("Limpar dados de navegação"))
        .unwrap();
    find_button(&confirmation.child().unwrap(), "Fechar abas e limpar dados")
        .unwrap()
        .emit_clicked();
    assert!(normal.clearing_data.get());
    assert!(!normal_window.is_sensitive());
    assert!(profile.borrow().session().unwrap().urls.is_empty());
    drop(normal_view);
    wait_until(|| !normal.clearing_data.get());
    assert!(normal_window.is_sensitive());
    assert_eq!(normal.notebook.n_pages(), 1);
    assert_eq!(normal.storage_status.text(), "Dados de navegação limpos.");
    assert!(profile.borrow().history().unwrap().is_empty());
    assert_eq!(profile.borrow().bookmarks().unwrap().len(), 1);
    assert_eq!(profile.borrow().preferences().unwrap(), preferences);
    let (_, cleared_view) = normal.active_view().unwrap();
    cleared_view.load_uri(&format!("{}/cleared", server.url));
    wait_until(|| {
        cleared_view.title().as_deref() == Some("Local Kairo test") && !cleared_view.is_loading()
    });
    assert_eq!(javascript(&cleared_view, "document.cookie"), "");
    assert_eq!(
        javascript(&cleared_view, "String(localStorage.getItem('kairo'))"),
        "null"
    );
    assert_eq!(
        javascript(&cleared_view, "String(sessionStorage.getItem('kairo'))"),
        "null"
    );
    assert_eq!(
        javascript(&private_view, "document.cookie"),
        "kairo=private"
    );
    assert_eq!(
        javascript(&private_view, "localStorage.getItem('kairo')"),
        "private"
    );
    javascript(
        &cleared_view,
        "window.probe = 'pending'; Promise.all([indexedDB.databases(), caches.keys()]).then(([dbs, keys]) => window.probe = JSON.stringify([dbs.length, keys.length])).catch(() => window.probe = 'error'); 'started'",
    );
    wait_until(|| javascript(&cleared_view, "window.probe") != "pending");
    assert_eq!(javascript(&cleared_view, "window.probe"), "[0,0]");
    let private_session = private.network_session.downgrade();
    drop(related);
    drop(sibling);
    drop(private_view);
    private_window.destroy();
    drop(private_window);
    drop(private);
    wait_until(|| private_session.upgrade().is_none());
    normal_window.destroy();
    println!(
        "GTK: janela privada isolada; limpeza de cookies, armazenamento, histórico e sessão verificada."
    );
}

fn downloads_work(app: &gtk4::Application) {
    use crate::browser::downloads::DownloadState;
    let server = LocalServer::start();
    let directory = std::env::temp_dir().join(format!(
        "kairo-downloads-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let mut store = Store::memory().unwrap();
    store
        .save_preferences(&Preferences {
            home_page: format!("{}/page", server.url),
            ..Default::default()
        })
        .unwrap();
    let (window, browser) = build_window(app, Some(Rc::new(RefCell::new(store))), None);
    window.present();
    let (_, view) = browser.active_view().unwrap();
    wait_until(|| !view.is_loading());
    let target = directory.join("café file.bin");
    *browser.download_test_destination.borrow_mut() = Some(gio::File::for_path(&target));
    view.load_uri(&format!("{}/download", server.url));
    wait_until(|| {
        browser
            .downloads
            .borrow()
            .get(&0)
            .is_some_and(|task| !task.transfer.active())
    });
    assert_eq!(
        browser.downloads.borrow().get(&0).unwrap().transfer.state,
        DownloadState::Complete
    );
    assert_eq!(std::fs::read(&target).unwrap(), vec![b'K'; 128 * 1024]);
    *browser.download_test_destination.borrow_mut() = Some(gio::File::for_path(&target));
    view.download_uri(&format!("{}/download", server.url))
        .unwrap();
    wait_until(|| {
        browser
            .downloads
            .borrow()
            .get(&1)
            .is_some_and(|task| !task.transfer.active())
    });
    assert!(matches!(
        browser.downloads.borrow().get(&1).unwrap().transfer.state,
        DownloadState::Failed(_)
    ));
    assert_eq!(std::fs::read(&target).unwrap(), vec![b'K'; 128 * 1024]);
    browser.download_test_cancel.set(true);
    view.download_uri(&format!("{}/download", server.url))
        .unwrap();
    wait_until(|| {
        browser
            .downloads
            .borrow()
            .get(&2)
            .is_some_and(|task| !task.transfer.active())
    });
    assert_eq!(
        browser.downloads.borrow().get(&2).unwrap().transfer.state,
        DownloadState::Cancelled
    );
    *browser.download_test_destination.borrow_mut() =
        Some(gio::File::for_path(directory.join("failed.bin")));
    view.download_uri(&format!("{}/download-fail", server.url))
        .unwrap();
    wait_until(|| {
        browser
            .downloads
            .borrow()
            .get(&3)
            .is_some_and(|task| !task.transfer.active())
    });
    assert!(matches!(
        browser.downloads.borrow().get(&3).unwrap().transfer.state,
        DownloadState::Failed(_)
    ));
    *browser.download_test_destination.borrow_mut() =
        Some(gio::File::for_path(directory.join("cancelled.bin")));
    view.download_uri(&format!("{}/download-slow", server.url))
        .unwrap();
    wait_until(|| {
        browser
            .downloads
            .borrow()
            .get(&4)
            .is_some_and(|task| task.transfer.received > 0)
    });
    window.close();
    let prompt = visible_dialog("Downloads em andamento");
    find_button(&prompt.titlebar().unwrap(), "Continuar baixando")
        .unwrap()
        .emit_clicked();
    assert!(window.is_visible());
    let manager = visible_dialog("Downloads do Kairo");
    tooltip_button(&manager.child().unwrap(), "Cancelar download", true)
        .unwrap()
        .emit_clicked();
    wait_until(|| !browser.has_active_downloads());
    assert_eq!(
        browser.downloads.borrow().get(&4).unwrap().transfer.state,
        DownloadState::Cancelled
    );
    *browser.download_test_destination.borrow_mut() =
        Some(gio::File::for_path(directory.join("exit.bin")));
    view.download_uri(&format!("{}/download-slow", server.url))
        .unwrap();
    wait_until(|| {
        browser
            .downloads
            .borrow()
            .get(&5)
            .is_some_and(|task| task.transfer.received > 0)
    });
    window.close();
    let prompt = visible_dialog("Downloads em andamento");
    find_button(&prompt.child().unwrap(), "Cancelar downloads e sair")
        .unwrap()
        .emit_clicked();
    wait_until(|| !window.is_visible() && !browser.has_active_downloads());
    assert_eq!(
        browser.downloads.borrow().get(&5).unwrap().transfer.state,
        DownloadState::Cancelled
    );
    std::fs::remove_dir_all(directory).unwrap();
}

fn permissions_work(app: &gtk4::Application) {
    let server = LocalServer::start();
    let mut store = Store::memory().unwrap();
    store
        .save_preferences(&Preferences {
            home_page: format!("{}/permissions", server.url),
            ..Default::default()
        })
        .unwrap();
    let (window, browser) = build_window(app, Some(Rc::new(RefCell::new(store))), None);
    window.present();
    let (_, view) = browser.active_view().unwrap();
    wait_until(|| !view.is_loading());
    javascript(
        &view,
        "window.geoStatus = 'pending'; navigator.geolocation.getCurrentPosition(() => window.geoStatus = 'allowed', error => window.geoStatus = 'denied:' + error.code); 'requested'",
    );
    wait_until(|| !browser.permission_prompts.borrow().is_empty());
    let dialog = visible_dialog("Permissão do site");
    find_button(&dialog.child().unwrap(), "Não permitir")
        .unwrap()
        .emit_clicked();
    wait_until(|| browser.permission_prompts.borrow().is_empty());
    assert_eq!(javascript(&view, "window.geoStatus"), "denied:1");
    let second = LocalServer::start();
    view.load_uri(&format!("{}/permissions", second.url));
    wait_until(|| !view.is_loading());
    javascript(
        &view,
        "navigator.geolocation.getCurrentPosition(() => {}, () => {}); 'requested'",
    );
    wait_until(|| !browser.permission_prompts.borrow().is_empty());
    let prompt = visible_dialog("Permissão do site");
    view.load_uri(&format!("{}/navigated", server.url));
    wait_until(|| !view.is_loading() && browser.permission_prompts.borrow().is_empty());
    assert!(!prompt.is_visible());
    let third = LocalServer::start();
    browser.add_view(
        browser.new_view(),
        Some(&format!("{}/background", third.url)),
    );
    let (_, background) = browser.active_view().unwrap();
    wait_until(|| !background.is_loading());
    browser.add_view(browser.new_view(), Some("about:blank"));
    javascript(
        &background,
        "window.geoStatus = 'pending'; navigator.geolocation.getCurrentPosition(() => window.geoStatus = 'allowed', error => window.geoStatus = 'denied:' + error.code); 'requested'",
    );
    wait_until(|| javascript(&background, "window.geoStatus") != "pending");
    assert_eq!(javascript(&background, "window.geoStatus"), "denied:1");
    assert!(browser.permission_prompts.borrow().is_empty());
    window.destroy();
}

fn tls_errors_are_blocked(app: &gtk4::Application) {
    let server = TlsServer::start();
    let mut store = Store::memory().unwrap();
    store
        .save_preferences(&Preferences {
            home_page: "about:blank".into(),
            ..Default::default()
        })
        .unwrap();
    let (window, browser) = build_window(app, Some(Rc::new(RefCell::new(store))), None);
    window.present();
    let (id, view) = browser.active_view().unwrap();
    wait_until(|| !view.is_loading());
    view.load_uri(&server.url);
    wait_until(|| {
        browser.tabs.borrow().get(id).is_some_and(|tab| {
            tab.error
                .as_ref()
                .is_some_and(|error| error.contains("certificado"))
        }) && !view.is_loading()
    });
    assert!(browser.status.is_visible());
    assert!(
        browser
            .store
            .as_ref()
            .unwrap()
            .borrow()
            .history()
            .unwrap()
            .is_empty()
    );
    window.destroy();
}

struct TlsServer {
    child: std::process::Child,
    directory: std::path::PathBuf,
    url: String,
}

impl TlsServer {
    fn start() -> Self {
        use std::io::BufRead;
        let directory = std::env::temp_dir().join(format!(
            "kairo-tls-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&directory).unwrap();
        let cert = directory.join("cert.pem");
        let key = directory.join("key.pem");
        let output = std::process::Command::new("openssl")
            .args([
                "req",
                "-x509",
                "-newkey",
                "rsa:2048",
                "-nodes",
                "-subj",
                "/CN=localhost",
                "-days",
                "1",
                "-addext",
                "subjectAltName=DNS:localhost",
                "-keyout",
            ])
            .arg(&key)
            .arg("-out")
            .arg(&cert)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "Não foi possível gerar o certificado de teste"
        );
        let script = "import http.server, ssl, sys\nclass Handler(http.server.BaseHTTPRequestHandler):\n def do_GET(self):\n  self.send_response(200)\n  self.end_headers()\n  self.wfile.write(b'tls-test')\n def log_message(self, *args): pass\ns = http.server.HTTPServer(('127.0.0.1', 0), Handler)\nc = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)\nc.load_cert_chain(sys.argv[1], sys.argv[2])\ns.socket = c.wrap_socket(s.socket, server_side=True)\nprint(s.server_port, flush=True)\ns.serve_forever()";
        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .arg(&cert)
            .arg(&key)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap();
        let mut port = String::new();
        std::io::BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut port)
            .unwrap();
        let port = port.trim().parse::<u16>().unwrap();
        Self {
            child,
            directory,
            url: format!("https://localhost:{port}/"),
        }
    }
}

impl Drop for TlsServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn bookmarks_work(app: &gtk4::Application) {
    let server = LocalServer::start();
    let first_uri = format!("{}/first-favorite", server.url);
    let second_uri = format!("{}/second-favorite", server.url);
    let profile = Rc::new(RefCell::new(Store::memory().unwrap()));
    let (window, browser) = build_window(app, Some(profile.clone()), None);
    window.present();
    let (_, view) = browser.active_view().unwrap();
    wait_until(|| view.title().as_deref() == Some("Nova aba") && !view.is_loading());
    javascript(
        &view,
        "document.querySelector('.manage-shortcuts').click(); 'opened'",
    );
    wait_until(|| {
        gtk4::Window::list_toplevels()
            .into_iter()
            .filter_map(|widget| widget.downcast::<gtk4::Window>().ok())
            .any(|window| {
                window.is_visible() && window.title().as_deref() == Some("Favoritos do Kairo")
            })
    });
    let manager = visible_dialog("Favoritos do Kairo");
    assert_eq!(view.uri().as_deref(), Some(start_page::URI));
    find_button(&manager.child().unwrap(), "Adicionar favorito")
        .unwrap()
        .emit_clicked();
    let editor = visible_dialog("Adicionar favorito");
    let name = bookmark_entry(&editor.child().unwrap(), "bookmark-name").unwrap();
    let address = bookmark_entry(&editor.child().unwrap(), "bookmark-address").unwrap();
    name.set_text("<b> Café & GTK");
    address.set_text("javascript:alert(1)");
    find_button(&editor.child().unwrap(), "Salvar favorito")
        .unwrap()
        .emit_clicked();
    assert!(editor.is_visible());
    assert!(profile.borrow().bookmarks().unwrap().is_empty());
    address.set_text(&first_uri);
    find_button(&editor.child().unwrap(), "Salvar favorito")
        .unwrap()
        .emit_clicked();
    wait_until(|| !editor.is_visible());
    let first = profile.borrow().bookmarks().unwrap()[0].id;
    assert_eq!(
        javascript(
            &view,
            "document.getElementById('shortcut-heading').textContent"
        ),
        "Favoritos"
    );
    assert_eq!(
        javascript(&view, "document.querySelector('.site-title').textContent"),
        "<b> Café & GTK"
    );
    assert_eq!(
        javascript(&view, "document.querySelector('.site-title b') === null"),
        "true"
    );

    find_button(&manager.child().unwrap(), "Adicionar favorito")
        .unwrap()
        .emit_clicked();
    let editor = visible_dialog("Adicionar favorito");
    bookmark_entry(&editor.child().unwrap(), "bookmark-name")
        .unwrap()
        .set_text("Second");
    bookmark_entry(&editor.child().unwrap(), "bookmark-address")
        .unwrap()
        .set_text(&second_uri);
    find_button(&editor.child().unwrap(), "Salvar favorito")
        .unwrap()
        .emit_clicked();
    wait_until(|| !editor.is_visible());
    let second = profile.borrow().bookmarks().unwrap()[1].id;
    tooltip_button(&manager.child().unwrap(), "Mover para cima", true)
        .unwrap()
        .emit_clicked();
    assert_eq!(profile.borrow().bookmarks().unwrap()[0].id, second);
    assert_eq!(
        javascript(&view, "document.querySelector('.site-title').textContent"),
        "Second"
    );
    tooltip_button(&manager.child().unwrap(), "Editar favorito", true)
        .unwrap()
        .emit_clicked();
    let editor = visible_dialog("Editar favorito");
    bookmark_entry(&editor.child().unwrap(), "bookmark-name")
        .unwrap()
        .set_text("Edited second");
    find_button(&editor.child().unwrap(), "Salvar favorito")
        .unwrap()
        .emit_clicked();
    wait_until(|| !editor.is_visible());
    assert_eq!(
        profile.borrow().bookmarks().unwrap()[0].title,
        "Edited second"
    );
    tooltip_button(&manager.child().unwrap(), "Remover favorito", true)
        .unwrap()
        .emit_clicked();
    assert_eq!(profile.borrow().bookmarks().unwrap()[0].id, first);
    assert_eq!(
        javascript(&view, "document.querySelectorAll('.shortcut').length"),
        "1"
    );
    profile.borrow().clear_history().unwrap();
    assert_eq!(profile.borrow().bookmarks().unwrap().len(), 1);
    find_button(&manager.titlebar().unwrap(), "Voltar à navegação")
        .unwrap()
        .emit_clicked();
    javascript(
        &view,
        "document.querySelector('.shortcut').click(); 'navigated'",
    );
    wait_until(|| view.uri().as_deref() == Some(first_uri.as_str()) && !view.is_loading());
    browser.bookmark_current_page();
    let editor = visible_dialog("Editar favorito");
    assert_eq!(
        bookmark_entry(&editor.child().unwrap(), "bookmark-name")
            .unwrap()
            .text(),
        "<b> Café & GTK"
    );
    find_button(&editor.titlebar().unwrap(), "Cancelar")
        .unwrap()
        .emit_clicked();
    assert_eq!(profile.borrow().bookmarks().unwrap().len(), 1);

    // Uma página web não pode abrir os controles internos de favoritos por navegação.
    javascript(
        &view,
        "window.webkit.messageHandlers.kairoBookmarks.postMessage('manage'); 'blocked'",
    );
    let settled = std::time::Instant::now();
    wait_until(|| settled.elapsed() >= std::time::Duration::from_millis(200));
    assert!(!manager.is_visible());
    assert!(
        !browser
            .bookmarks_window
            .upgrade()
            .is_some_and(|window| window.is_visible())
    );
    view.go_back();
    wait_until(|| view.uri().as_deref() == Some(start_page::URI) && !view.is_loading());
    window.close();
    let (restored_window, restored) = build_window(app, Some(profile.clone()), None);
    restored_window.present();
    let (_, restored_view) = restored.active_view().unwrap();
    wait_until(|| {
        restored_view.title().as_deref() == Some("Nova aba") && !restored_view.is_loading()
    });
    assert_eq!(
        javascript(
            &restored_view,
            "document.getElementById('shortcut-heading').textContent"
        ),
        "Favoritos"
    );
    restored.show_bookmarks();
    let manager = visible_dialog("Favoritos do Kairo");
    tooltip_button(&manager.child().unwrap(), "Remover favorito", true)
        .unwrap()
        .emit_clicked();
    assert!(profile.borrow().bookmarks().unwrap().is_empty());
    assert_ne!(
        javascript(
            &restored_view,
            "document.getElementById('shortcut-heading').textContent"
        ),
        "Favoritos"
    );
    restored_window.destroy();
}

fn bookmark_entry(widget: &gtk4::Widget, name: &str) -> Option<gtk4::Entry> {
    if widget.widget_name() == name {
        return widget.clone().downcast::<gtk4::Entry>().ok();
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(entry) = bookmark_entry(&current, name) {
            return Some(entry);
        }
        child = current.next_sibling();
    }
    None
}

fn tooltip_button(widget: &gtk4::Widget, text: &str, enabled: bool) -> Option<gtk4::Button> {
    if let Some(button) = widget.downcast_ref::<gtk4::Button>()
        && button.tooltip_text().as_deref() == Some(text)
        && (!enabled || button.is_sensitive())
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(button) = tooltip_button(&current, text, enabled) {
            return Some(button);
        }
        child = current.next_sibling();
    }
    None
}

fn start_page_works(app: &gtk4::Application) {
    let server = LocalServer::start();
    let uri = format!("{}/recent", server.url);
    let profile = Rc::new(RefCell::new(Store::memory().unwrap()));
    profile
        .borrow()
        .record_visit(&uri, "Uma página recente")
        .unwrap();
    let (window, browser) = build_window(app, Some(profile.clone()), None);
    window.present();
    let (_, view) = browser.active_view().unwrap();
    wait_until(|| view.title().as_deref() == Some("Nova aba") && !view.is_loading());
    assert_eq!(view.uri().as_deref(), Some(start_page::URI));
    assert_eq!(browser.address.text(), "");
    assert_eq!(profile.borrow().history().unwrap().len(), 1);
    assert_eq!(
        javascript(&view, "document.querySelectorAll('.shortcut').length"),
        "1"
    );
    assert_ne!(
        javascript(&view, "document.getElementById('clock').textContent"),
        "--:--"
    );
    assert_eq!(
        javascript(
            &view,
            "document.querySelector('.brand-logo').naturalWidth > 0"
        ),
        "true"
    );
    assert!(
        gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap())
            .has_icon(kairo::branding::ICON_NAME)
    );
    let context = browser.address.pango_context();
    let description = context.font_description().unwrap();
    let font = context.load_font(&description).unwrap();
    assert_eq!(
        font.describe().family().as_deref(),
        Some("Roboto"),
        "CSS: {description}; mapa: {}",
        context.font_map().unwrap().type_()
    );
    wait_until(|| javascript(&view, "document.fonts.status") == "loaded");
    assert_eq!(javascript(&view, "document.fonts.size"), "1");
    assert_eq!(
        javascript(
            &view,
            "Array.from(document.fonts).every(font => font.status === 'loaded')"
        ),
        "true"
    );
    let icon_theme = gtk4::IconTheme::for_display(&gtk4::gdk::Display::default().unwrap());
    for icon in kairo::icons::ALL {
        assert!(icon_theme.has_icon(icon), "Ícone embarcado ausente: {icon}");
        assert!(
            icon_theme
                .lookup_icon(
                    icon,
                    &[],
                    18,
                    1,
                    gtk4::TextDirection::Ltr,
                    gtk4::IconLookupFlags::empty(),
                )
                .is_symbolic(),
            "Ícone precisa acompanhar as cores do tema: {icon}"
        );
    }
    save_preview_if_requested(&window);

    // Intercepta a pesquisa antes da rede: valida a ação real do formulário sem acessar serviços externos.
    let search = Rc::new(RefCell::new(None::<String>));
    let captured = search.clone();
    view.connect_decide_policy(move |_, decision, kind| {
        if kind == webkit6::PolicyDecisionType::NavigationAction
            && let Some(navigation) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>()
            && let Some(uri) = navigation
                .navigation_action()
                .and_then(|action| action.request())
                .and_then(|request| request.uri())
            && (uri.starts_with("https://duckduckgo.com/")
                || uri.starts_with("https://www.google.com/search"))
        {
            *captured.borrow_mut() = Some(uri.to_string());
            decision.ignore();
            return true;
        }
        false
    });
    for (index, engine) in crate::browser::search::SearchEngine::ALL
        .into_iter()
        .enumerate()
    {
        // Uma pesquisa pela barra normalmente sai da tela inicial. Reabre-a após a navegação interceptada.
        view.load_uri(start_page::URI);
        wait_until(|| view.uri().as_deref() == Some(start_page::URI) && !view.is_loading());
        browser.show_preferences();
        let preferences = visible_dialog("Preferências do Kairo");
        let selector = find_search_selector(&preferences.child().unwrap()).unwrap();
        selector.set_selected(index as u32);
        find_button(&preferences.child().unwrap(), "Salvar")
            .unwrap()
            .emit_clicked();
        wait_until(|| !preferences.is_visible() && !view.is_loading());
        assert_eq!(
            profile.borrow().preferences().unwrap().search_engine,
            engine
        );
        assert_eq!(
            javascript(&view, "document.getElementById('query').placeholder"),
            format!("Pesquisar com {}", engine.name())
        );
        for from_bar in [false, true] {
            search.borrow_mut().take();
            if from_bar {
                browser.address.set_text("rust & gtk café");
                browser.address.emit_by_name::<()>("activate", &[]);
            } else {
                javascript(
                    &view,
                    "document.getElementById('query').value = 'rust & gtk café'; document.querySelector('form').requestSubmit(); 'submitted'",
                );
            }
            wait_until(|| search.borrow().is_some());
            let target = url::Url::parse(search.borrow().as_ref().unwrap()).unwrap();
            let expected = engine.query("rust & gtk café");
            assert_eq!(target, expected);
        }
    }
    javascript(
        &view,
        "document.querySelector('.shortcut').click(); 'clicked'",
    );
    wait_until(|| view.uri().as_deref() == Some(uri.as_str()) && !view.is_loading());
    view.go_back();
    wait_until(|| view.uri().as_deref() == Some(start_page::URI) && !view.is_loading());
    browser.flush_session();
    window.close();
    assert_eq!(profile.borrow().session().unwrap().urls, [start_page::URI]);
    let (restored_window, restored) = build_window(app, Some(profile), None);
    let (_, restored_view) = restored.active_view().unwrap();
    wait_until(|| {
        restored_view.title().as_deref() == Some("Nova aba") && !restored_view.is_loading()
    });
    assert_eq!(
        javascript(
            &restored_view,
            "document.getElementById('query').placeholder"
        ),
        "Pesquisar com Google"
    );
    restored_window.destroy();
}

fn find_search_selector(widget: &gtk4::Widget) -> Option<gtk4::DropDown> {
    if let Some(selector) = widget.downcast_ref::<gtk4::DropDown>() {
        return Some(selector.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(selector) = find_search_selector(&current) {
            return Some(selector);
        }
        child = current.next_sibling();
    }
    None
}

fn javascript(view: &webkit6::WebView, source: &str) -> String {
    let result = Rc::new(RefCell::new(None));
    let received = result.clone();
    view.evaluate_javascript(
        source,
        None,
        None,
        None::<&gio::Cancellable>,
        move |value| {
            *received.borrow_mut() = Some(value.map(|value| value.to_str().to_string()));
        },
    );
    wait_until(|| result.borrow().is_some());
    result.borrow_mut().take().unwrap().unwrap()
}

fn history_can_be_closed(parent: &gtk4::ApplicationWindow, browser: &Rc<BrowserWindow>) {
    let selected = browser.tabs.borrow().active();
    let count = browser.notebook.n_pages();
    for populated in [false, true] {
        if populated {
            browser
                .store
                .as_ref()
                .unwrap()
                .borrow()
                .record_visit("https://example.org", "Test history")
                .unwrap();
        }
        for use_escape in [false, true] {
            browser.show_history();
            let dialog = visible_dialog("Histórico do Kairo");
            let header = dialog
                .titlebar()
                .expect("Histórico precisa ter controles visíveis");
            let close =
                find_button(&header, "Voltar à navegação").expect("Botão para sair do histórico");
            if use_escape {
                let controllers = dialog.observe_controllers();
                let keys = (0..controllers.n_items())
                    .filter_map(|index| controllers.item(index))
                    .filter_map(|controller| controller.downcast::<gtk4::EventControllerKey>().ok())
                    .find(|controller| {
                        controller.propagation_phase() == gtk4::PropagationPhase::Capture
                    })
                    .unwrap();
                let handled = keys.emit_by_name::<bool>(
                    "key-pressed",
                    &[
                        &gtk4::gdk::Key::Escape,
                        &0u32,
                        &gtk4::gdk::ModifierType::empty(),
                    ],
                );
                assert!(handled);
            } else {
                close.emit_clicked();
            }
            wait_until(|| !dialog.is_visible());
            assert!(parent.is_visible());
            assert_eq!(browser.notebook.n_pages(), count);
            assert_eq!(browser.tabs.borrow().active(), selected);
            assert_eq!(
                browser
                    .store
                    .as_ref()
                    .unwrap()
                    .borrow()
                    .history()
                    .unwrap()
                    .len(),
                usize::from(populated)
            );
        }
    }
    browser.show_preferences();
    let preferences = visible_dialog("Preferências do Kairo");
    find_button(&preferences.titlebar().unwrap(), "Cancelar")
        .unwrap()
        .emit_clicked();
    wait_until(|| !preferences.is_visible());
    assert!(parent.is_visible());
    browser.show_performance();
    let performance = visible_dialog("Desempenho do Kairo");
    let button = find_button(&performance.child().unwrap(), "Medir agora").unwrap();
    button.emit_clicked();
    wait_until(|| button.is_sensitive());
    assert!(
        find_button(&performance.child().unwrap(), "Salvar relatório")
            .unwrap()
            .is_sensitive()
    );
    find_button(&performance.titlebar().unwrap(), "Voltar à navegação")
        .unwrap()
        .emit_clicked();
    wait_until(|| !performance.is_visible());
}

fn collect_smoke_baseline_if_requested(browser: &Rc<BrowserWindow>) {
    use std::io::Write;
    let args: Vec<_> = std::env::args().collect();
    let Some(pair) = args.windows(2).find(|pair| pair[0] == "--metrics-output") else {
        return;
    };
    let settled = std::time::Instant::now();
    wait_until(|| settled.elapsed() >= std::time::Duration::from_secs(1));
    let first = crate::performance::processes::snapshot(std::process::id()).unwrap();
    let interval = std::time::Instant::now();
    wait_until(|| interval.elapsed() >= std::time::Duration::from_secs(1));
    let second = crate::performance::processes::snapshot(std::process::id()).unwrap();
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&pair[1])
        .unwrap();
    file.write_all(crate::performance::processes::Snapshot::csv_header().as_bytes())
        .unwrap();
    file.write_all(
        second
            .csv_rows(
                1,
                "Kairo GTK integration debug",
                "blank-10-tabs",
                crate::performance::processes::cpu_percent(&first, &second),
            )
            .as_bytes(),
    )
    .unwrap();
    println!(
        "{}\n{}",
        second.report(crate::performance::processes::cpu_percent(&first, &second)),
        browser.metrics.borrow().report()
    );
}

fn visible_dialog(title: &str) -> gtk4::Window {
    gtk4::Window::list_toplevels()
        .into_iter()
        .filter_map(|widget| widget.downcast::<gtk4::Window>().ok())
        .find(|window| window.is_visible() && window.title().as_deref() == Some(title))
        .unwrap()
}

fn find_button(widget: &gtk4::Widget, label: &str) -> Option<gtk4::Button> {
    if let Some(button) = widget.downcast_ref::<gtk4::Button>()
        && button.label().as_deref() == Some(label)
    {
        return Some(button.clone());
    }
    let mut child = widget.first_child();
    while let Some(current) = child {
        if let Some(button) = find_button(&current, label) {
            return Some(button);
        }
        child = current.next_sibling();
    }
    None
}

fn persistence_survives_window_restart(app: &gtk4::Application) {
    let server = LocalServer::start();
    let first_uri = format!("{}/first", server.url);
    let second_uri = format!("{}/second", server.url);
    let mut profile = Store::memory().unwrap();
    profile
        .save_preferences(&Preferences {
            home_page: first_uri.clone(),
            restore_session: true,
            ..Default::default()
        })
        .unwrap();
    let profile = Rc::new(RefCell::new(profile));
    let (window, browser) = build_window(app, Some(profile.clone()), None);
    window.present();
    wait_until(|| profile.borrow().history().unwrap().len() == 1);
    save_preview_if_requested(&window);
    let first = browser.tabs.borrow().active().unwrap();
    browser.add_view(webkit6::WebView::new(), Some(&second_uri));
    wait_until(|| profile.borrow().history().unwrap().len() == 2);
    browser.select(first);
    window.close();
    let expected = Session {
        urls: vec![first_uri.clone(), second_uri],
        active: 0,
    };
    assert_eq!(profile.borrow().session().unwrap(), expected);

    let (restored_window, restored) = build_window(app, Some(profile.clone()), None);
    assert_eq!(restored.notebook.n_pages(), 2);
    assert_eq!(restored.notebook.current_page(), Some(0));
    wait_until(|| {
        restored
            .views
            .borrow()
            .values()
            .all(|tab| !tab.webview.is_loading())
    });
    restored.flush_session();
    assert_eq!(profile.borrow().session().unwrap(), expected);
    assert_eq!(restored.address.text().as_str(), first_uri);

    // Limpar o histórico não apaga a sessão; desativar restauração apaga só a sessão.
    profile.borrow().clear_history().unwrap();
    assert!(profile.borrow().history().unwrap().is_empty());
    assert_eq!(profile.borrow().session().unwrap(), expected);
    let preferences = Preferences {
        restore_session: false,
        home_page: first_uri,
        ..Default::default()
    };
    profile.borrow_mut().save_preferences(&preferences).unwrap();
    *restored.preferences.borrow_mut() = preferences;
    restored.flush_session();
    assert!(profile.borrow().session().unwrap().urls.is_empty());
    restored_window.destroy();
    let (fresh_window, fresh) = build_window(app, Some(profile), None);
    assert_eq!(fresh.notebook.n_pages(), 1);
    wait_until(|| {
        fresh
            .views
            .borrow()
            .values()
            .all(|tab| !tab.webview.is_loading())
    });
    fresh_window.destroy();
}

struct LocalServer {
    url: String,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl LocalServer {
    fn start() -> Self {
        use std::{
            io::{Read, Write},
            sync::{
                Arc,
                atomic::{AtomicBool, Ordering},
            },
        };
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let stopped = Arc::new(AtomicBool::new(false));
        let stop = stopped.clone();
        let thread = std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream
                        .set_read_timeout(Some(std::time::Duration::from_millis(200)))
                        .unwrap();
                    let mut request = [0; 4096];
                    let length = stream.read(&mut request).unwrap_or(0);
                    let request = String::from_utf8_lossy(&request[..length]);
                    let path = request.split_whitespace().nth(1).unwrap_or("/");
                    if path.starts_with("/download") {
                        let body = vec![b'K'; 128 * 1024];
                        let header = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=\"../../kairo-test.bin\"\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(header.as_bytes());
                        if path == "/download-fail" {
                            let _ = stream.write_all(b"short");
                        } else {
                            for chunk in body.chunks(4096) {
                                if stream.write_all(chunk).is_err() {
                                    break;
                                }
                                if path == "/download-slow" {
                                    std::thread::sleep(std::time::Duration::from_millis(100));
                                }
                            }
                        }
                        continue;
                    }
                    let body =
                        "<!doctype html><title>Local Kairo test</title><p>Local test page</p>";
                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = stream.write_all(response.as_bytes());
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
        });
        Self {
            url,
            stopped,
            thread: Some(thread),
        }
    }
}

impl Drop for LocalServer {
    fn drop(&mut self) {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
    }
}

#[track_caller]
fn wait_until(condition: impl Fn() -> bool) {
    let caller = std::panic::Location::caller();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let context = glib::MainContext::default();
    while !condition() {
        assert!(
            std::time::Instant::now() < deadline,
            "Tempo esgotado aguardando o WebKit em {caller}"
        );
        while context.pending() {
            context.iteration(false);
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

fn save_preview_if_requested(window: &gtk4::ApplicationWindow) {
    let args: Vec<_> = std::env::args().collect();
    let Some(pair) = args.windows(2).find(|pair| pair[0] == "--screenshot") else {
        return;
    };
    let settled = std::time::Instant::now();
    wait_until(|| settled.elapsed() >= std::time::Duration::from_millis(200));
    let paintable = gtk4::WidgetPaintable::new(Some(window));
    let snapshot = gtk4::Snapshot::new();
    paintable.snapshot(
        &snapshot,
        f64::from(window.width()),
        f64::from(window.height()),
    );
    let node = snapshot.to_node().expect("Janela renderizada");
    let renderer = gtk4::gsk::Renderer::for_surface(&window.surface().unwrap()).unwrap();
    let texture = renderer.render_texture(&node, None);
    texture.save_to_png(&pair[1]).unwrap();
    renderer.unrealize();
}
