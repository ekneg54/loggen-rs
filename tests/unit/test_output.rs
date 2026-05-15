use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use loggen::output::{BufferedLogWriter, FileWriter, HttpWriter, ProgressReporter, StdoutWriter};
use loggen::{LogEntry, LogWriter};

fn test_entry() -> LogEntry {
    LogEntry {
        timestamp: "12345".to_string(),
        level: "INFO".to_string(),
        message: "test message #1".to_string(),
    }
}

#[test]
fn test_stdout_writer_write() {
    let mut writer = StdoutWriter::new();
    let entry = test_entry();
    assert!(writer.write_entry(&entry).is_ok());
}

#[test]
fn test_stdout_writer_flush() {
    let mut writer = StdoutWriter::new();
    assert!(writer.flush().is_ok());
}

#[test]
fn test_stdout_writer_template_mode() {
    let mut writer = StdoutWriter { template_mode: true };
    let entry = LogEntry {
        timestamp: "12345".to_string(),
        level: "INFO".to_string(),
        message: "test message".to_string(),
    };
    assert!(writer.write_entry(&entry).is_ok());
}

#[test]
fn test_file_writer_template_mode() {
    let path = "test_template_mode.log";
    {
        let mut writer = FileWriter::new(path, true, None).unwrap();
        writer.template_mode = true;
        let entry = LogEntry {
            timestamp: "12345".to_string(),
            level: "INFO".to_string(),
            message: "template output only".to_string(),
        };
        writer.write_entry(&entry).unwrap();
        writer.flush().unwrap();
    }
    let mut content = String::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(!content.contains("[12345]"));
    assert!(content.contains("template output only"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn test_file_writer_write_and_read() {
    let path = "test_output.log";
    let entry = test_entry();

    {
        let mut writer = FileWriter::new(path, true, None).unwrap();
        writer.write_entry(&entry).unwrap();
        writer.flush().unwrap();
    }

    let mut content = String::new();
    std::fs::File::open(path)
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert!(content.contains("[12345] [INFO] test message #1"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn test_file_writer_new_creates_file() {
    let path = "test_new_file.log";
    {
        let writer = FileWriter::new(path, true, None);
        assert!(writer.is_ok());
        assert_eq!(writer.unwrap().path(), path);
    }
    assert!(std::path::Path::new(path).exists());
    std::fs::remove_file(path).unwrap();
}

// ── Buffered Writer ──

#[test]
fn test_buffered_writer_flush_on_size() {
    let inner = StdoutWriter::new();
    let mut buf = BufferedLogWriter::new(inner, 1);
    for _ in 0..5 {
        buf.write_entry(&test_entry()).unwrap();
    }
    buf.flush().unwrap();
}

#[test]
fn test_buffered_writer_flush_on_drop() {
    let inner = StdoutWriter::new();
    {
        let mut buf = BufferedLogWriter::new(inner, 8192);
        for _ in 0..3 {
            buf.write_entry(&test_entry()).unwrap();
        }
    }
}

// ── Progress Reporter ──

#[test]
fn test_progress_basic_output() {
    let mut pr = ProgressReporter::new(true, 1000, 0.0, 500);
    pr.report(0);
    pr.report(500);
    pr.report(1000);
    pr.done();
}

#[test]
fn test_progress_disabled() {
    let mut pr = ProgressReporter::new(false, 10, 1.0, 10000);
    pr.report(5);
    pr.report(10);
    pr.done();
}

#[test]
fn test_progress_auto_enable_conditions() {
    let mut pr = ProgressReporter::new(true, 150000, 1.0, 10000);
    pr.report(1000);
    pr.report(50000);
    pr.report(100000);
    pr.report(150000);
    pr.done();
}

#[test]
fn test_progress_interval_minimum() {
    let pr = ProgressReporter::new(true, 100, 1.0, 100);
    pr.done();
}

// ── File Rotation ──

#[test]
fn test_file_append_vs_truncate() {
    let path = "test_output_append.log";
    {
        let mut writer = FileWriter::new(path, true, None).unwrap();
        writer.template_mode = true;
        writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "first write".to_string(),
        }).unwrap();
        writer.flush().unwrap();
    }
    {
        let mut writer = FileWriter::new(path, false, None).unwrap();
        writer.template_mode = true;
        writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "second write".to_string(),
        }).unwrap();
        writer.flush().unwrap();
    }
    let mut content = String::new();
    std::fs::File::open(path).unwrap().read_to_string(&mut content).unwrap();
    assert!(content.contains("first write") && content.contains("second write"));
    std::fs::remove_file(path).unwrap();
}

#[test]
fn test_file_rotation_bytes() {
    let path = "test_output_rot.log";
    {
        let mut writer = FileWriter::new(path, true, Some(100)).unwrap();
        writer.template_mode = true;
        for i in 0..30 {
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: format!("entry-{}", i),
            }).unwrap();
        }
        writer.flush().unwrap();
    }
    assert!(std::path::Path::new(path).exists(), "main file should exist");
    assert!(std::path::Path::new(&format!("{}.1", path)).exists(), "rotated file should exist");
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(format!("{}.1", path));
}

