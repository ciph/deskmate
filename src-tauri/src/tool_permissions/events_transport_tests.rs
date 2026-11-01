use super::*;
use std::{io::Write, net::TcpListener, sync::mpsc, time::Instant};

fn request(stream: &mut std::net::TcpStream) {
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .unwrap();
    let mut line = String::new();
    let mut reader = BufReader::new(stream);
    reader.read_line(&mut line).unwrap();
    assert!(line.starts_with("GET /global/event "));
    loop {
        line.clear();
        reader.read_line(&mut line).unwrap();
        if line == "\r\n" {
            break;
        }
    }
}

#[test]
fn stalled_headers_have_a_separate_deadline_and_next_connection_recovers() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        request(&mut first);
        let hold = std::thread::spawn(move || {
            let _ = released.recv_timeout(Duration::from_secs(6));
            drop(first);
        });
        let (mut next, _) = listener.accept().unwrap();
        request(&mut next);
        write!(next, "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n").unwrap();
        hold.join().unwrap();
    });
    let events = PermissionEvents::default();
    let start = Instant::now();
    let result = events.read(&base);
    let elapsed = start.elapsed();
    let _ = release.send(());
    assert!(events.snapshot().is_err());
    let recovery = events.read(&base);
    server.join().unwrap();
    assert!(result.is_err());
    assert!(recovery.is_ok(), "{recovery:?}");
    assert!(
        elapsed < Duration::from_secs(5),
        "header wait took {elapsed:?}"
    );
}

#[test]
fn stop_interrupts_a_silent_body_and_cannot_reconnect() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (ready, started) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        request(&mut stream);
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
        ready.send(()).unwrap();
        let _ = released.recv_timeout(Duration::from_secs(4));
    });
    let events = PermissionEvents::default();
    let reader = events.clone();
    let task = std::thread::spawn(move || reader.read(&base));
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let connected_by = Instant::now() + Duration::from_secs(3);
    while !events.state.lock().unwrap().connected {
        assert!(Instant::now() < connected_by);
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(20));
    let start = Instant::now();
    events.stop();
    let _ = task.join().unwrap();
    let elapsed = start.elapsed();
    let _ = release.send(());
    server.join().unwrap();
    assert!(events.snapshot().is_err());
    assert!(elapsed < Duration::from_secs(2), "stop took {elapsed:?}");
    assert!(events.begin_connection(0).is_err(), "stop must be terminal");
}

#[test]
fn slow_split_frame_outlives_header_deadline_without_losing_bytes() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let root = tempfile::tempdir().unwrap();
    let directory = crate::agent::opencode_wire_directory(&root.path().canonicalize().unwrap());
    let frame = format!(
        "data: {}\n\n",
        serde_json::json!({
            "directory": directory, "payload": { "type": "permission.asked", "id": "evt-split",
            "properties": { "id": "p-split", "sessionID": "s-owned", "permission": "webfetch",
            "patterns": ["http://example.test"], "metadata": {"url": "http://example.test"} } }
        })
    );
    let server = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        request(&mut stream);
        write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
        let (first, second) = frame.split_at(17);
        write!(stream, "{:x}\r\n{first}\r\n", first.len()).unwrap();
        std::thread::sleep(Duration::from_secs(4));
        write!(stream, "{:x}\r\n{second}\r\n0\r\n\r\n", second.len()).unwrap();
    });
    let events = PermissionEvents {
        default_directory: directory_key(&directory),
        ..Default::default()
    };
    events.read(&base).unwrap();
    server.join().unwrap();
    assert_eq!(events.snapshot().unwrap()[0].id, "p-split");
}

#[test]
fn generation_change_interrupts_silent_connection_and_worker_reconnects() {
    use std::sync::atomic::AtomicU64;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let (ready, started) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let server = std::thread::spawn(move || {
        let (mut first, _) = listener.accept().unwrap();
        request(&mut first);
        write!(first, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n9\r\ndata: {{\"x\r\n").unwrap();
        ready.send(()).unwrap();
        listener.set_nonblocking(true).unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut next = loop {
            match listener.accept() {
                Ok((stream, _)) => break stream,
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    assert!(Instant::now() < deadline, "generation did not reconnect");
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(error) => panic!("{error}"),
            }
        };
        request(&mut next);
        write!(next, "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nTransfer-Encoding: chunked\r\n\r\n").unwrap();
        let _ = released.recv_timeout(Duration::from_secs(5));
    });
    let generation = Arc::new(AtomicU64::new(0));
    let current = generation.clone();
    let events = PermissionEvents {
        current_generation: Arc::new(move || Ok(current.load(Ordering::SeqCst))),
        ..Default::default()
    }
    .start_worker(base);
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let connected_by = Instant::now() + Duration::from_secs(3);
    while !events.state.lock().unwrap().connected {
        assert!(Instant::now() < connected_by);
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(20));
    generation.store(1, Ordering::SeqCst);
    let deadline = Instant::now() + Duration::from_secs(4);
    loop {
        let state = events.state.lock().unwrap();
        if state.connected && state.generation == Some(1) {
            assert!(state.requests.is_empty());
            break;
        }
        drop(state);
        assert!(Instant::now() < deadline, "new generation did not connect");
        std::thread::sleep(Duration::from_millis(10));
    }
    events.stop();
    let _ = release.send(());
    server.join().unwrap();
}
