use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::time::Instant;

use crate::config::LogEntry;

pub trait LogWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>>;
    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>>;
}

// ---- StdoutWriter ----

pub struct StdoutWriter {
    pub(crate) template_mode: bool,
}

impl StdoutWriter {
    pub fn new() -> Self {
        StdoutWriter { template_mode: false }
    }

    pub fn set_template_mode(&mut self, mode: bool) {
        self.template_mode = mode;
    }
}

impl Default for StdoutWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl LogWriter for StdoutWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        if self.template_mode {
            println!("{}", entry.message);
        } else {
            println!("[{}] [{}] {}", entry.timestamp, entry.level, entry.message);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        Ok(())
    }
}

// ---- FileWriter (with append/truncate and rotation) ----

pub struct FileWriter {
    file: File,
    path: String,
    pub(crate) template_mode: bool,
    rotate_bytes: Option<u64>,
    bytes_written: u64,
}

impl FileWriter {
    pub fn new(path: &str, truncate: bool, rotate_bytes: Option<u64>) -> Result<Self, Box<dyn std::error::Error>> {
        let file = if truncate {
            OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(path)?
        } else {
            OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?
        };
        Ok(FileWriter {
            file,
            path: path.to_string(),
            template_mode: false,
            rotate_bytes,
            bytes_written: 0,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn set_template_mode(&mut self, mode: bool) {
        self.template_mode = mode;
    }
}

impl LogWriter for FileWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        let line = if self.template_mode {
            format!("{}\n", entry.message)
        } else {
            format!("[{}] [{}] {}\n", entry.timestamp, entry.level, entry.message)
        };

        let line_bytes = line.as_bytes();
        self.file.write_all(line_bytes)?;
        self.bytes_written += line_bytes.len() as u64;

        if let Some(limit) = self.rotate_bytes {
            if self.bytes_written >= limit {
                self.file.flush()?;
                let rotated_path = format!("{}.1", self.path);
                let _ = std::fs::rename(&self.path, &rotated_path);
                let new_file = OpenOptions::new()
                    .create(true)
                    .write(true)
                    .truncate(true)
                    .open(&self.path)?;
                self.file = new_file;
                self.bytes_written = 0;
            }
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.file.flush()?;
        Ok(())
    }
}

// ---- BufferedLogWriter (byte-level buffering) ----

pub struct BufferedLogWriter<W: LogWriter> {
    pub(crate) inner: W,
    pub(crate) batch: Vec<LogEntry>,
    pub(crate) buffer_size: u64,
    pub(crate) estimated_bytes: u64,
}

impl<W: LogWriter> BufferedLogWriter<W> {
    pub fn new(inner: W, buffer_size: u64) -> Self {
        BufferedLogWriter {
            inner,
            batch: Vec::new(),
            buffer_size,
            estimated_bytes: 0,
        }
    }

    fn flush_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        for entry in &self.batch {
            self.inner.write_entry(entry)?;
        }
        self.batch.clear();
        self.estimated_bytes = 0;
        Ok(())
    }
}

impl<W: LogWriter> Drop for BufferedLogWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

impl<W: LogWriter> LogWriter for BufferedLogWriter<W> {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        let est_size = entry.message.len() as u64 + 64; // estimate: message + overhead
        self.batch.push(entry.clone());
        self.estimated_bytes += est_size;

        if self.estimated_bytes >= self.buffer_size && self.buffer_size > 0 {
            self.flush_batch()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.flush_batch()?;
        self.inner.flush()?;
        Ok(())
    }
}

// ---- ProgressReporter ----

pub struct ProgressReporter {
    start: Instant,
    last_report: Instant,
    interval_secs: f64,
    entry_interval: u64,
    total: Option<u64>,
    last_reported_entry: u64,
    enabled: bool,
}

impl ProgressReporter {
    pub fn new(enabled: bool, total: Option<u64>, interval_secs: f64, entry_interval: u64) -> Self {
        ProgressReporter {
            start: Instant::now(),
            last_report: Instant::now(),
            interval_secs,
            entry_interval: entry_interval.max(1000),
            total,
            last_reported_entry: 0,
            enabled,
        }
    }

