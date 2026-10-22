//! Bounded, sanitized sidecar diagnostics. A single shared sink survives process
//! restarts, so stdout/stderr and a retiring process cannot race log rotation.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

const FILE_LIMIT: usize = 64 * 1024;
const LINE_LIMIT: usize = 8 * 1024;
const BACKUPS: usize = 3;
const REDACTED: &str = "[redacted]";
const SENSITIVE_LINE: &str = "[redacted sensitive log line]";
const OVERSIZED_LINE: &str = "[discarded log line exceeding 8192 bytes]";

pub(crate) fn configure(command: &mut Command) {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
}

/// Collect values, never print parse errors (which could include credentials).
pub(crate) fn collect_json_secrets(json: &str, secrets: &mut Vec<String>) {
    fn visit(value: &serde_json::Value, secrets: &mut Vec<String>) {
        match value {
            serde_json::Value::String(value) => secrets.push(value.clone()),
            serde_json::Value::Array(values) => {
                for value in values {
                    visit(value, secrets);
                }
            }
            serde_json::Value::Object(values) => {
                for value in values.values() {
                    visit(value, secrets);
                }
            }
            _ => {}
        }
    }
    match serde_json::from_str(json) {
        Ok(value) => visit(&value, secrets),
        Err(_) => secrets.push(json.to_owned()),
    }
}

struct Redactor {
    secrets: Vec<String>,
}

impl Redactor {
    fn new(secrets: &[String]) -> Self {
        let mut values = Vec::new();
        for secret in secrets.iter().filter(|value| !value.is_empty()) {
            values.push(secret.clone());
            // Credential values can contain newlines or be JSON-escaped in a log.
            // Redact each nonempty line as well as the encoded representation.
            values.extend(
                secret
                    .lines()
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned),
            );
            if let Ok(encoded) = serde_json::to_string(secret) {
                values.push(encoded[1..encoded.len() - 1].to_owned());
            }
        }
        values.sort_unstable_by(|left, right| right.len().cmp(&left.len()).then(left.cmp(right)));
        values.dedup();
        Self { secrets: values }
    }

    fn line(&self, bytes: &[u8]) -> String {
        let line = String::from_utf8_lossy(bytes);
        let lower = line.to_ascii_lowercase();
        // Err on the side of dropping a diagnostic when it names a credential.
        // This covers JSON, headers, environment dumps and command-line flags.
        if [
            "authorization",
            "password",
            "passwd",
            "token",
            "api_key",
            "api-key",
            "apikey",
            "secret",
        ]
        .iter()
        .any(|marker| lower.contains(marker))
        {
            return SENSITIVE_LINE.to_owned();
        }
        let mut remaining = line.as_ref();
        let mut sanitized = String::with_capacity(line.len());
        while !remaining.is_empty() {
            let matched = self
                .secrets
                .iter()
                .filter_map(|secret| remaining.find(secret).map(|index| (index, secret.len())))
                .min_by(|left, right| left.0.cmp(&right.0).then(right.1.cmp(&left.1)));
            let Some((index, length)) = matched else {
                sanitized.push_str(remaining);
                break;
            };
            sanitized.push_str(&remaining[..index]);
            sanitized.push_str(REDACTED);
            if sanitized.len() > LINE_LIMIT {
                return SENSITIVE_LINE.to_owned();
            }
            remaining = &remaining[index + length..];
        }
        if sanitized.len() > LINE_LIMIT {
            SENSITIVE_LINE.to_owned()
        } else {
            sanitized
        }
    }
}