#[test]
fn test_file_rotation_single_entry() {
    let path = "test_output_no_rot.log";
    {
        let mut writer = FileWriter::new(path, true, Some(1000)).unwrap();
        writer.template_mode = true;
        writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "small entry".to_string(),
        }).unwrap();
        writer.flush().unwrap();
    }
    assert!(!std::path::Path::new(&format!("{}.1", path)).exists(),
        "rotated file should NOT exist for small entries");
    std::fs::remove_file(path).unwrap();
}

// ── HttpWriter connection failure (no server) ──

#[test]
fn test_http_writer_connection_refused() {
    let mut writer = HttpWriter::new(
        "http://localhost:1", 10, "ndjson", None, 1, 1,
    ).unwrap();
    let result = writer.write_entry(&test_entry());
    match result {
        Ok(_) => {
            let flush_result = writer.flush();
            assert!(flush_result.is_err(), "should fail to connect");
        }
        Err(e) => {
            let msg = format!("{}", e);
            assert!(msg.contains("retries") || msg.contains("Failed")
                || msg.contains("refused") || msg.contains("connect"),
                "unexpected error: {}", msg);
        }
    }
}

#[test]
fn test_http_writer_batch_connection_refused() {
    let mut writer = HttpWriter::new(
        "http://localhost:1", 100, "ndjson", None, 1, 1,
    ).unwrap();
    for _ in 0..250 {
        let _ = writer.write_entry(&test_entry());
    }
    let _ = writer.flush();
}

// ── Mock HTTP Server helpers for HttpWriter tests ──

fn poll_until<F>(check: F, timeout_ms: u64)
where
    F: Fn() -> bool,
{
    let start = std::time::Instant::now();
    while !check() {
        if start.elapsed() > Duration::from_millis(timeout_ms) {
            panic!("timed out after {}ms waiting for condition", timeout_ms);
        }
        thread::sleep(Duration::from_millis(10));
    }
}

fn run_mock_server<F>(handler: F) -> u16
where
    F: Fn(&str) -> (usize, String) + Send + Sync + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let handler = Arc::new(handler);
    let h = handler.clone();

    thread::spawn(move || {
        listener.set_nonblocking(true).ok();
        for _ in 0..50 {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buf = [0; 4096];
                    if let Ok(n) = stream.read(&mut buf) {
                        let raw = String::from_utf8_lossy(&buf[..n]).to_string();
                        let (status, body) = h(&raw);
                        let response = format!(
                            "HTTP/1.1 {} {}\r\nContent-Length: {}\r\n\r\n{}",
                            status,
                            if status == 200 { "OK" } else { "Error" },
                            body.len(),
                            body,
                        );
                        stream.write_all(response.as_bytes()).ok();
                        stream.flush().ok();
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(_) => break,
            }
        }
    });

    port
}

fn run_mock_collector() -> (u16, Arc<std::sync::Mutex<Vec<String>>>, Arc<AtomicUsize>) {
    let requests: Arc<std::sync::Mutex<Vec<String>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
    let count = Arc::new(AtomicUsize::new(0));
    let r = requests.clone();
    let c = count.clone();

    let port = run_mock_server(move |raw| {
        c.fetch_add(1, Ordering::Relaxed);
        r.lock().unwrap().push(raw.to_string());
        (200, "OK".to_string())
    });

    (port, requests, count)
}

// ── HttpWriter Integration Tests (with mock server) ──

#[test]
fn test_http_writer_send_single() {
    let (port, requests, _) = run_mock_collector();
    let url = format!("http://127.0.0.1:{}", port);

    let mut writer = HttpWriter::new(&url, 1, "ndjson", None, 1, 1).unwrap();
    writer.write_entry(&test_entry()).unwrap();
    writer.flush().unwrap();

    poll_until(|| !requests.lock().unwrap().is_empty(), 1000);

    let reqs = requests.lock().unwrap();
    assert!(!reqs.is_empty(), "should have received at least one request");
}