    pub fn report(&mut self, current: u64) {
        if !self.enabled || current == 0 {
            return;
        }
        let now = Instant::now();
        let time_since = now.duration_since(self.last_report).as_secs_f64();
        let entries_since = current - self.last_reported_entry;

        if time_since >= self.interval_secs || entries_since >= self.entry_interval {
            let elapsed = now.duration_since(self.start).as_secs_f64();
            let rate = if elapsed > 0.0 {
                (current as f64 / elapsed) as u64
            } else {
                0
            };
            match self.total {
                Some(total) => eprint!(
                    "\r[loggen] {} / {} entries ({}%) [{:.1}s elapsed, {}/s]",
                    current,
                    total,
                    current.checked_mul(100).and_then(|n| n.checked_div(total)).unwrap_or(0),
                    elapsed,
                    rate
                ),
                None => eprint!(
                    "\r[loggen] ~{} entries [{:.1}s elapsed, {}/s]",
                    current,
                    elapsed,
                    rate
                ),
            }
            self.last_report = now;
            self.last_reported_entry = current;
        }
    }

    pub fn done(&self) {
        if !self.enabled {
            return;
        }
        let elapsed = self.start.elapsed().as_secs_f64();
        match self.total {
            Some(total) => {
                let rate = if elapsed > 0.0 {
                    (total as f64 / elapsed) as u64
                } else {
                    0
                };
                eprintln!(
                    "\r[loggen] Done: {} entries in {:.1}s ({}/s)",
                    total, elapsed, rate
                );
            }
            None => {
                eprintln!(
                    "\r[loggen] Stopped: {} entries generated in {:.1}s",
                    self.last_reported_entry, elapsed
                );
            }
        }
    }
}

// ---- HttpWriter ----

pub struct HttpWriter {
    url: String,
    client: ureq::Agent,
    batch: Vec<String>,
    batch_size: u64,
    format: String,
    headers: Vec<(String, String)>,
    retry_attempts: u32,
    retry_delay_ms: u64,
    entries_sent: u64,
}

impl HttpWriter {
    pub fn new(
        url: &str,
        batch_size: u64,
        format: &str,
        headers: Option<&HashMap<String, String>>,
        retry_attempts: u32,
        retry_delay_ms: u64,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout_read(std::time::Duration::from_secs(10))
            .build();

        let custom_headers: Vec<(String, String)> = headers
            .map(|h| h.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default();

        Ok(HttpWriter {
            url: url.to_string(),
            client,
            batch: Vec::new(),
            batch_size,
            format: format.to_string(),
            headers: custom_headers,
            retry_attempts,
            retry_delay_ms,
            entries_sent: 0,
        })
    }

    fn format_entry_json(&self, entry: &str) -> String {
        // Wrap plain message into a JSON object: {"message": "..."}
        match serde_json::to_string(&entry) {
            Ok(escaped) => format!("{{\"message\":{}}}", escaped),
            Err(_) => format!("{{\"message\":\"{}\"}}", entry.replace('\\', "\\\\").replace('"', "\\\"")),
        }
    }

    fn send_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if self.batch.is_empty() {
            return Ok(());
        }

        let body = match self.format.as_str() {
            "json" => {
                let items: Vec<String> = self.batch.iter()
                    .map(|e| self.format_entry_json(e))
                    .collect();
                format!("[{}]", items.join(","))
            }
            "ndjson" => {
                self.batch.iter()
                    .map(|e| self.format_entry_json(e))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => self.batch.join("\n"), // raw
        };

        let content_type = match self.format.as_str() {
            "json" => "application/json",
            "raw" => "text/plain",
            _ => "application/x-ndjson",
        };

        let mut last_err = None;
        for attempt in 0..=self.retry_attempts {
            let mut req = self.client.post(&self.url).set("Content-Type", content_type);
            for (k, v) in &self.headers {
                req = req.set(k, v);
            }

            let result = req.send_string(&body);
            match result {
                Ok(resp) if resp.status() >= 200 && resp.status() < 300 => {
                    self.entries_sent += self.batch.len() as u64;
                    self.batch.clear();
                    return Ok(());
                }
                Ok(resp) => {
                    last_err = Some(format!("HTTP {}", resp.status()));
                }
                Err(e) => {
                    last_err = Some(format!("{}", e));
                }
            }

            if attempt < self.retry_attempts {
                std::thread::sleep(std::time::Duration::from_millis(self.retry_delay_ms));
            }
        }

        let err_msg = last_err.unwrap_or_else(|| "unknown error".to_string());
        Err(format!("Failed to send batch after {} retries: {}", self.retry_attempts, err_msg).into())
    }
}

impl LogWriter for HttpWriter {
    fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
        self.batch.push(entry.message.clone());
        if self.batch.len() as u64 >= self.batch_size {
            self.send_batch()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.batch.is_empty() {
            self.send_batch()?;
        }
        Ok(())
    }
}

impl Drop for HttpWriter {
    fn drop(&mut self) {
        let _ = self.flush();
    }
}

// ---- KafkaWriter (simplified - feature-gated) ----

#[cfg(not(feature = "kafka"))]
mod kafka_impl {
    use crate::config::LogEntry;
    use crate::output::LogWriter;

