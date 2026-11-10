//! Sitzungsgebundene Tab-Gruppen; unabhängig von WebViews testbar.
use std::collections::{HashMap, HashSet};

const COLORS: [&str; 6] = [
    "#19d3ff", "#a78bfa", "#fbbf24", "#34d399", "#fb7185", "#60a5fa",
];

#[derive(Debug)]
pub struct Island {
    pub id: u32,
    pub name: String,
    pub color: String,
    pub collapsed: bool,
    pub private: bool,
}

#[derive(Default, Debug)]
pub struct TabIslands {
    pub islands: Vec<Island>,
    membership: HashMap<u32, u32>,
    next_id: u32,
}

impl TabIslands {
    pub fn island_for(&self, tab: u32) -> Option<u32> {
        self.membership.get(&tab).copied()
    }

    pub fn get(&self, id: u32) -> Option<&Island> {
        self.islands.iter().find(|g| g.id == id)
    }

    pub fn members(&self, id: u32) -> Vec<u32> {
        let mut ids: Vec<_> = self
            .membership
            .iter()
            .filter_map(|(&tab, &island)| (island == id).then_some(tab))
            .collect();
        ids.sort_unstable();
        ids
    }

    // Ungültige, leere oder gemischt-private Auswahlen verändern nichts.
    fn selection(ids: &[u32], tabs: &[(u32, bool)]) -> Option<(Vec<u32>, bool)> {
        let first = *ids.first()?;
        let private = tabs.iter().find(|t| t.0 == first)?.1;
        let mut seen = HashSet::new();
        let mut selected = Vec::new();
        for &id in ids {
            let tab = tabs.iter().find(|t| t.0 == id)?;
            if tab.1 != private {
                return None;
            }
            if seen.insert(id) {
                selected.push(id);
            }
        }
        Some((selected, private))
    }

    pub fn create(&mut self, ids: &[u32], tabs: &[(u32, bool)]) -> Option<u32> {
        let (selected, private) = Self::selection(ids, tabs)?;
        self.next_id = self.next_id.checked_add(1)?;
        let id = self.next_id;
        self.islands.push(Island {
            id,
            name: format!("Insel {id}"),
            color: COLORS[((id - 1) as usize) % COLORS.len()].into(),
            collapsed: false,
            private,
        });
        for tab in selected {
            self.membership.insert(tab, id);
        }
        self.clean_empty();
        Some(id)
    }

    pub fn add(&mut self, island: u32, ids: &[u32], tabs: &[(u32, bool)]) -> bool {
        let Some((selected, private)) = Self::selection(ids, tabs) else {
            return false;
        };
        if !self.get(island).is_some_and(|g| g.private == private) {
            return false;
        }
        for tab in selected {
            self.membership.insert(tab, island);
        }
        self.clean_empty();
        true
    }

    /// Ein neuer Link-Tab gehört zum tatsächlichen Quell-Tab, nie zum zufällig aktiven Tab.
    pub fn link(&mut self, source: u32, child: u32, tabs: &[(u32, bool)]) -> Option<u32> {
        let (_, private) = Self::selection(&[source, child], tabs)?;
        if source == child {
            return self.island_for(source);
        }
        if let Some(id) = self.island_for(source) {
            if self.get(id)?.private != private {
                return None;
            }
            self.add(id, &[child], tabs).then_some(id)
        } else {
            self.create(&[source, child], tabs)
        }
    }

    pub fn remove(&mut self, ids: &[u32]) {
        for id in ids {
            self.membership.remove(id);
        }
        self.clean_empty();
    }

    fn clean_empty(&mut self) {
        self.islands
            .retain(|g| self.membership.values().any(|&id| id == g.id));
    }

    pub fn dissolve(&mut self, id: u32) {
        self.membership.retain(|_, group| *group != id);
        self.clean_empty();
    }

    pub fn toggle(&mut self, id: u32) {
        if let Some(g) = self.islands.iter_mut().find(|g| g.id == id) {
            g.collapsed = !g.collapsed;
        }
    }

    pub fn expand_tab(&mut self, tab: u32) {
        if let Some(id) = self.island_for(tab) {
            if let Some(g) = self.islands.iter_mut().find(|g| g.id == id) {
                g.collapsed = false;
            }
        }
    }

    pub fn rename(&mut self, id: u32, name: &str, color: &str) {
        if let Some(g) = self.islands.iter_mut().find(|g| g.id == id) {
            let name: String = name
                .trim()
                .chars()
                .filter(|c| !c.is_control())
                .take(40)
                .collect();
            if !name.is_empty() {
                g.name = name;
            }
            if color.len() == 7
                && color.starts_with('#')
                && color[1..].bytes().all(|c| c.is_ascii_hexdigit())
            {
                g.color = color.to_ascii_lowercase();
            }
        }
    }

