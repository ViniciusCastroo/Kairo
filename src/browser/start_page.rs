use super::search::SearchEngine;
use crate::storage::{Bookmark, HistoryEntry};

pub const URI: &str = "kairo://start/";

pub struct Shortcuts {
    pub title: &'static str,
    pub html: String,
}

pub fn shortcuts(
    history: &[HistoryEntry],
    bookmarks: &[Bookmark],
    engine: SearchEngine,
) -> Shortcuts {
    let mut favorites = String::new();
    let mut favorite_count = 0;
    for bookmark in bookmarks {
        let Ok(url) = url::Url::parse(&bookmark.uri) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            continue;
        }
        let Some(host) = url.host_str() else {
            continue;
        };
        favorites.push_str(&card(
            &bookmark.uri,
            &bookmark.title,
            host,
            favorite_count % 3,
        ));
        favorite_count += 1;
        if favorite_count == 12 {
            break;
        }
    }
    if !favorites.is_empty() {
        return Shortcuts {
            title: "Favoritos",
            html: favorites,
        };
    }
    let mut hosts = std::collections::HashSet::new();
    let mut shortcuts = String::new();
    let mut count = 0;
    for entry in history {
        let Ok(url) = url::Url::parse(&entry.uri) else {
            continue;
        };
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            continue;
        }
        let Some(host) = url.host_str() else {
            continue;
        };
        if !hosts.insert(host.to_owned()) {
            continue;
        }
        let title = if entry.title.trim().is_empty() {
            host
        } else {
            &entry.title
        };
        shortcuts.push_str(&card(&entry.uri, title, host, count));
        count += 1;
        if count == 6 {
            break;
        }
    }
    if count == 0 {
        let search_home = url::Url::parse(engine.homepage()).expect("URL embutida válida");
        let search_host = search_home
            .host_str()
            .unwrap_or("")
            .trim_start_matches("www.");
        for (index, (uri, title, host)) in [
            (engine.homepage(), engine.name(), search_host),
            ("https://wikipedia.org/", "Wikipedia", "wikipedia.org"),
            ("https://github.com/", "GitHub", "github.com"),
        ]
        .into_iter()
        .enumerate()
        {
            shortcuts.push_str(&card(uri, title, host, index));
        }
    }
    Shortcuts {
        title: if count == 0 {
            "Acesso rápido"
        } else {
            "Continue de onde parou"
        },
        html: shortcuts,
    }
}

pub fn render(history: &[HistoryEntry], bookmarks: &[Bookmark], engine: SearchEngine) -> String {
    let shortcuts = shortcuts(history, bookmarks, engine);
    let nonce = gtk4::glib::uuid_string_random();
    include_str!("start_page.html")
        .replace("{{SEARCH_ICON}}", &crate::icons::search_svg())
        .replace("{{SUBMIT_ICON}}", &crate::icons::forward_svg())
        .replace("{{EDIT_ICON}}", &crate::icons::edit_svg())
        .replace("{{LOGO}}", &crate::branding::white_logo_data_uri())
        .replace("{{SEARCH_ACTION}}", engine.endpoint())
        .replace("{{SEARCH_NAME}}", engine.name())
        .replace("{{NONCE}}", &nonce)
        .replace("{{CSS}}", include_str!("start_page.css"))
        .replace("{{ROBOTO_FONT}}", &crate::fonts::roboto_data_uri())
        .replace("{{SCRIPT}}", include_str!("start_page.js"))
        .replace("{{SECTION}}", shortcuts.title)
        .replace("{{SHORTCUTS}}", &shortcuts.html)
}

fn card(uri: &str, title: &str, host: &str, index: usize) -> String {
    let initial = host
        .trim_start_matches("www.")
        .chars()
        .next()
        .unwrap_or('K')
        .to_uppercase()
        .to_string();
    format!(
        "<a class=\"shortcut tone-{index}\" href=\"{}\" title=\"{}\"><span class=\"site-mark\" aria-hidden=\"true\">{}</span><span class=\"site-title\">{}</span><span class=\"site-host\">{}</span></a>",
        escape(uri),
        escape(title),
        escape(&initial),
        escape(title),
        escape(host)
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[cfg(test)]
mod tests {
    #[test]
    fn history_is_escaped_deduplicated_and_internal_urls_are_excluded() {
        let entries = [
            crate::storage::HistoryEntry {
                uri: "javascript:alert(1)".into(),
                title: "bad".into(),
                visited_at: 0,
            },
            crate::storage::HistoryEntry {
                uri: "https://example.org/?q=\"test\"&a=1".into(),
                title: "<script>alert('x')</script>".into(),
                visited_at: 0,
            },
            crate::storage::HistoryEntry {
                uri: "https://example.org/second".into(),
                title: "duplicate".into(),
                visited_at: 0,
            },
        ];
        let html = super::render(&entries, &[], Default::default());
        assert!(html.contains("&lt;script&gt;"));
        assert!(!html.contains("<script>alert"));
        assert!(!html.contains("javascript:"));
        assert!(!html.contains("duplicate"));
        assert!(html.contains("Continue de onde parou"));
    }

    #[test]
    fn empty_profile_has_working_search_and_default_shortcuts() {
        let html = super::render(&[], &[], Default::default());
        assert!(html.contains("action=\"https://duckduckgo.com/\""));
        assert!(html.contains("name=\"q\""));
        assert!(html.contains("Acesso rápido"));
        assert!(!html.contains("{{"));
        assert!(html.contains("connect-src 'none'"));
        let google = super::render(&[], &[], crate::browser::search::SearchEngine::Google);
        assert!(google.contains("action=\"https://www.google.com/search\""));
        assert!(google.contains("Pesquisar com Google"));
        assert!(
            google.contains("form-action https://duckduckgo.com/ https://www.google.com/search;")
        );
    }

    #[test]
    fn favorites_take_priority_and_escape_user_text() {
        let bookmarks = [crate::storage::Bookmark {
            id: 1,
            title: "<img src=x onerror=alert(1)> & 'name'".into(),
            uri: "https://example.org/?q=1&b=2".into(),
        }];
        let shortcuts = super::shortcuts(&[], &bookmarks, Default::default());
        assert_eq!(shortcuts.title, "Favoritos");
        assert!(shortcuts.html.contains("&lt;img"));
        assert!(!shortcuts.html.contains("<img"));
        assert!(shortcuts.html.contains("&amp;b=2"));
        assert!(!shortcuts.html.contains("wikipedia.org"));
    }
}
