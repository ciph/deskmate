use super::WorkbenchOwnership;

#[test]
fn hidden_host_cannot_reclaim_from_still_visible_document() {
    // Given a host window hidden after owning a session.
    let ownership = WorkbenchOwnership::default();
    ownership.set_host_visible(true).unwrap();
    ownership.claim_visible("C:/one", "ses_same", true).unwrap();
    ownership.set_host_visible(false).unwrap();
    // When its stale page attempts to reclaim with an earlier visible observation.
    let result = ownership.claim_visible("C:/one", "ses_same", true);
    // Then the hidden host cannot own the session.
    assert!(result.is_err());
    assert!(!ownership.is_owned("C:/one", "ses_same").unwrap());
}

#[test]
fn actual_hidden_host_cannot_refresh_a_lease() {
    // Given a visible lease whose native window has become hidden.
    let ownership = WorkbenchOwnership::default();
    ownership.set_host_visible(true).unwrap();
    ownership.claim_visible("C:/one", "ses_same", true).unwrap();
    // When the host rejects the document's visible heartbeat.
    let refreshed = ownership.heartbeat_visible("C:/one", "ses_same", false).unwrap();
    // Then no ownership remains.
    assert!(!refreshed);
    assert!(!ownership.is_owned("C:/one", "ses_same").unwrap());
}

#[test]
fn showing_host_allows_same_session_to_be_claimed_again() {
    // Given a previously hidden workbench with its page retained.
    let ownership = WorkbenchOwnership::default();
    ownership.set_host_visible(false).unwrap();
    // When the native window is shown and its page claims the same session.
    ownership.set_host_visible(true).unwrap();
    ownership.claim_visible("C:/one", "ses_same", true).unwrap();
    // Then claim and heartbeat work normally.
    assert!(ownership.is_owned("C:/one", "ses_same").unwrap());
    assert!(ownership.heartbeat_visible("C:/one", "ses_same", true).unwrap());
}


#[test]
fn lazy_creation_is_single_flight_and_reuses_the_hidden_window() {
    use super::WorkbenchWindow;
    use std::sync::{Arc, Mutex, Barrier, atomic::{AtomicUsize, Ordering}};
    let state = Arc::new(WorkbenchWindow::default());
    let slot = Arc::new(Mutex::new(None));
    let creations = Arc::new(AtomicUsize::new(0));
    let barrier = Arc::new(Barrier::new(8));
    assert_eq!(creations.load(Ordering::SeqCst), 0);
    let threads: Vec<_> = (0..8).map(|_| {
        let (state, slot, creations, barrier) = (state.clone(), slot.clone(), creations.clone(), barrier.clone());
        std::thread::spawn(move || {
            barrier.wait();
            state.ensure(|| *slot.lock().unwrap(), || {
                creations.fetch_add(1, Ordering::SeqCst);
                *slot.lock().unwrap() = Some(7);
                Ok(7)
            }).unwrap()
        })
    }).collect();
    for thread in threads { assert_eq!(thread.join().unwrap(), 7); }
    assert_eq!(creations.load(Ordering::SeqCst), 1);
    assert_eq!(state.ensure(|| *slot.lock().unwrap(), || panic!("retained window must be reused")).unwrap(), 7);
}

#[test]
fn failed_creation_can_retry_without_consuming_saved_handoff_or_claiming_input() {
    use super::{WorkbenchWindow, WorkbenchHandoff, WorkbenchSessionTarget};
    let window = WorkbenchWindow::default();
    let handoff = WorkbenchHandoff::default();
    let ownership = WorkbenchOwnership::default();
    *handoff.0.lock().unwrap() = Some(WorkbenchSessionTarget { directory: "C:/one".into(), session_id: "ses_one".into() });
    assert!(window.ensure::<u32>(|| None, || Err("injected create failure".into())).is_err());
    assert_eq!(handoff.0.lock().unwrap().as_ref().unwrap().session_id, "ses_one");
    assert!(!ownership.is_owned("C:/one", "ses_one").unwrap());
    assert_eq!(window.ensure(|| None, || Ok(9)).unwrap(), 9);
    let latest = WorkbenchSessionTarget { directory: "C:/two".into(), session_id: "ses_one".into() };
    *handoff.0.lock().unwrap() = Some(latest);
    let ready_target = handoff.0.lock().unwrap().take().unwrap();
    assert_eq!(ready_target.directory, "C:/two");
    assert!(handoff.0.lock().unwrap().take().is_none());
}
