use kairo::{browser, performance, storage};
#[path = "../src/ui/mod.rs"]
mod ui;

fn main() {
    performance::start_clock();
    // WebKit/GTK precisam inicializar na thread principal, fora do libtest.
    let _ = ui::present_window as fn(&gtk4::Application);
    if !std::env::args().any(|arg| arg == "--run-gtk") {
        println!("Teste GTK opcional: cargo test --test gtk-tabs -- --run-gtk");
        return;
    }
    ui::tests::run();
    if std::env::args().any(|arg| arg == "--only-tls") {
        println!("GTK: certificado inválido bloqueado e mensagem de erro preservada.");
    } else if std::env::args().any(|arg| arg == "--only-fonts") {
        println!("GTK: Roboto local no GTK e na tela inicial verificada.");
    } else if !std::env::args().any(|arg| arg == "--only-privacy") {
        println!(
            "GTK: abas, sessões, favoritos, buscas, downloads, permissões, bloqueio TLS e privacidade verificados."
        );
    }
}
