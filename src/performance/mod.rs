use std::{collections::HashMap, sync::OnceLock, time::Instant};

pub mod processes;

static STARTED: OnceLock<Instant> = OnceLock::new();

pub fn start_clock() {
    STARTED.get_or_init(Instant::now);
}

#[derive(Default)]
pub struct NavigationMetrics {
    pending: HashMap<crate::browser::tabs::TabId, Instant>,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
    pub last_load_ms: Option<f64>,
    pub window_map_ms: Option<f64>,
}

impl NavigationMetrics {
    pub fn window_mapped(&mut self) {
        if self.window_map_ms.is_none() {
            self.window_map_ms = STARTED
                .get()
                .map(|started| started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    pub fn begin(&mut self, id: crate::browser::tabs::TabId) {
        self.pending.insert(id, Instant::now());
    }

    pub fn complete(&mut self, id: crate::browser::tabs::TabId) {
        if let Some(started) = self.pending.remove(&id) {
            self.completed += 1;
            self.last_load_ms = Some(started.elapsed().as_secs_f64() * 1000.0);
        }
    }

    pub fn fail(&mut self, id: crate::browser::tabs::TabId, cancelled: bool) {
        if self.pending.remove(&id).is_some() {
            if cancelled {
                self.cancelled += 1;
            } else {
                self.failed += 1;
            }
        }
    }

    pub fn forget(&mut self, id: crate::browser::tabs::TabId) {
        self.pending.remove(&id);
    }

    pub fn report(&self) -> String {
        let timing = |value: Option<f64>| {
            value.map_or_else(|| "não disponível".into(), |ms| format!("{ms:.1} ms"))
        };
        format!(
            "Até mapear a janela: {}\nÚltimo carregamento concluído: {}\nCarregamentos concluídos: {}\nFalhas de carregamento: {}\nCancelamentos: {}",
            timing(self.window_map_ms),
            timing(self.last_load_ms),
            self.completed,
            self.failed,
            self.cancelled
        )
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn navigation_failures_are_counted_once_and_cancel_is_separate() {
        let mut tabs = crate::browser::tabs::Tabs::default();
        let id = tabs.open();
        let mut metrics = super::NavigationMetrics::default();
        metrics.begin(id);
        metrics.fail(id, false);
        metrics.fail(id, false);
        metrics.complete(id);
        assert_eq!(metrics.failed, 1);
        assert_eq!(metrics.completed, 0);
        metrics.begin(id);
        metrics.fail(id, true);
        assert_eq!(metrics.cancelled, 1);
        metrics.begin(id);
        metrics.complete(id);
        assert_eq!(metrics.completed, 1);
        assert!(metrics.last_load_ms.is_some());
        metrics.begin(id);
        metrics.forget(id);
        metrics.complete(id);
        assert_eq!(metrics.completed, 1);
    }
}