#[test]
fn test_http_writer_batching() {
    let (port, _requests, count) = run_mock_collector();
    let url = format!("http://127.0.0.1:{}", port);

    let mut writer = HttpWriter::new(&url, 100, "ndjson", None, 1, 1).unwrap();
    for _ in 0..250 {
        writer.write_entry(&test_entry()).unwrap();
    }
    writer.flush().unwrap();

    poll_until(|| count.load(Ordering::Relaxed) >= 3, 1000);

    let n = count.load(Ordering::Relaxed);
    assert_eq!(n, 3, "expected 3 POST requests for 250 entries at batch_size=100, got {}", n);
}

#[test]
fn test_http_writer_retry() {
    let attempts = Arc::new(AtomicUsize::new(0));
    let a = attempts.clone();

    let port = run_mock_server(move |_| {
        let n = a.fetch_add(1, Ordering::Relaxed);
        if n < 2 {
            (503, "Service Unavailable".to_string())
        } else {
            (200, "OK".to_string())
        }
    });

    let url = format!("http://127.0.0.1:{}", port);
    let mut writer = HttpWriter::new(&url, 1, "ndjson", None, 3, 1).unwrap();
    let result = writer.write_entry(&test_entry());
    match result {
        Ok(_) => {
            let flush_result = writer.flush();
            assert!(flush_result.is_ok(), "should succeed after retries: {:?}", flush_result);
        }
        Err(e) => {
            panic!("unexpected error on first entry: {}", e);
        }
    }

    poll_until(|| attempts.load(Ordering::Relaxed) >= 3, 1000);
    let n = attempts.load(Ordering::Relaxed);
    assert!(n >= 3, "expected at least 3 attempts (2 failures + 1 success), got {}", n);
}

#[test]
fn test_http_writer_retry_exhausted() {
    let port = run_mock_server(|_| (500, "Internal Server Error".to_string()));
    let url = format!("http://127.0.0.1:{}", port);

    let mut writer = HttpWriter::new(&url, 1, "ndjson", None, 2, 1).unwrap();
    match writer.write_entry(&test_entry()) {
        Ok(_) => {
            let result = writer.flush();
            assert!(result.is_err(), "should fail after exhausting retries");
        }
        Err(e) => {
            let err = format!("{}", e);
            assert!(err.contains("retries") || err.contains("Failed") || err.contains("status code"),
                "error should mention retry failure: {}", err);
            return;
        }
    }
    let result = writer.flush();
    assert!(result.is_err(), "should still fail even if first write buffered");
}

#[test]
fn test_http_writer_format_ndjson() {
    let (port, requests, _) = run_mock_collector();
    let url = format!("http://127.0.0.1:{}", port);

    let mut writer = HttpWriter::new(&url, 1, "ndjson", None, 1, 1).unwrap();
    writer.write_entry(&test_entry()).unwrap();
    writer.flush().unwrap();

    poll_until(|| !requests.lock().unwrap().is_empty(), 1000);
    let reqs = requests.lock().unwrap();
    assert!(!reqs.is_empty(), "should have requests");
    assert!(reqs[0].contains("application/x-ndjson"), "expected ndjson content type");
}

#[test]
fn test_http_writer_format_json() {
    let (port, requests, _) = run_mock_collector();
    let url = format!("http://127.0.0.1:{}", port);

    let mut writer = HttpWriter::new(&url, 1, "json", None, 1, 1).unwrap();
    writer.write_entry(&test_entry()).unwrap();
    writer.flush().unwrap();

    poll_until(|| !requests.lock().unwrap().is_empty(), 1000);
    let reqs = requests.lock().unwrap();
    assert!(!reqs.is_empty(), "should have requests");
    assert!(reqs[0].contains("application/json"), "expected json content type");
}

#[test]
fn test_http_writer_format_raw() {
    let (port, requests, _) = run_mock_collector();
    let url = format!("http://127.0.0.1:{}", port);

    let mut writer = HttpWriter::new(&url, 1, "raw", None, 1, 1).unwrap();
    writer.write_entry(&test_entry()).unwrap();
    writer.flush().unwrap();

    poll_until(|| !requests.lock().unwrap().is_empty(), 1000);
    let reqs = requests.lock().unwrap();
    assert!(!reqs.is_empty(), "should have requests");
    assert!(reqs[0].contains("text/plain"), "expected plain text content type");
}