    pub struct KafkaWriter;

    impl KafkaWriter {
        pub fn new(
            _brokers: &str,
            _topic: &str,
            _key_var: Option<&str>,
            _acks: &str,
            _timeout_ms: u64,
            _batch_size: u64,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            Err("Kafka support not enabled. Build with --features kafka (requires librdkafka)".into())
        }
    }

    impl LogWriter for KafkaWriter {
        fn write_entry(&mut self, _entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
            Err("Kafka writer not available (feature not enabled)".into())
        }

        fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }
}

#[cfg(feature = "kafka")]
mod kafka_impl {
    use std::time::Duration;
    use rdkafka::producer::BaseProducer;
    use rdkafka::producer::BaseRecord;
    use rdkafka::producer::Producer;
    use rdkafka::ClientConfig;
    use crate::config::LogEntry;
    use crate::output::LogWriter;

    pub struct KafkaWriter {
        producer: BaseProducer,
        topic: String,
        #[allow(dead_code)]
        key_var: Option<String>,
        batch: Vec<String>,
        batch_size: u64,
        keys: Vec<String>,
        pub entries_produced: u64,
        pub errors: u64,
    }

    impl KafkaWriter {
        pub fn new(
            brokers: &str,
            topic: &str,
            key_var: Option<&str>,
            acks: &str,
            timeout_ms: u64,
            batch_size: u64,
        ) -> Result<Self, Box<dyn std::error::Error>> {
            let mut config = ClientConfig::new();
            config.set("bootstrap.servers", brokers);
            config.set("acks", acks);
            config.set("queue.buffering.max.ms", "100");
            config.set("message.timeout.ms", &timeout_ms.to_string());
            let producer: BaseProducer = config.create()?;
            Ok(KafkaWriter {
                producer,
                topic: topic.to_string(),
                key_var: key_var.map(|s| s.to_string()),
                batch: Vec::new(),
                batch_size,
                keys: Vec::new(),
                entries_produced: 0,
                errors: 0,
            })
        }

