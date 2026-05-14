use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::{Config, LogEntry};

fn current_timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_string())
}

pub struct Generator {
    config: Config,
}

impl Generator {
    pub fn new(config: Config) -> Self {
        Generator { config }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn generate(&self) -> Vec<LogEntry> {
        self.generate_with_count(self.config.count)
    }

    pub fn generate_with_count(&self, count: u64) -> Vec<LogEntry> {
        let mut entries = Vec::with_capacity(count as usize);
        for i in 0..count {
            entries.push(LogEntry {
                timestamp: current_timestamp(),
                level: self.config.log_level.clone(),
                message: format!("{} #{}", self.config.message, i + 1),
            });
        }
        entries
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::OutputConfig;

    fn test_config() -> Config {
        Config {
            output: OutputConfig::default(),
            count: 1,
            log_level: "INFO".to_string(),
            message: "test".to_string(),
        }
    }

    #[test]
    fn test_generate_default_count() {
        let config = Config {
            count: 3,
            ..test_config()
        };
        let generator = Generator::new(config);
        let entries = generator.generate();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_generate_with_count() {
        let generator = Generator::new(test_config());
        let entries = generator.generate_with_count(5);
        assert_eq!(entries.len(), 5);
    }

    #[test]
    fn test_generate_zero_count() {
        let generator = Generator::new(test_config());
        let entries = generator.generate_with_count(0);
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_generate_entry_content() {
        let config = Config {
            count: 1,
            log_level: "ERROR".to_string(),
            message: "custom message".to_string(),
            ..test_config()
        };
        let generator = Generator::new(config);
        let entries = generator.generate();
        assert_eq!(entries[0].level, "ERROR");
        assert_eq!(entries[0].message, "custom message #1");
        assert!(!entries[0].timestamp.is_empty());
    }

    #[test]
    fn test_generate_large_count() {
        let generator = Generator::new(test_config());
        let entries = generator.generate_with_count(1000);
        assert_eq!(entries.len(), 1000);
        assert_eq!(entries[999].message, "test #1000");
    }
}
