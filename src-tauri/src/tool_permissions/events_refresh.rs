use std::collections::{BTreeMap, BTreeSet, VecDeque};

const MAX_DIRTY_MESSAGES: usize = 64;
const MAX_SEEN_EVENTS: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RefreshScope {
    pub generation: u64,
    pub run_id: String,
    pub directory: String,
    pub session_id: String,
}

#[derive(Debug)]
pub(crate) struct RefreshChanges {
    pub ticket: u64,
    pub full: bool,
    pub connected: bool,
    pub message_ids: Vec<String>,
}

pub(super) struct ActiveRefresh {
    pub scope: RefreshScope,
    pub directory: String,
    pub full_at: Option<u64>,
    pub dirty: BTreeMap<String, u64>,
    seen: BTreeSet<(String, String)>,
    order: VecDeque<(String, String)>,
}

impl ActiveRefresh {
    pub fn new(scope: RefreshScope, directory: String, ticket: u64) -> Self {
        Self {
            scope,
            directory,
            full_at: Some(ticket),
            dirty: BTreeMap::new(),
            seen: BTreeSet::new(),
            order: VecDeque::new(),
        }
    }

    pub fn full(&mut self, ticket: u64) {
        self.full_at = Some(ticket);
    }

    pub fn dirty(&mut self, message: &str, ticket: u64) {
        if self.dirty.contains_key(message) || self.dirty.len() < MAX_DIRTY_MESSAGES {
            self.dirty.insert(message.to_owned(), ticket);
        } else {
            self.full(ticket);
        }
    }

    pub fn duplicate(&mut self, kind: &str, id: Option<&str>) -> bool {
        let Some(id) = id else {
            return false;
        };
        let key = (kind.to_owned(), id.to_owned());
        if !self.seen.insert(key.clone()) {
            return true;
        }
        self.order.push_back(key);
        if self.order.len() > MAX_SEEN_EVENTS {
            if let Some(expired) = self.order.pop_front() {
                self.seen.remove(&expired);
            }
        }
        false
    }

    pub fn ack(&mut self, ticket: u64) {
        self.dirty.retain(|_, at| *at > ticket);
        if self.full_at.is_some_and(|at| at <= ticket) {
            self.full_at = None;
        }
    }
}