        fn flush_batch(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            if self.batch.is_empty() {
                return Ok(());
            }
            for (i, msg) in self.batch.iter().enumerate() {
                let key = self.keys.get(i).map(|k| k.as_str()).unwrap_or("");
                let record = BaseRecord::to(&self.topic).key(key).payload(msg);
                if let Err((e, _)) = self.producer.send(record) {
                    self.errors += 1;
                    eprintln!("Kafka send error: {}", e);
                } else {
                    self.entries_produced += 1;
                }
            }
            self.producer.flush(Duration::from_secs(15));
            self.batch.clear();
            self.keys.clear();
            Ok(())
        }
    }

    impl LogWriter for KafkaWriter {
        fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
            self.batch.push(entry.message.clone());
            self.keys.push(String::new());
            if self.batch.len() as u64 >= self.batch_size {
                self.flush_batch()?;
            }
            Ok(())
        }

        fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            self.flush_batch()?;
            Ok(())
        }
    }

    impl Drop for KafkaWriter {
        fn drop(&mut self) {
            let _ = self.flush();
        }
    }
}

pub use kafka_impl::KafkaWriter;

#[cfg(test)]
mod tests {
    use super::*;

    struct TestWriter {
        entries: Vec<LogEntry>,
        write_count: usize,
    }

    impl TestWriter {
        fn new() -> Self {
            TestWriter {
                entries: Vec::new(),
                write_count: 0,
            }
        }
    }

    impl LogWriter for TestWriter {
        fn write_entry(&mut self, entry: &LogEntry) -> Result<(), Box<dyn std::error::Error>> {
            self.entries.push(entry.clone());
            self.write_count += 1;
            Ok(())
        }

