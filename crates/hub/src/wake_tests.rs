use std::collections::BTreeMap;

use mergegate_core::agent::Agent;
use mergegate_core::store::StoreError;

use super::*;

/// One item, `app#1`, whose newest event is `cursor`.
struct Store {
    worktree: String,
    cursor: u64,
    woken: BTreeMap<ItemId, u64>,
}

impl Store {
    fn new(cursor: u64) -> (tempfile::TempDir, Self) {
        let dir = tempfile::tempdir().unwrap();
        let worktree = dir.path().display().to_string();
        let store = Self {
            worktree,
            cursor,
            woken: BTreeMap::new(),
        };
        (dir, store)
    }
}

impl AgentStore for Store {
    fn agent(&self, _: &ItemId) -> Result<Agent, StoreError> {
        unreachable!()
    }

    fn set_agent(&mut self, _: &ItemId, _: &Agent) -> Result<(), StoreError> {
        unreachable!()
    }

    fn due_wakes(&self) -> Result<Vec<DueWake>, StoreError> {
        let item = id();
        let woken = self.woken.get(&item).copied().unwrap_or(0);
        Ok((self.cursor > woken)
            .then(|| DueWake {
                item,
                command: "resume".into(),
                worktree: Some(self.worktree.clone()),
                cursor: self.cursor,
            })
            .into_iter()
            .collect())
    }

    fn set_woken(&mut self, id: &ItemId, cursor: u64) -> Result<(), StoreError> {
        self.woken.insert(id.clone(), cursor);
        Ok(())
    }
}

fn id() -> ItemId {
    ItemId::parse("app#1").unwrap()
}

fn started(store: &mut Store, wakes: &Wakes) -> Vec<u64> {
    take(store, wakes)
        .unwrap()
        .into_iter()
        .map(|w| w.cursor)
        .collect()
}

#[test]
fn wakes_once_per_event() {
    let (_dir, mut store) = Store::new(3);
    let wakes = Wakes::default();
    assert_eq!(started(&mut store, &wakes), [3]);
    wakes.lock().running.clear();
    assert_eq!(started(&mut store, &wakes), [] as [u64; 0]);
    store.cursor = 4;
    assert_eq!(started(&mut store, &wakes), [4]);
}

#[test]
fn waits_out_a_read_and_skips_what_it_returned() {
    let (_dir, mut store) = Store::new(3);
    let wakes = Wakes::default();
    let reading = wakes.reading(&id());
    assert_eq!(started(&mut store, &wakes), [] as [u64; 0]);
    assert_eq!(store.woken.get(&id()), None, "the read may stop short");
    wakes.handed(&id(), 3);
    drop(reading);
    assert_eq!(started(&mut store, &wakes), [] as [u64; 0]);
    assert_eq!(store.woken.get(&id()), Some(&3));

    // An event logged after the read's last look.
    store.cursor = 4;
    let reading = wakes.reading(&id());
    wakes.handed(&id(), 3);
    drop(reading);
    assert_eq!(started(&mut store, &wakes), [4]);
}

#[test]
fn runs_one_command_per_item_at_a_time() {
    let (_dir, mut store) = Store::new(3);
    let wakes = Wakes::default();
    assert_eq!(started(&mut store, &wakes), [3]);
    store.cursor = 4;
    assert_eq!(started(&mut store, &wakes), [] as [u64; 0]);
    assert_eq!(store.woken.get(&id()), Some(&3), "4 waits for it");
    wakes.lock().running.remove(&id());
    assert_eq!(started(&mut store, &wakes), [4]);
}

#[test]
fn a_gone_worktree_is_skipped_for_good() {
    let (dir, mut store) = Store::new(3);
    drop(dir);
    let wakes = Wakes::default();
    assert_eq!(started(&mut store, &wakes), [] as [u64; 0]);
    assert_eq!(store.woken.get(&id()), Some(&3));
    assert!(wakes.lock().running.is_empty());
}