/// A failed sink still drains the pipe. Stopping reads would eventually block
/// the child on a full stdout/stderr pipe and turn a disk error into a hang.
fn drain_lines(
    mut reader: impl Read,
    redactor: &Redactor,
    mut emit: impl FnMut(&str) -> io::Result<()>,
) -> io::Result<()> {
    let mut chunk = [0u8; 2048];
    let mut line = Vec::with_capacity(LINE_LIMIT);
    let mut oversized = false;
    let mut sink_error = None;
    let mut emit_line = |line: &[u8], oversized: bool| {
        if sink_error.is_none() {
            let sanitized = if oversized {
                OVERSIZED_LINE.to_owned()
            } else {
                redactor.line(line)
            };
            if let Err(error) = emit(&sanitized) {
                sink_error = Some(error);
            }
        }
    };
    loop {
        let count = match reader.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        for byte in &chunk[..count] {
            if *byte == b'\n' {
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                emit_line(&line, oversized);
                line.clear();
                oversized = false;
            } else if !oversized {
                if line.len() < LINE_LIMIT {
                    line.push(*byte);
                } else {
                    line.clear();
                    oversized = true;
                }
            }
        }
    }
    if oversized || !line.is_empty() {
        emit_line(&line, oversized);
    }
    match sink_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn backup_path(path: &Path, number: usize) -> PathBuf {
    if number == 0 {
        return path.to_owned();
    }
    let mut name = path.as_os_str().to_os_string();
    name.push(format!(".{number}"));
    PathBuf::from(name)
}

fn scrub_existing(path: &Path, redactor: &Redactor) -> io::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error),
    };
    if !metadata.file_type().is_file() {
        return Err(io::Error::other("sidecar log path must be a regular file"));
    }
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let offset = metadata.len().saturating_sub(FILE_LIMIT as u64);
    file.seek(SeekFrom::Start(offset))?;
    let mut tail = Vec::with_capacity(FILE_LIMIT);
    (&mut file).take(FILE_LIMIT as u64).read_to_end(&mut tail)?;
    let start = if offset == 0 {
        0
    } else {
        // The first tail fragment could begin halfway through a credential.
        tail.iter()
            .position(|byte| *byte == b'\n')
            .map_or(tail.len(), |index| index + 1)
    };
    let mut records = VecDeque::new();
    let mut size = 0;
    drain_lines(&tail[start..], redactor, |line| {
        let record = format!("{line}\n").into_bytes();
        while size + record.len() > FILE_LIMIT {
            if let Some(previous) = records.pop_front() {
                let previous: Vec<u8> = previous;
                size -= previous.len();
            }
        }
        size += record.len();
        records.push_back(record);
        Ok(())
    })?;
    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    for record in records {
        file.write_all(&record)?;
    }
    Ok(())
}

struct BoundedLog {
    path: PathBuf,
    file: Option<File>,
    size: usize,
}

