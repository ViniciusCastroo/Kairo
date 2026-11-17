pub fn secure_page_origin(uri: &str) -> Option<String> {
    let url = url::Url::parse(uri).ok()?;
    if !url.username().is_empty() || url.password().is_some() {
        return None;
    }
    let local = url.host_str().is_some_and(|host| {
        host == "localhost"
            || host
                .trim_matches(['[', ']'])
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback())
    });
    if url.scheme() == "https" || (url.scheme() == "http" && local) {
        Some(url.origin().ascii_serialization())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn permission_prompts_require_https_or_loopback_and_never_show_credentials() {
        assert_eq!(
            super::secure_page_origin("https://example.org/path?private=value"),
            Some("https://example.org".into())
        );
        assert!(super::secure_page_origin("http://localhost:8765").is_some());
        assert!(super::secure_page_origin("http://127.0.0.1:8765").is_some());
        assert!(super::secure_page_origin("http://[::1]:8765").is_some());
        for uri in [
            "http://example.org",
            "http://192.168.1.1",
            "file:///tmp",
            "kairo://start/",
            "https://user:pass@example.org",
        ] {
            assert!(super::secure_page_origin(uri).is_none(), "{uri}");
        }
    }
}
