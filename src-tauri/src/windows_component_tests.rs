use super::*;
use std::io::Cursor;

#[test]
#[ignore = "downloads the pinned 52 MiB archive from GitHub into an isolated temp directory"]
fn live_component_download_verifies_and_extracts() {
    let m = manifest();
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("component.zip");
    let mut progress = 0;
    download(
        response(&m).unwrap().into_reader(),
        fs::File::create(&archive).unwrap(),
        &m,
        &AtomicBool::new(false),
        |n| progress = n,
    )
    .unwrap();
    assert_eq!(progress, m.archive_bytes);
    let binary = temp.path().join(&m.binary_name);
    extract(&archive, &binary, &m).unwrap();
    assert!(verified_file(&binary, &m));
}

#[test]
fn concurrent_install_requests_share_one_download_and_retry_after_completion() {
    let state = std::sync::Arc::new(ComponentState::default());
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let state = state.clone();
            std::thread::spawn(move || state.start_download())
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| usize::from(t.join().unwrap()))
            .sum::<usize>(),
        1
    );
    state.cancel.store(true, Ordering::Release);
    state.downloading.store(false, Ordering::Release);
    assert!(state.start_download());
    assert!(!state.cancel.load(Ordering::Acquire));
}

fn fixture(bytes: &[u8]) -> Manifest {
    Manifest {
        version: "test".into(),
        url: "https://invalid.test/component.zip".into(),
        archive_bytes: bytes.len() as u64,
        archive_sha256: format!("{:x}", Sha256::digest(bytes)),
        binary_name: "component.exe".into(),
        binary_bytes: bytes.len() as u64,
        binary_sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}
#[test]
fn interrupted_or_corrupt_downloads_never_pass_verification_and_can_retry() {
    let data = b"fixed component archive";
    let m = fixture(data);
    let cancel = AtomicBool::new(false);
    for bad in [
        b"truncated".as_slice(),
        b"wrong component archive".as_slice(),
    ] {
        assert!(download(Cursor::new(bad), Vec::new(), &m, &cancel, |_| {}).is_err());
    }
    let mut output = Vec::new();
    let mut progress = 0;
    download(Cursor::new(data), &mut output, &m, &cancel, |n| {
        progress = n
    })
    .unwrap();
    assert_eq!(output, data);
    assert_eq!(progress, data.len() as u64);
}
#[test]
fn cancelled_and_oversized_downloads_stop_before_installation() {
    let data = vec![42; 200_000];
    let m = fixture(&data);
    let cancel = AtomicBool::new(false);
    let mut output = Vec::new();
    assert_eq!(
        download(Cursor::new(&data), &mut output, &m, &cancel, |_| cancel
            .store(true, Ordering::Release)),
        Err("component_cancelled".into())
    );
    assert!(output.len() < data.len());
    cancel.store(false, Ordering::Release);
    assert_eq!(
        download(
            Cursor::new(b"too long"),
            Vec::new(),
            &fixture(b"tiny"),
            &cancel,
            |_| {}
        ),
        Err("component_size".into())
    );
}
#[test]
fn network_and_disk_errors_are_reported() {
    struct Broken;
    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::TimedOut.into())
        }
    }
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::ErrorKind::StorageFull.into())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let cancel = AtomicBool::new(false);
    assert_eq!(
        download(Broken, Vec::new(), &fixture(b"data"), &cancel, |_| {}),
        Err("component_network".into())
    );
    assert_eq!(
        download(
            Cursor::new(b"data"),
            Broken,
            &fixture(b"data"),
            &cancel,
            |_| {}
        ),
        Err("component_storage".into())
    );
}
#[test]
fn extracts_only_the_expected_binary_and_checks_it_again() {
    let temp = tempfile::tempdir().unwrap();
    let archive = temp.path().join("archive.zip");
    let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
    let options = zip::write::SimpleFileOptions::default();
    zip.start_file("../escaped.exe", options).unwrap();
    zip.write_all(b"untrusted extra file").unwrap();
    zip.start_file("component.exe", options).unwrap();
    zip.write_all(b"verified executable").unwrap();
    zip.finish().unwrap();
    let binary = temp.path().join("installed.exe");
    let m = fixture(b"verified executable");
    extract(&archive, &binary, &m).unwrap();
    assert!(verified_file(&binary, &m));
    assert!(!temp.path().parent().unwrap().join("escaped.exe").exists());
    fs::write(&binary, b"modified executable").unwrap();
    assert!(!verified_file(&binary, &m));
    assert!(
        extract(&archive, &binary, &m).is_err(),
        "cannot overwrite a destination"
    );
}
#[test]
fn staging_is_removed_after_failure_or_cancel() {
    let temp = tempfile::tempdir().unwrap();
    let staging = temp.path().join("own-staging");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("partial.zip"), b"partial").unwrap();
    drop(Staging(staging.clone()));
    assert!(!staging.exists());
}
#[test]
fn published_manifest_is_fixed_and_complete() {
    let m = manifest();
    assert!(m
        .url
        .starts_with("https://github.com/sbroenne/mcp-windows/releases/download/v"));
    assert!(m.url.contains(&format!("/v{}/", m.version)));
    assert_eq!(m.archive_sha256.len(), 64);
    assert_eq!(m.binary_sha256.len(), 64);
    assert_eq!(m.binary_name, "Sbroenne.WindowsMcp.exe");
    assert!(m.archive_bytes > 0 && m.binary_bytes > 0);
}
#[test]
#[ignore = "requires the pinned upstream archive in YUME_COMPONENT_TEST_ARCHIVE"]
fn real_component_archive_passes_both_pinned_checksums() {
    let path = std::env::var("YUME_COMPONENT_TEST_ARCHIVE").unwrap();
    let m = manifest();
    let cancel = AtomicBool::new(false);
    download(
        fs::File::open(&path).unwrap(),
        std::io::sink(),
        &m,
        &cancel,
        |_| {},
    )
    .unwrap();
    let temp = tempfile::tempdir().unwrap();
    extract(Path::new(&path), &temp.path().join(&m.binary_name), &m).unwrap();
}
