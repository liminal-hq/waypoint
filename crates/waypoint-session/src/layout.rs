// Window layout rules: which tab order is legal, and a function that turns any order into the
// nearest legal one. Commands set what they mean and let `canonical_order` settle the rest.
//
// (c) Copyright 2026 Liminal HQ, Scott Morris
// SPDX-License-Identifier: Apache-2.0 OR MIT

use std::collections::HashSet;

use crate::model::{Group, GroupId, Pair, PairId, TabId, TabSnapshot, WindowState};

impl WindowState {
    pub fn index_of(&self, tab: TabId) -> Option<usize> {
        self.tabs.iter().position(|t| t.id == tab)
    }

    pub fn tab(&self, tab: TabId) -> Option<&TabSnapshot> {
        self.tabs.iter().find(|t| t.id == tab)
    }

    pub fn group(&self, group: GroupId) -> Option<&Group> {
        self.groups.iter().find(|g| g.id == group)
    }

    pub fn pair(&self, pair: PairId) -> Option<&Pair> {
        self.pairs.iter().find(|p| p.id == pair)
    }

    /// The pair a tab is a pane of.
    pub fn pair_of(&self, tab: TabId) -> Option<&Pair> {
        self.pairs.iter().find(|p| p.panes.contains(&tab))
    }

    /// The tabs that must travel together with `tab`: its pair's panes in pane order, or the tab.
    pub fn unit(&self, tab: TabId) -> Vec<TabId> {
        match self.pair_of(tab) {
            Some(pair) => pair
                .panes
                .iter()
                .copied()
                .filter(|p| self.index_of(*p).is_some())
                .collect(),
            None => vec![tab],
        }
    }

    /// The members of a group in display order.
    pub fn group_tabs(&self, group: GroupId) -> Vec<TabId> {
        self.tabs
            .iter()
            .filter(|t| t.group == Some(group))
            .map(|t| t.id)
            .collect()
    }

    /// The legal order nearest to the current one. Units (a pair's panes, or a single tab) keep
    /// their first-seen position, a group gathers at the position of its first member, and the
    /// pinned blocks move before the unpinned ones, each side keeping its relative order.
    pub fn canonical_order(&self) -> Vec<TabId> {
        let mut seen: HashSet<TabId> = HashSet::new();
        let mut blocks: Vec<(Option<GroupId>, bool, Vec<TabId>)> = Vec::new();
        for t in &self.tabs {
            if seen.contains(&t.id) {
                continue;
            }
            let unit: Vec<TabId> = self
                .unit(t.id)
                .into_iter()
                .filter(|id| seen.insert(*id))
                .collect();
            let lead = self.tab(unit[0]).unwrap_or(t);
            let group = lead.group;
            let pinned = lead.pinned;
            match group.and_then(|g| blocks.iter_mut().find(|b| b.0 == Some(g))) {
                Some(block) => block.2.extend(unit),
                None => blocks.push((group, pinned, unit)),
            }
        }
        let (pinned, unpinned): (Vec<_>, Vec<_>) = blocks.into_iter().partition(|b| b.1);
        pinned
            .into_iter()
            .chain(unpinned)
            .flat_map(|b| b.2)
            .collect()
    }

    /// Everything wrong with this window, as readable lines; empty when every invariant holds.
    pub fn violations(&self) -> Vec<String> {
        let mut out = Vec::new();
        let label = &self.label;
        let mut ids: Vec<u32> = self.tabs.iter().map(|t| t.id.0).collect();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.tabs.len() {
            out.push(format!("{label}: tab ids are not unique"));
        }
        match self.active {
            None if !self.tabs.is_empty() => out.push(format!("{label}: no active tab")),
            Some(a) if self.index_of(a).is_none() => {
                out.push(format!("{label}: the active tab is not a tab"))
            }
            _ => {}
        }
        let mut mru_seen = HashSet::new();
        for id in &self.mru {
            if self.index_of(*id).is_none() || !mru_seen.insert(*id) {
                out.push(format!(
                    "{label}: the mru list names a missing or repeated tab"
                ));
                break;
            }
        }
        let mut group_ids = HashSet::new();
        for g in &self.groups {
            if !group_ids.insert(g.id) {
                out.push(format!("{label}: group ids are not unique"));
            }
            let members: Vec<&TabSnapshot> =
                self.tabs.iter().filter(|t| t.group == Some(g.id)).collect();
            if members.is_empty() {
                out.push(format!("{label}: group {} has no tabs", g.id.0));
            } else if members.iter().any(|t| t.pinned != members[0].pinned) {
                out.push(format!(
                    "{label}: group {} mixes pinned and unpinned",
                    g.id.0
                ));
            }
        }
        for t in &self.tabs {
            if let Some(g) = t.group {
                if !group_ids.contains(&g) {
                    out.push(format!("{label}: tab {} is in a missing group", t.id.0));
                }
            }
        }
        let mut paired = HashSet::new();
        let mut pair_ids = HashSet::new();
        for p in &self.pairs {
            if !pair_ids.insert(p.id) {
                out.push(format!("{label}: pair ids are not unique"));
            }
            if p.panes.len() < 2 {
                out.push(format!("{label}: pair {} has fewer than two panes", p.id.0));
            }
            if p.sizes.len() != p.panes.len() || p.sizes.iter().sum::<u32>() != 1000 {
                out.push(format!("{label}: pair {} has bad sizes", p.id.0));
            }
            let tabs: Vec<Option<&TabSnapshot>> = p.panes.iter().map(|id| self.tab(*id)).collect();
            if tabs.iter().any(Option::is_none) {
                out.push(format!("{label}: pair {} has a missing pane", p.id.0));
                continue;
            }
            let tabs: Vec<&TabSnapshot> = tabs.into_iter().flatten().collect();
            if p.panes.iter().any(|id| !paired.insert(*id)) {
                out.push(format!("{label}: a tab is in two pairs"));
            }
            if tabs
                .iter()
                .any(|t| t.group != tabs[0].group || t.pinned != tabs[0].pinned)
            {
                out.push(format!(
                    "{label}: pair {} spans groups or mixes pinned and unpinned",
                    p.id.0
                ));
            }
            let first = self.index_of(p.panes[0]).unwrap_or(0);
            let contiguous = p
                .panes
                .iter()
                .enumerate()
                .all(|(i, id)| self.index_of(*id) == Some(first + i));
            if !contiguous {
                out.push(format!("{label}: pair {} panes are not contiguous", p.id.0));
            }
        }
        let canonical: Vec<TabId> = self.canonical_order();
        let current: Vec<TabId> = self.tabs.iter().map(|t| t.id).collect();
        if canonical != current {
            out.push(format!(
                "{label}: tabs are not in a legal order (groups contiguous, pinned first)"
            ));
        }
        out
    }
}
