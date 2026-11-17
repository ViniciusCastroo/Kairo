#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TabId(u64);

pub struct Tab {
    pub id: TabId,
    pub uri: String,
    pub title: String,
    pub error: Option<String>,
}

#[derive(Default)]
pub struct Tabs {
    items: Vec<Tab>,
    active: Option<TabId>,
    next_id: u64,
}

impl Tabs {
    pub fn open(&mut self) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        self.items.push(Tab {
            id,
            uri: String::new(),
            title: "Nova aba".into(),
            error: None,
        });
        self.active = Some(id);
        id
    }

    pub fn active(&self) -> Option<TabId> {
        self.active
    }

    pub fn iter(&self) -> impl Iterator<Item = &Tab> {
        self.items.iter()
    }

    pub fn get(&self, id: TabId) -> Option<&Tab> {
        self.items.iter().find(|tab| tab.id == id)
    }

    pub fn get_mut(&mut self, id: TabId) -> Option<&mut Tab> {
        self.items.iter_mut().find(|tab| tab.id == id)
    }

    pub fn select(&mut self, id: TabId) {
        if self.get(id).is_some() {
            self.active = Some(id);
        }
    }

    pub fn adjacent(&self, delta: isize) -> Option<TabId> {
        let index = self
            .items
            .iter()
            .position(|tab| Some(tab.id) == self.active)?;
        let next = (index as isize + delta).rem_euclid(self.items.len() as isize) as usize;
        Some(self.items[next].id)
    }

    pub fn close(&mut self, id: TabId) {
        let Some(index) = self.items.iter().position(|tab| tab.id == id) else {
            return;
        };
        self.items.remove(index);
        if self.active == Some(id) {
            self.active = self
                .items
                .get(index.min(self.items.len().saturating_sub(1)))
                .map(|tab| tab.id);
        }
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn ten_tabs_have_distinct_ids_and_can_be_closed() {
        let mut tabs = super::Tabs::default();
        let ids: Vec<_> = (0..10).map(|_| tabs.open()).collect();
        assert_eq!(
            ids.iter().collect::<std::collections::HashSet<_>>().len(),
            10
        );
        for id in ids {
            tabs.close(id);
            assert!(tabs.get(id).is_none());
        }
        assert!(tabs.active().is_none());
        assert!(tabs.adjacent(1).is_none());
    }

    #[test]
    fn closing_active_tab_selects_neighbor_and_background_close_preserves_selection() {
        let mut tabs = super::Tabs::default();
        let first = tabs.open();
        let middle = tabs.open();
        let last = tabs.open();
        tabs.select(middle);
        tabs.close(middle);
        assert_eq!(tabs.active(), Some(last));
        tabs.close(first);
        assert_eq!(tabs.active(), Some(last));
        tabs.select(first);
        assert_eq!(tabs.active(), Some(last));
        tabs.close(last);
        assert!(tabs.active().is_none());
    }

    #[test]
    fn switching_wraps_and_new_ids_do_not_reuse_closed_tabs() {
        let mut tabs = super::Tabs::default();
        let first = tabs.open();
        let last = tabs.open();
        assert_eq!(tabs.adjacent(1), Some(first));
        tabs.select(first);
        assert_eq!(tabs.adjacent(-1), Some(last));
        tabs.close(last);
        assert_ne!(tabs.open(), last);
    }
}