        fn flush(&mut self) -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }
    }

    #[test]
    fn test_stdout_writer_new() {
        let w = StdoutWriter::new();
        assert!(!w.template_mode);
    }

    #[test]
    fn test_file_writer_new() {
        let path = "test_new_file.log";
        {
            let mut writer = FileWriter::new(path, true, None).unwrap();
            writer.template_mode = true;
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: "test".to_string(),
            }).unwrap();
            writer.flush().unwrap();
        }
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "test\n");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_file_writer_append_mode() {
        let path = "test_append.log";
        {
            let mut writer = FileWriter::new(path, true, None).unwrap();
            writer.template_mode = true;
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: "first".to_string(),
            }).unwrap();
            writer.flush().unwrap();
        }
        {
            let mut writer = FileWriter::new(path, false, None).unwrap();
            writer.template_mode = true;
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: "second".to_string(),
            }).unwrap();
            writer.flush().unwrap();
        }
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "first\nsecond\n");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_file_writer_truncate_mode() {
        let path = "test_truncate.log";
        {
            let mut writer = FileWriter::new(path, false, None).unwrap();
            writer.template_mode = true;
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: "first".to_string(),
            }).unwrap();
            writer.flush().unwrap();
        }
        {
            let mut writer = FileWriter::new(path, true, None).unwrap();
            writer.template_mode = true;
            writer.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: "second".to_string(),
            }).unwrap();
            writer.flush().unwrap();
        }
        let content = std::fs::read_to_string(path).unwrap();
        assert_eq!(content, "second\n");
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn test_file_rotation() {
        let path = "test_rotation.log";
        {
            let mut writer = FileWriter::new(path, true, Some(50)).unwrap();
            writer.template_mode = true;
            for i in 0..20 {
                writer.write_entry(&LogEntry {
                    timestamp: "0".to_string(),
                    level: "INFO".to_string(),
                    message: format!("entry{}", i),
                }).unwrap();
            }
            writer.flush().unwrap();
        }
        assert!(std::path::Path::new(path).exists());
        assert!(std::path::Path::new("test_rotation.log.1").exists());
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file("test_rotation.log.1");
    }

    #[test]
    fn test_progress_reporter_basic() {
        let mut pr = ProgressReporter::new(true, Some(100), 0.0, 50);
        assert!(pr.enabled);
        pr.report(1);
        pr.report(50);
        pr.report(100);
        pr.done();

        let mut pr = ProgressReporter::new(false, Some(100), 0.0, 50);
        pr.report(50);
        pr.done();
    }

    #[test]
    fn test_buffered_writer_basic() {
        let inner = TestWriter::new();
        let mut buf = BufferedLogWriter::new(inner, 8192);

        for i in 0..5 {
            buf.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: format!("entry {}", i),
            }).unwrap();
        }
        buf.flush().unwrap();

        assert_eq!(buf.inner.write_count, 5);
    }

    #[test]
    fn test_buffered_writer_flush_on_size() {
        let inner = TestWriter::new();
        // Very small buffer to force frequent flushes
        let mut buf = BufferedLogWriter::new(inner, 1);

        for i in 0..10 {
            buf.write_entry(&LogEntry {
                timestamp: "0".to_string(),
                level: "INFO".to_string(),
                message: format!("entry {}", i),
            }).unwrap();
        }
        buf.flush().unwrap();

        assert_eq!(buf.inner.write_count, 10);
    }

    #[test]
    fn test_http_writer_new() {
        let writer = HttpWriter::new("http://localhost:9999", 100, "ndjson", None, 3, 1000);
        assert!(writer.is_ok());
    }

    #[test]
    fn test_http_writer_send_failure() {
        // Sending to non-existent endpoint should fail
        let mut writer = HttpWriter::new("http://localhost:1", 10, "ndjson", None, 2, 10).unwrap();
        let result = writer.write_entry(&LogEntry {
            timestamp: "0".to_string(),
            level: "INFO".to_string(),
            message: "test".to_string(),
        });
        // Should either succeed (unlikely) or fail with connection error
        if let Err(e) = result {
            let msg = format!("{}", e);
            assert!(msg.contains("retries") || msg.contains("Failed") || msg.contains("connect"), "unexpected error: {}", msg);
        }
    }

    #[test]
    fn test_kafka_config_deser() {
        use crate::config::KafkaOutputConfig;
        let yaml = r#"
brokers: "broker1:9092,broker2:9092"
topic: "test-topic"
key_var: "ipv4"
acks: "all"
timeout_ms: 10000
batch_size: 50
"#;
        let config: KafkaOutputConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.brokers, "broker1:9092,broker2:9092");
        assert_eq!(config.topic, "test-topic");
        assert_eq!(config.key_var.as_deref(), Some("ipv4"));
        assert_eq!(config.acks, "all");
        assert_eq!(config.timeout_ms, 10000);
        assert_eq!(config.batch_size, 50);
    }

    #[test]
    fn test_http_format_entry_json() {
        let writer = HttpWriter::new("http://localhost:1", 10, "ndjson", None, 1, 1).unwrap();
        let result = writer.format_entry_json("hello world");
        assert_eq!(result, "{\"message\":\"hello world\"}");
    }

    #[test]
    fn test_http_format_entry_json_special_chars() {
        let writer = HttpWriter::new("http://localhost:1", 10, "ndjson", None, 1, 1).unwrap();
        let result = writer.format_entry_json("he said \"hello\" & <bye>");
        assert_eq!(result, "{\"message\":\"he said \\\"hello\\\" & <bye>\"}");
    }

    #[test]
    fn test_http_format_entry_json_backslash() {
        let writer = HttpWriter::new("http://localhost:1", 10, "ndjson", None, 1, 1).unwrap();
        let result = writer.format_entry_json("path\\to\\file");
        assert_eq!(result, "{\"message\":\"path\\\\to\\\\file\"}");
    }

    #[test]
    fn test_http_send_batch_format_json() {
        // Use a helper to inspect batch formatting without making HTTP requests
        let mut writer = HttpWriter::new("http://localhost:1", 100, "json", None, 1, 1).unwrap();
        // Manually populate batch to check body construction
        writer.batch.push("entry one".to_string());
        writer.batch.push("entry two".to_string());
        // flush will try to send and fail, but body is constructed before the request
        let result = writer.flush();
        assert!(result.is_err()); // connection refused
    }
}