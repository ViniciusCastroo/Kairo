//! Página de aviso exibida quando o certificado TLS de um site é rejeitado.
use super::start_page::escape;
use gtk4::gio::TlsCertificateFlags as Flags;

/// Código no estilo do Chromium e explicação curta para o primeiro erro relevante.
pub fn describe(errors: Flags) -> (&'static str, &'static str) {
    if errors.contains(Flags::UNKNOWN_CA) {
        (
            "ERR_CERT_AUTHORITY_INVALID",
            "O certificado não foi emitido por uma autoridade confiável. Isso é comum em equipamentos de rede e servidores internos com certificado autoassinado.",
        )
    } else if errors.contains(Flags::BAD_IDENTITY) {
        (
            "ERR_CERT_COMMON_NAME_INVALID",
            "O certificado pertence a outro endereço.",
        )
    } else if errors.intersects(Flags::EXPIRED | Flags::NOT_ACTIVATED) {
        (
            "ERR_CERT_DATE_INVALID",
            "O certificado está expirado ou ainda não é válido. Confira também a data do computador.",
        )
    } else if errors.contains(Flags::REVOKED) {
        (
            "ERR_CERT_REVOKED",
            "O certificado foi revogado pelo emissor.",
        )
    } else if errors.contains(Flags::INSECURE) {
        (
            "ERR_CERT_WEAK_SIGNATURE_ALGORITHM",
            "O certificado usa um algoritmo considerado inseguro.",
        )
    } else {
        ("ERR_CERT_INVALID", "O certificado não pôde ser validado.")
    }
}

pub fn page(host: &str, errors: Flags) -> String {
    let (code, detail) = describe(errors);
    include_str!("tls_error.html")
        .replace("{{HOST}}", &escape(host))
        .replace("{{CODE}}", code)
        .replace("{{DETAIL}}", detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_is_escaped_and_error_is_named() {
        let html = page("<b>x</b>", Flags::UNKNOWN_CA | Flags::EXPIRED);
        assert!(html.contains("&lt;b&gt;x&lt;/b&gt;"));
        assert!(!html.contains("<b>x</b>"));
        assert!(html.contains("ERR_CERT_AUTHORITY_INVALID"));
        assert!(!html.contains("{{"));
        assert_eq!(describe(Flags::EXPIRED).0, "ERR_CERT_DATE_INVALID");
    }
}