impl BoundedLog {
    fn rotate(&mut self) -> io::Result<()> {
        // Windows cannot rename the file while our writer still holds it open.
        self.file.take();
        match fs::remove_file(backup_path(&self.path, BACKUPS)) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        for number in (1..=BACKUPS).rev() {
            match fs::rename(
                backup_path(&self.path, number - 1),
                backup_path(&self.path, number),
            ) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        self.file = Some(
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?,
        );
        self.size = 0;
        Ok(())
    }

    fn write(&mut self, record: &[u8]) -> io::Result<()> {
        if record.len() > FILE_LIMIT {
            return Err(io::Error::other("sidecar log record exceeds limit"));
        }
        if self.size + record.len() > FILE_LIMIT {
            self.rotate()?;
        }
        let result = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::other("sidecar log sink unavailable"))?
            .write_all(record);
        if let Err(error) = result {
            // A partial write makes the tracked length uncertain. Stop both
            // writers rather than allowing later records to exceed the cap.
            self.file.take();
            return Err(error);
        }
        self.size += record.len();
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct SidecarLogs(Arc<Mutex<BoundedLog>>);

impl SidecarLogs {
    /// Open once per host and reuse clones across child restarts. Existing legacy
    /// logs are read with a 64 KiB cap, sanitized, then shrunk before appending.
    pub(crate) fn open(path: impl AsRef<Path>, known_secrets: &[String]) -> io::Result<Self> {
        let path = path.as_ref();
        let redactor = Redactor::new(known_secrets);
        for number in 0..=BACKUPS {
            scrub_existing(&backup_path(path, number), &redactor)?;
        }
        let file = OpenOptions::new().create(true).append(true).open(path)?;
        let size = file.metadata()?.len() as usize;
        Ok(Self(Arc::new(Mutex::new(BoundedLog {
            path: path.to_owned(),
            file: Some(file),
            size,
        }))))
    }

    fn write_line(&self, stream: &str, line: &str) -> io::Result<()> {
        let record = format!("[{stream}] {line}\n");
        self.0
            .lock()
            .map_err(|_| io::Error::other("sidecar log lock poisoned"))?
            .write(record.as_bytes())
    }

    /// Host diagnostics should use fixed event names and numeric error kinds,
    /// not error strings or command dumps. This still applies the same bounds
    /// and redaction in case a caller accidentally includes sensitive content.
    pub(crate) fn record(&self, line: &str, known_secrets: &[String]) -> io::Result<()> {
        drain_lines(line.as_bytes(), &Redactor::new(known_secrets), |line| {
            self.write_line("host", line)
        })
    }

    /// On error the caller stops its exact owned Child. A reader already started
    /// here will drain until that child closes its pipe; no reader owns a process.
    pub(crate) fn attach(&self, child: &mut Child, known_secrets: Vec<String>) -> io::Result<()> {
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("sidecar stdout pipe missing"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("sidecar stderr pipe missing"))?;
        self.write_line("host", "sidecar launch")?;
        let redactor = Arc::new(Redactor::new(&known_secrets));
        self.start_reader(stdout, "stdout", redactor.clone())?;
        self.start_reader(stderr, "stderr", redactor)?;
        Ok(())
    }

    fn start_reader(
        &self,
        reader: impl Read + Send + 'static,
        stream: &'static str,
        redactor: Arc<Redactor>,
    ) -> io::Result<()> {
        let sink = self.clone();
        std::thread::Builder::new()
            .name(format!("sidecar-{stream}"))
            .spawn(move || {
                if drain_lines(reader, &redactor, |line| sink.write_line(stream, line)).is_err() {
                    // Do not include raw I/O errors or child output in host logs.
                    eprintln!("sidecar diagnostic capture unavailable");
                }
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct Chunks<R> {
        reader: R,
        size: usize,
    }
    impl<R: Read> Read for Chunks<R> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let limit = bytes.len().min(self.size);
            self.reader.read(&mut bytes[..limit])
        }
    }

    fn capture(input: &[u8], secrets: &[&str], chunk: usize) -> String {
        let secrets = secrets
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let mut output = String::new();
        drain_lines(
            Chunks {
                reader: input,
                size: chunk,
            },
            &Redactor::new(&secrets),
            |line| {
                output.push_str(line);
                output.push('\n');
                Ok(())
            },
        )
        .unwrap();
        output
    }

    #[test]
    fn credentials_split_at_every_byte_and_utf8_are_redacted_before_emission() {
        let input = "故障 before sk-live-ABC123 after\n";
        assert_eq!(
            capture(input.as_bytes(), &["sk-live-ABC123"], 1),
            "故障 before [redacted] after\n"
        );
    }

    #[test]
    fn sensitive_fields_are_redacted_case_insensitively_even_without_known_values() {
        for line in [
            "Authorization: Basic abc",
            "PASSWORD=abc",
            "{\"api_key\":\"abc\"}",
            "--token abc",
            "client_secret: abc",
        ] {
            assert_eq!(
                capture(line.as_bytes(), &[], 2),
                format!("{SENSITIVE_LINE}\n")
            );
        }
        assert_eq!(
            capture(b"connection refused: port 17891\n", &[], 1),
            "connection refused: port 17891\n"
        );
    }

    #[test]
    fn oversized_lines_are_discarded_until_newline_without_leaking_a_prefix() {
        let mut input = vec![b'x'; LINE_LIMIT - 4];
        input.extend_from_slice(b"sk-live-ABC123");
        input.extend_from_slice(&vec![b'y'; LINE_LIMIT * 20]);
        input.extend_from_slice(b"\nnext diagnostic\n");
        assert_eq!(
            capture(&input, &["sk-live-ABC123"], 7),
            format!("{OVERSIZED_LINE}\nnext diagnostic\n")
        );
    }

    #[test]
    fn eof_without_newline_redacts_and_oversized_eof_is_only_a_marker() {
        assert_eq!(
            capture(b"failure sk-live-ABC123", &["sk-live-ABC123"], 3),
            "failure [redacted]\n"
        );
        assert_eq!(
            capture(&vec![b'x'; LINE_LIMIT + 1], &[], 5),
            format!("{OVERSIZED_LINE}\n")
        );
    }

    #[test]
    fn escaped_and_multiline_values_and_overlapping_secrets_are_removed() {
        assert_eq!(
            capture(b"abXYZ abcXYZ\n", &["ab", "abc"], 2),
            "[redacted]XYZ [redacted]XYZ\n"
        );
        assert_eq!(
            capture(b"aaa\nbbb\naaa\\nbbb\n", &["aaa\nbbb"], 1),
            "[redacted]\n[redacted]\n[redacted]\n"
        );
        assert_eq!(capture(b"safe\n", &[""], 1), "safe\n");
    }

    #[test]
    fn huge_redaction_expansion_remains_bounded() {
        assert_eq!(
            capture(&vec![b'x'; LINE_LIMIT], &["x"], 8),
            format!("{SENSITIVE_LINE}\n")
        );
    }

    #[test]
    fn json_credential_values_are_collected_recursively() {
        let mut secrets = Vec::new();
        collect_json_secrets(
            r#"{"provider":{"key":"value-a","nested":["value-b",7,false]},"other":null}"#,
            &mut secrets,
        );
        secrets.sort();
        assert_eq!(secrets, ["value-a", "value-b"]);
        collect_json_secrets("malformed-credential", &mut secrets);
        assert!(secrets.iter().any(|value| value == "malformed-credential"));
    }

    #[test]
    fn sink_failure_does_not_stop_reading_the_child_pipe() {
        struct Counted {
            reader: Cursor<Vec<u8>>,
            count: Arc<AtomicUsize>,
        }
        impl Read for Counted {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                let count = self.reader.read(bytes)?;
                self.count.fetch_add(count, Ordering::SeqCst);
                Ok(count)
            }
        }
        let input = "diagnostic\n".repeat(20_000).into_bytes();
        let length = input.len();
        let count = Arc::new(AtomicUsize::new(0));
        let result = drain_lines(
            Counted {
                reader: Cursor::new(input),
                count: count.clone(),
            },
            &Redactor::new(&[]),
            |_| Err(io::Error::other("disk failed")),
        );
        assert!(result.is_err());
        assert_eq!(count.load(Ordering::SeqCst), length);
    }

    #[test]
    fn bad_paths_return_errors_instead_of_falling_back_to_raw_output() {
        let directory = tempfile::tempdir().unwrap();
        assert!(SidecarLogs::open(directory.path(), &[]).is_err());
        assert!(
            SidecarLogs::open(directory.path().join("missing").join("sidecar.log"), &[]).is_err()
        );
    }

    #[test]
    fn first_fault_survives_multiple_launches_and_a_host_reopen() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sidecar.log");
        let sink = SidecarLogs::open(&path, &[]).unwrap();
        sink.write_line("stderr", "first connection fault").unwrap();
        sink.clone().write_line("host", "sidecar launch").unwrap();
        sink.clone().write_line("stderr", "second fault").unwrap();
        drop(sink);
        let reopened = SidecarLogs::open(&path, &[]).unwrap();
        reopened.write_line("host", "sidecar launch").unwrap();
        let output = fs::read_to_string(path).unwrap();
        assert!(output.contains("first connection fault"));
        assert!(output.contains("second fault"));
        assert_eq!(output.matches("sidecar launch").count(), 2);
    }

    #[test]
    fn rotation_keeps_only_four_files_with_each_file_within_64_kib() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sidecar.log");
        let sink = SidecarLogs::open(&path, &[]).unwrap();
        let diagnostic = "x".repeat(LINE_LIMIT);
        for index in 0..100 {
            sink.write_line("stderr", &format!("{index} {diagnostic}"))
                .unwrap();
        }
        let mut total = 0;
        for number in 0..=BACKUPS {
            let size = fs::metadata(backup_path(&path, number)).unwrap().len();
            assert!(size <= FILE_LIMIT as u64);
            total += size;
        }
        assert!(total <= (FILE_LIMIT * 4) as u64);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 4);
        assert!(fs::read_to_string(path).unwrap().contains("99 "));
    }

    #[test]
    fn stdout_and_stderr_share_atomic_records_and_rotation_lock() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sidecar.log");
        let sink = SidecarLogs::open(&path, &[]).unwrap();
        let mut workers = Vec::new();
        for stream in ["stdout", "stderr"] {
            let sink = sink.clone();
            workers.push(std::thread::spawn(move || {
                for index in 0..200 {
                    sink.write_line(stream, &format!("entry-{index} {}", "x".repeat(200)))
                        .unwrap();
                }
            }));
        }
        for worker in workers {
            worker.join().unwrap();
        }
        let output = fs::read_to_string(&path).unwrap()
            + &fs::read_to_string(backup_path(&path, 1)).unwrap();
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);
        assert_eq!(output.lines().count(), 400);
        for stream in ["stdout", "stderr"] {
            for index in 0..200 {
                assert_eq!(
                    output
                        .lines()
                        .filter(
                            |line| *line == format!("[{stream}] entry-{index} {}", "x".repeat(200))
                        )
                        .count(),
                    1
                );
            }
        }
    }

    #[test]
    fn host_events_share_the_same_redaction_and_line_limits() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sidecar.log");
        let sink = SidecarLogs::open(&path, &[]).unwrap();
        sink.record("spawn_failed kind=NotFound os=2", &[]).unwrap();
        sink.record("failure sk-live-ABC123", &["sk-live-ABC123".into()])
            .unwrap();
        sink.record(&"x".repeat(LINE_LIMIT + 1), &[]).unwrap();
        let output = fs::read_to_string(path).unwrap();
        assert!(output.contains("spawn_failed kind=NotFound os=2"));
        assert!(output.contains("failure [redacted]"));
        assert!(output.contains(OVERSIZED_LINE));
        assert!(!output.contains("sk-live-ABC123"));
    }

    #[test]
    fn oversized_legacy_current_and_backups_are_bounded_and_scrubbed() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("sidecar.log");
        let legacy = format!(
            "{}\nold failure sk-live-ABC123\nAuthorization: Basic abc\nlast useful fault\n",
            "x".repeat(FILE_LIMIT * 3)
        );
        for number in 0..=BACKUPS {
            fs::write(backup_path(&path, number), &legacy).unwrap();
        }
        let _sink = SidecarLogs::open(&path, &["sk-live-ABC123".into()]).unwrap();
        for number in 0..=BACKUPS {
            let output = fs::read_to_string(backup_path(&path, number)).unwrap();
            assert!(output.len() <= FILE_LIMIT);
            assert!(!output.contains("sk-live-ABC123"));
            assert!(!output.contains("Basic abc"));
            assert!(output.contains("old failure [redacted]"));
            assert!(output.contains("last useful fault"));
        }
    }
}
