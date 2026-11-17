use super::*;
use crate::performance::processes::{cpu_percent, snapshot};

impl BrowserWindow {
    pub(super) fn show_performance(self: &Rc<Self>) {
        let Some(parent) = self.window.upgrade() else {
            return;
        };
        let dialog = gtk4::Window::builder()
            .title("Desempenho do Kairo")
            .transient_for(&parent)
            .destroy_with_parent(true)
            .default_width(700)
            .default_height(520)
            .build();
        data::dialog_controls(&dialog, "Voltar à navegação");
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 12);
        content.set_margin_top(12);
        content.set_margin_bottom(12);
        content.set_margin_start(12);
        content.set_margin_end(12);
        content.append(
            &gtk4::Label::builder()
                .label("Medição local do Kairo e dos processos filhos. Nenhum dado é enviado.")
                .wrap(true)
                .xalign(0.0)
                .build(),
        );
        let buttons = gtk4::Box::new(gtk4::Orientation::Horizontal, 8);
        let measure = gtk4::Button::with_label("Medir agora");
        let save = gtk4::Button::with_label("Salvar relatório");
        save.set_sensitive(false);
        buttons.append(&measure);
        buttons.append(&save);
        content.append(&buttons);
        let output = gtk4::Label::builder()
            .label("Clique em Medir agora. A coleta leva cerca de um segundo.")
            .selectable(true)
            .xalign(0.0)
            .yalign(0.0)
            .build();
        output.add_css_class("monospace");
        let scroll = gtk4::ScrolledWindow::builder()
            .vexpand(true)
            .child(&output)
            .build();
        content.append(&scroll);
        let latest = Rc::new(RefCell::new(String::new()));
        let browser = self.clone();
        measure.connect_clicked(glib::clone!(#[weak] browser, #[weak] dialog, #[weak] output, #[weak] save, #[strong] latest, move |measure| {
            measure.set_sensitive(false);
            save.set_sensitive(false);
            output.set_text("Medindo memória e CPU…");
            glib::spawn_future_local(glib::clone!(#[weak] browser, #[weak] dialog, #[weak] output, #[weak] save, #[weak] measure, #[strong] latest, async move {
                let result = gio::spawn_blocking(|| {
                    let first = snapshot(std::process::id())?;
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    let second = snapshot(std::process::id())?;
                    let report = second.report(cpu_percent(&first, &second));
                    Ok::<_, std::io::Error>(report)
                }).await;
                if !dialog.is_visible() { return; }
                match result {
                    Ok(Ok(report)) => {
                        let report = format!("{}\n\n{}\n\nRSS pode contar memória compartilhada mais de uma vez. PSS depende de acesso a /proc. A coleta inclui o custo do próprio medidor.\n", report, browser.metrics.borrow().report());
                        output.set_text(&report);
                        *latest.borrow_mut() = report;
                        save.set_sensitive(true);
                    }
                    Ok(Err(error)) => output.set_text(&format!("Não foi possível medir: {error}")),
                    Err(_) => output.set_text("Não foi possível concluir a medição."),
                }
                measure.set_sensitive(true);
            }));
        }));
        save.connect_clicked(glib::clone!(
            #[weak]
            dialog,
            #[weak]
            output,
            #[strong]
            latest,
            move |_| {
                let report = latest.borrow().clone();
                glib::spawn_future_local(glib::clone!(
                    #[weak]
                    dialog,
                    #[weak]
                    output,
                    async move {
                        let chooser = gtk4::FileDialog::builder()
                            .title("Salvar relatório de desempenho")
                            .initial_name("kairo-desempenho.txt")
                            .build();
                        if let Ok(file) = chooser.save_future(Some(&dialog)).await {
                            match file
                                .path()
                                .ok_or_else(|| std::io::Error::other("Selecione um arquivo local"))
                                .and_then(|path| std::fs::write(path, report))
                            {
                                Ok(()) => (),
                                Err(error) => output.set_text(&format!(
                                    "Não foi possível salvar o relatório: {error}"
                                )),
                            }
                        }
                    }
                ));
            }
        ));
        dialog.set_child(Some(&content));
        dialog.present();
    }
}
