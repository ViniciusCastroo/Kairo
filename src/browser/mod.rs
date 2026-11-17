use url::Url;

pub mod downloads;
pub mod permissions;
pub mod search;
pub mod start_page;
pub mod tabs;
pub mod tls_error;

pub fn navigation_target(input: &str, engine: search::SearchEngine) -> Result<Url, &'static str> {
    let input = input.trim();
    if input.is_empty() {
        return Err("Digite um endereço ou uma pesquisa.");
    }
    if matches!(input, "kairo://start" | "kairo://start/") {
        return Ok(Url::parse(start_page::URI).expect("URI interna válida"));
    }
    if input.chars().any(char::is_control) {
        return Err("O endereço contém caracteres de controle.");
    }
    let authority = input.split(['/', '?', '#']).next().unwrap_or(input);
    let host_port = authority
        .rsplit_once(':')
        .is_some_and(|(host, port)| looks_like_host(host) && port.parse::<u16>().is_ok());
    let scheme = input.split_once(':').and_then(|(prefix, _)| {
        let valid = prefix.starts_with(|c: char| c.is_ascii_alphabetic())
            && prefix
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'));
        valid.then(|| prefix.to_ascii_lowercase())
    });
    let is_http = scheme
        .as_deref()
        .is_some_and(|s| matches!(s, "http" | "https"));
    if scheme.is_some() && !is_http && !host_port {
        return Err("A barra aceita apenas endereços HTTP e HTTPS.");
    }
    let target = if is_http {
        Url::parse(input)
    } else if looks_like_host(authority) || host_port {
        Url::parse(&format!("https://{input}"))
    } else {
        return Ok(engine.query(input));
    }
    .map_err(|_| "Endereço inválido. Verifique o domínio e a porta.")?;
    if target.host_str().is_none() {
        return Err("O endereço precisa ter um domínio.");
    }
    if !target.username().is_empty() || target.password().is_some() {
        return Err("Endereços com credenciais não são aceitos na barra.");
    }
    Ok(target)
}

fn looks_like_host(value: &str) -> bool {
    !value.chars().any(char::is_whitespace)
        && (value.contains('.')
            || value == "localhost"
            || value.starts_with("localhost:")
            || value.starts_with('['))
}

#[cfg(test)]
mod tests {

    #[test]
    fn resolves_addresses_and_preserves_http() {
        for (input, expected) in [
            ("kairo://start", super::start_page::URI),
            (" example.org ", "https://example.org/"),
            ("example.org:8443/a", "https://example.org:8443/a"),
            ("http://localhost:3000", "http://localhost:3000/"),
            ("localhost:3000", "https://localhost:3000/"),
            ("127.0.0.1:8000", "https://127.0.0.1:8000/"),
            ("[::1]:8000", "https://[::1]:8000/"),
        ] {
            for engine in super::search::SearchEngine::ALL {
                assert_eq!(
                    super::navigation_target(input, engine).unwrap().as_str(),
                    expected
                );
            }
        }
    }

    #[test]
    fn encodes_search_as_one_query_parameter() {
        let query = "rust & gtk #linux café";
        let target = super::navigation_target(query, Default::default()).unwrap();
        assert_eq!(target.host_str(), Some("duckduckgo.com"));
        assert_eq!(
            target.query_pairs().collect::<Vec<_>>(),
            [("q".into(), query.into())]
        );
        assert!(target.fragment().is_none());
    }

    #[test]
    fn rejects_internal_and_executable_schemes() {
        for input in [
            "javascript:alert(1)",
            "JaVaScRiPt:alert(1)",
            "data:text/html,hello",
            "file:///etc/passwd",
            "about:blank",
            "mailto:user@example.org",
            "ftp://example.org",
            "custom:payload",
            "kairo://other/",
            "kairo://start/?command=test",
        ] {
            assert!(
                super::navigation_target(input, Default::default()).is_err(),
                "{input}"
            );
        }
    }

    #[test]
    fn rejects_empty_invalid_and_misleading_addresses() {
        for input in [
            "",
            "   ",
            "https://",
            "https://example.org:wrong",
            "https://user:pass@example.org",
            "https://example.org\n/path",
        ] {
            assert!(
                super::navigation_target(input, Default::default()).is_err(),
                "{input:?}"
            );
        }
    }
}