    /// Jede Insel bleibt zusammenhängend an der Position ihres ersten Tabs.
    /// Reihenfolge innerhalb der Insel bleibt erhalten; WebViews werden nicht neu erstellt.
    pub fn arrange<T>(&self, tabs: &mut Vec<T>, id: impl Fn(&T) -> u32) {
        let mut remaining = std::mem::take(tabs);
        while !remaining.is_empty() {
            let first = remaining.remove(0);
            let group = self.island_for(id(&first));
            tabs.push(first);
            if let Some(group) = group {
                let mut i = 0;
                while i < remaining.len() {
                    if self.island_for(id(&remaining[i])) == Some(group) {
                        tabs.push(remaining.remove(i));
                    } else {
                        i += 1;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const TABS: [(u32, bool); 5] = [(1, false), (2, false), (3, false), (4, true), (5, true)];

    #[test]
    fn manual_group_and_stable_order() {
        let mut groups = TabIslands::default();
        let id = groups.create(&[1, 3, 1], &TABS).unwrap();
        assert_eq!(groups.members(id), vec![1, 3]);
        let mut tabs = vec![1, 2, 3, 4, 5];
        groups.arrange(&mut tabs, |t| *t);
        assert_eq!(tabs, vec![1, 3, 2, 4, 5]);
    }

    #[test]
    fn linked_tabs_follow_source_island() {
        let mut groups = TabIslands::default();
        let id = groups.link(1, 2, &TABS).unwrap();
        assert_eq!(groups.link(2, 3, &TABS), Some(id));
        assert_eq!(groups.members(id), vec![1, 2, 3]);
    }

    #[test]
    fn mixed_privacy_is_never_grouped() {
        let mut groups = TabIslands::default();
        assert_eq!(groups.create(&[1, 4], &TABS), None);
        assert_eq!(groups.link(1, 4, &TABS), None);
        let id = groups.create(&[4, 5], &TABS).unwrap();
        assert!(!groups.add(id, &[1], &TABS));
        assert_eq!(groups.members(id), vec![4, 5]);
        assert!(groups.get(id).unwrap().private);
    }

    #[test]
    fn moving_and_removing_cleans_only_empty_islands() {
        let mut groups = TabIslands::default();
        let a = groups.create(&[1, 2], &TABS).unwrap();
        let b = groups.create(&[3], &TABS).unwrap();
        assert!(groups.add(b, &[1, 2], &TABS));
        assert!(groups.get(a).is_none());
        groups.remove(&[1, 2]);
        assert_eq!(groups.members(b), vec![3]);
        groups.remove(&[3]);
        assert!(groups.islands.is_empty());
    }

    #[test]
    fn collapse_expand_and_dissolve_preserve_tabs() {
        let mut groups = TabIslands::default();
        let id = groups.create(&[1, 2], &TABS).unwrap();
        groups.toggle(id);
        assert!(groups.get(id).unwrap().collapsed);
        groups.expand_tab(2);
        assert!(!groups.get(id).unwrap().collapsed);
        groups.dissolve(id);
        assert!(groups.island_for(1).is_none());
        assert!(groups.islands.is_empty());
    }

    #[test]
    fn invalid_requests_are_atomic() {
        let mut groups = TabIslands::default();
        let id = groups.create(&[1], &TABS).unwrap();
        assert_eq!(groups.create(&[], &TABS), None);
        assert_eq!(groups.create(&[2, 99], &TABS), None);
        assert!(!groups.add(id, &[2, 99], &TABS));
        assert!(!groups.add(99, &[2], &TABS));
        assert_eq!(groups.members(id), vec![1]);
        assert_eq!(groups.islands.len(), 1);
    }

    #[test]
    fn rename_limits_text_and_validates_color() {
        let mut groups = TabIslands::default();
        let id = groups.create(&[1], &TABS).unwrap();
        groups.rename(id, "  Recherche\n  ", "#ABC123");
        assert_eq!(groups.get(id).unwrap().name, "Recherche");
        assert_eq!(groups.get(id).unwrap().color, "#abc123");
        groups.rename(id, "", "red; background:url(bad)");
        assert_eq!(groups.get(id).unwrap().color, "#abc123");
        groups.rename(id, &"a".repeat(60), "#19d3ff");
        assert_eq!(groups.get(id).unwrap().name.len(), 40);
    }

    #[test]
    fn private_link_group_and_closed_source() {
        let mut groups = TabIslands::default();
        let id = groups.link(4, 5, &TABS).unwrap();
        assert!(groups.get(id).unwrap().private);
        assert_eq!(groups.link(99, 3, &TABS), None);
        assert!(groups.island_for(3).is_none());
    }

    #[test]
    fn multiple_groups_keep_relative_member_order() {
        let mut groups = TabIslands::default();
        groups.create(&[1, 3], &TABS);
        groups.create(&[4, 5], &TABS);
        let mut tabs = vec![1, 4, 2, 5, 3];
        groups.arrange(&mut tabs, |t| *t);
        assert_eq!(tabs, vec![1, 3, 4, 5, 2]);
        groups.arrange(&mut tabs, |t| *t);
        assert_eq!(tabs, vec![1, 3, 4, 5, 2]);
    }
}
