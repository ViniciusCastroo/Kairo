#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DownloadState {
    Choosing,
    Transferring,
    Complete,
    Cancelled,
    Failed(String),
}

pub struct Transfer {
    pub name: String,
    pub state: DownloadState,
    pub received: u64,
    pub total: u64,
    pub progress: f64,
}

impl Transfer {
    pub fn new() -> Self {
        Self {
            name: "Preparando download…".into(),
            state: DownloadState::Choosing,
            received: 0,
            total: 0,
            progress: 0.0,
        }
    }

    pub fn active(&self) -> bool {
        matches!(
            self.state,
            DownloadState::Choosing | DownloadState::Transferring
        )
    }

    pub fn finish(&mut self) {
        if self.state == DownloadState::Transferring {
            if self.total > 0 && self.received < self.total {
                self.state = DownloadState::Failed(
                    "Arquivo incompleto: a conexão terminou antes do esperado.".into(),
                );
                return;
            }
            self.state = DownloadState::Complete;
            self.progress = 1.0;
        }
    }

    pub fn fail(&mut self, message: String, cancelled: bool) {
        if self.active() {
            self.state = if cancelled {
                DownloadState::Cancelled
            } else {
                DownloadState::Failed(message)
            };
        }
    }

    pub fn status(&self) -> String {
        match &self.state {
            DownloadState::Choosing => "Escolha onde salvar o arquivo".into(),
            DownloadState::Transferring if self.total == 0 => {
                format!("{} recebidos", bytes(self.received))
            }
            DownloadState::Transferring => format!(
                "{} de {} · {:.0}%",
                bytes(self.received),
                bytes(self.total),
                self.progress * 100.0
            ),
            DownloadState::Complete => format!("Concluído · {}", bytes(self.received)),
            DownloadState::Cancelled => "Cancelado".into(),
            DownloadState::Failed(error) => format!("Falhou: {error}"),
        }
    }
}

impl Default for Transfer {
    fn default() -> Self {
        Self::new()
    }
}

pub fn suggested_filename(input: &str) -> String {
    let name: String = input
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .chars()
        .filter(|ch| !ch.is_control())
        .take(200)
        .collect();
    let name = name.trim();
    if name.is_empty() || matches!(name, "." | "..") {
        "download".into()
    } else if name.starts_with('.') {
        format!("download{name}")
    } else {
        name.into()
    }
}

fn bytes(value: u64) -> String {
    if value >= 1024 * 1024 {
        format!("{:.1} MiB", value as f64 / (1024.0 * 1024.0))
    } else if value >= 1024 {
        format!("{:.1} KiB", value as f64 / 1024.0)
    } else {
        format!("{value} bytes")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn terminal_errors_and_cancellation_are_not_overwritten_by_finished() {
        let mut transfer = super::Transfer::new();
        transfer.state = super::DownloadState::Transferring;
        transfer.fail("network".into(), false);
        transfer.finish();
        assert_eq!(
            transfer.state,
            super::DownloadState::Failed("network".into())
        );
        let mut transfer = super::Transfer::new();
        transfer.fail(String::new(), true);
        transfer.finish();
        assert_eq!(transfer.state, super::DownloadState::Cancelled);
    }

    #[test]
    fn server_names_cannot_choose_directories_or_inject_controls() {
        assert_eq!(super::suggested_filename("../../test.txt"), "test.txt");
        assert_eq!(
            super::suggested_filename("C:\\folder\\file.txt"),
            "file.txt"
        );
        assert_eq!(super::suggested_filename("\0\n"), "download");
        assert_eq!(super::suggested_filename(".."), "download");
        assert_eq!(super::suggested_filename(".hidden"), "download.hidden");
    }

    #[test]
    fn truncated_files_are_not_marked_complete() {
        let mut transfer = super::Transfer::new();
        transfer.state = super::DownloadState::Transferring;
        transfer.total = 100;
        transfer.received = 5;
        transfer.finish();
        assert!(matches!(transfer.state, super::DownloadState::Failed(_)));
        transfer.state = super::DownloadState::Transferring;
        transfer.received = 100;
        transfer.finish();
        assert_eq!(transfer.state, super::DownloadState::Complete);
    }
}
