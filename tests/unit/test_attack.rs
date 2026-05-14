use std::collections::HashMap;

use loggen::cli::{apply_cli_args, merge_cli_attacks, parse_attack_spec};
use loggen::config::{AttackConfig, AttackVarConfig, Config, OutputConfig, ThresholdConfig};
use loggen::Generator;

fn base_config() -> Config {
    Config {
        output: OutputConfig::default(),
        count: 100,
        log_level: "INFO".to_string(),
        message: "test".to_string(),
        logs: None,
        templates: None,
        template_vars: None,
        seed: Some(42),
        random_vars: None,
        random_intensity: 1.0,
        template_rotation: "sequential".to_string(),
        attacks: None,
        attack_only: false,
        num_threads: None,
        progress: None,
        progress_interval: 10000,
    }
}

// ── AttackConfig deserialization ──

#[test]
fn test_attack_config_deser() {
    let yaml = r#"
name: test-attack
type: single_event
template: '{{ ipv4 }} - {{ status }}'
count: 50
interleave: true
weight: 0.3
repeat: loop
vars:
  status:
    values: ["200", "404"]
    mode: weighted
"#;
    let config: AttackConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.name.as_deref(), Some("test-attack"));
    assert_eq!(config.attack_type, "single_event");
    assert_eq!(config.template.as_deref(), Some("{{ ipv4 }} - {{ status }}"));
    assert_eq!(config.count, Some(50));
    assert!(config.interleave);
    assert!((config.weight - 0.3).abs() < 1e-6);
    assert_eq!(config.repeat, "loop");
    assert!(config.sequence.is_none());
    assert!(config.threshold.is_none());

    let vars = config.vars.unwrap();
    assert_eq!(vars["status"].values, vec!["200", "404"]);
    assert_eq!(vars["status"].mode, "weighted");
}

#[test]
fn test_attack_config_multi_ordered_deser() {
    let yaml = r#"
type: multi_ordered
sequence:
  - "step one"
  - "step two"
  - "step three"
count: 30
repeat: once
"#;
    let config: AttackConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.attack_type, "multi_ordered");
    let seq = config.sequence.unwrap();
    assert_eq!(seq.len(), 3);
    assert_eq!(seq[0], "step one");
    assert_eq!(config.repeat, "once");
    assert_eq!(config.count, Some(30));
}

#[test]
fn test_attack_config_threshold_deser() {
    let yaml = r#"
type: threshold_field
template: '{{ ipv4 }} - {{ status }}'
threshold:
  field: status
  min: 500
  max: 599
  proportion: 0.8
count: 200
"#;
    let config: AttackConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.attack_type, "threshold_field");
    let th = config.threshold.unwrap();
    assert_eq!(th.field, "status");
    assert_eq!(th.min, Some(500));
    assert_eq!(th.max, Some(599));
    assert!((th.proportion - 0.8).abs() < 1e-6);
}

// ── Single event count ──

#[test]
fn test_single_event_count() {
    let attack = AttackConfig {
        name: Some("test".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("hello from attack {{ index }}".to_string()),
        sequence: None,
        count: Some(10),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 10);
    for (i, entry) in entries.iter().enumerate() {
        assert!(entry.message.contains("hello from attack"));
        assert!(entry.message.contains(&format!("{}", i + 1)));
    }
}

// ── Multi-ordered sequence order ──

#[test]
fn test_multi_ordered_sequence_order() {
    let attack = AttackConfig {
        name: Some("scan".to_string()),
        attack_type: "multi_ordered".to_string(),
        template: None,
        sequence: Some(vec![
            "port-22".to_string(),
            "port-80".to_string(),
            "port-443".to_string(),
            "port-8080".to_string(),
        ]),
        count: Some(8),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 8);
    // Expect: 0-3 = 22, 80, 443, 8080; 4-7 = 22, 80, 443, 8080
    assert_eq!(entries[0].message, "port-22");
    assert_eq!(entries[1].message, "port-80");
    assert_eq!(entries[2].message, "port-443");
    assert_eq!(entries[3].message, "port-8080");
    assert_eq!(entries[4].message, "port-22");
    assert_eq!(entries[5].message, "port-80");
    assert_eq!(entries[6].message, "port-443");
    assert_eq!(entries[7].message, "port-8080");
}

// ── Multi-ordered once ──

#[test]
fn test_multi_ordered_once() {
    let attack = AttackConfig {
        name: Some("scan".to_string()),
        attack_type: "multi_ordered".to_string(),
        template: None,
        sequence: Some(vec![
            "step-A".to_string(),
            "step-B".to_string(),
            "step-C".to_string(),
        ]),
        count: Some(10),
        interleave: false,
        weight: 0.5,
        repeat: "once".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    // Only 3 entries produced (sequence exhausted)
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].message, "step-A");
    assert_eq!(entries[1].message, "step-B");
    assert_eq!(entries[2].message, "step-C");
}

// ── Threshold field proportion ──

#[test]
fn test_threshold_field_proportion() {
    let attack = AttackConfig {
        name: Some("ddos".to_string()),
        attack_type: "threshold_field".to_string(),
        template: Some("status={{ status }}".to_string()),
        sequence: None,
        count: Some(1000),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: Some(ThresholdConfig {
            field: "status".to_string(),
            min: Some(500),
            max: None,
            proportion: 0.7,
        }),
        vars: None,
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 1000);

    let in_bucket = entries
        .iter()
        .filter(|e| {
            // Parse "status=NUMBER" and check >= 500
            let msg = &e.message;
            if let Some(val_str) = msg.strip_prefix("status=") {
                if let Ok(val) = val_str.parse::<u64>() {
                    return val >= 500;
                }
            }
            false
        })
        .count();

    // Statistical: expect roughly 65-75% in bucket (700 +/- 50)
    let pct = in_bucket as f64 / 1000.0;
    assert!(
        (pct - 0.7).abs() < 0.1,
        "expected ~70% in bucket, got {}% ({} / 1000)",
        pct * 100.0,
        in_bucket
    );
}

// ── Attack var override ──

#[test]
fn test_attack_var_override() {
    let attack = AttackConfig {
        name: Some("test".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("app={{ app }} status={{ status }}".to_string()),
        sequence: None,
        count: Some(5),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: Some(HashMap::from([(
            "status".to_string(),
            AttackVarConfig {
                values: vec!["999".to_string()],
                mode: "cycle".to_string(),
            },
        )])),
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        template_vars: Some(HashMap::from([("app".to_string(), "globalapp".to_string())])),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 5);
    for entry in &entries {
        assert!(entry.message.contains("app=globalapp"));
        // Attack var overrides the random status
        assert!(entry.message.contains("status=999"), "expected status=999 but got: {}", entry.message);
    }
}

// ── Interleaving total count ──

#[test]
fn test_interleaving_total_count() {
    let normal_config = Config {
        count: 100,
        logs: Some(vec!["normal-{{ index }}".to_string()]),
        templates: None,
        seed: Some(42),
        attacks: Some(vec![
            AttackConfig {
                name: Some("attack-a".to_string()),
                attack_type: "single_event".to_string(),
                template: Some("attack-a-{{ index }}".to_string()),
                sequence: None,
                count: Some(50),
                interleave: true,
                weight: 0.5,
                repeat: "loop".to_string(),
                threshold: None,
                vars: None,
                common: None,
            },
            AttackConfig {
                name: Some("attack-b".to_string()),
                attack_type: "single_event".to_string(),
                template: Some("attack-b-{{ index }}".to_string()),
                sequence: None,
                count: Some(30),
                interleave: true,
                weight: 0.5,
                repeat: "loop".to_string(),
                threshold: None,
                vars: None,
                common: None,
            },
        ]),
        ..base_config()
    };
    let gen = Generator::new(normal_config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 180);
    let normal_count = entries.iter().filter(|e| e.message.starts_with("normal-")).count();
    let a_count = entries.iter().filter(|e| e.message.starts_with("attack-a-")).count();
    let b_count = entries.iter().filter(|e| e.message.starts_with("attack-b-")).count();
    assert_eq!(normal_count, 100);
    assert_eq!(a_count, 50);
    assert_eq!(b_count, 30);
}

// ── Attack only ──

#[test]
fn test_attack_only() {
    let config = Config {
        count: 100,
        attacks: Some(vec![
            AttackConfig {
                name: Some("a".to_string()),
                attack_type: "single_event".to_string(),
                template: Some("only-a-{{ index }}".to_string()),
                sequence: None,
                count: Some(50),
                interleave: false,
                weight: 0.5,
                repeat: "loop".to_string(),
                threshold: None,
                vars: None,
                common: None,
            },
            AttackConfig {
                name: Some("b".to_string()),
                attack_type: "single_event".to_string(),
                template: Some("only-b-{{ index }}".to_string()),
                sequence: None,
                count: Some(30),
                interleave: false,
                weight: 0.5,
                repeat: "loop".to_string(),
                threshold: None,
                vars: None,
                common: None,
            },
        ]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 80);
    let a_count = entries.iter().filter(|e| e.message.starts_with("only-a-")).count();
    let b_count = entries.iter().filter(|e| e.message.starts_with("only-b-")).count();
    assert_eq!(a_count, 50);
    assert_eq!(b_count, 30);
    // No normal entries (no legacy templates configured)
    assert!(!entries.iter().any(|e| e.message.starts_with("Log entry generated")),
        "should have no normal entries");
}

// ── No interleave ordering ──

#[test]
fn test_attack_no_interleave_ordering() {
    let config = Config {
        count: 100,
        attacks: Some(vec![
            AttackConfig {
                name: Some("first".to_string()),
                attack_type: "single_event".to_string(),
                template: Some("first-attack-{{ index }}".to_string()),
                sequence: None,
                count: Some(5),
                interleave: false,
                weight: 0.5,
                repeat: "loop".to_string(),
                threshold: None,
                vars: None,
                common: None,
            },
            AttackConfig {
                name: Some("second".to_string()),
                attack_type: "single_event".to_string(),
                template: Some("second-attack-{{ index }}".to_string()),
                sequence: None,
                count: Some(5),
                interleave: false,
                weight: 0.5,
                repeat: "loop".to_string(),
                threshold: None,
                vars: None,
                common: None,
            },
        ]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 10);
    // All first-attack entries come before second-attack entries
    for i in 0..5 {
        assert!(entries[i].message.starts_with("first-attack-"), "idx {}: expected first-attack, got {}", i, entries[i].message);
    }
    for i in 5..10 {
        assert!(entries[i].message.starts_with("second-attack-"), "idx {}: expected second-attack, got {}", i, entries[i].message);
    }
}

// ── Attack var mode: cycle ──

#[test]
fn test_attack_var_mode_cycle() {
    let attack = AttackConfig {
        name: Some("cycle-test".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("val={{ x }}".to_string()),
        sequence: None,
        count: Some(6),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: Some(HashMap::from([(
            "x".to_string(),
            AttackVarConfig {
                values: vec!["a".to_string(), "b".to_string(), "c".to_string()],
                mode: "cycle".to_string(),
            },
        )])),
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        attack_only: true,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 6);
    assert_eq!(entries[0].message, "val=a");
    assert_eq!(entries[1].message, "val=b");
    assert_eq!(entries[2].message, "val=c");
    assert_eq!(entries[3].message, "val=a");
    assert_eq!(entries[4].message, "val=b");
    assert_eq!(entries[5].message, "val=c");
}

// ── Attack var mode: weighted ──

#[test]
fn test_attack_var_mode_weighted() {
    let attack = AttackConfig {
        name: Some("weight-test".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("val={{ x }}".to_string()),
        sequence: None,
        count: Some(1000),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: Some(HashMap::from([(
            "x".to_string(),
            AttackVarConfig {
                values: vec!["alpha".to_string(), "beta".to_string()],
                mode: "weighted".to_string(),
            },
        )])),
        common: None,
    };
    let config = Config {
        attacks: Some(vec![attack]),
        attack_only: true,
        seed: Some(123),
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    // With 2 values weighted: alpha weight=2, beta weight=1 -> alpha ~66%, beta ~33%
    let alpha = entries.iter().filter(|e| e.message == "val=alpha").count();
    let beta = entries.iter().filter(|e| e.message == "val=beta").count();
    assert_eq!(alpha + beta, 1000);
    assert!(alpha > beta, "expected alpha > beta, got alpha={} beta={}", alpha, beta);
    assert!((alpha as f64 / 1000.0 - 2.0/3.0).abs() < 0.1,
        "expected ~66.7% alpha, got {}%", alpha as f64 / 10.0);
}

// ── CLI parse_attack_spec ──

#[test]
fn test_parse_attack_spec_single() {
    let (name, config) = parse_attack_spec("test=single:hello {{ ip }} :50").unwrap();
    assert_eq!(name, "test");
    assert_eq!(config.attack_type, "single_event");
    assert_eq!(config.template.as_deref(), Some("hello {{ ip }}"));
    assert_eq!(config.count, Some(50));
}

#[test]
fn test_parse_attack_spec_multi() {
    let (name, config) = parse_attack_spec("scan=multi:probe {{ port }} :100").unwrap();
    assert_eq!(name, "scan");
    assert_eq!(config.attack_type, "multi_ordered");
    assert_eq!(config.sequence.as_ref().unwrap(), &vec!["probe {{ port }}".to_string()]);
    assert_eq!(config.count, Some(100));
}

#[test]
fn test_parse_attack_spec_no_count() {
    let (name, config) = parse_attack_spec("x=single:just a template").unwrap();
    assert_eq!(name, "x");
    assert_eq!(config.template.as_deref(), Some("just a template"));
    assert_eq!(config.count, None);
}

#[test]
fn test_parse_attack_spec_threshold() {
    let (name, config) = parse_attack_spec("ddos=threshold:status check").unwrap();
    assert_eq!(name, "ddos");
    assert_eq!(config.attack_type, "threshold_field");
    assert_eq!(config.template.as_deref(), Some("status check"));
}

#[test]
fn test_parse_attack_spec_malformed() {
    assert!(parse_attack_spec("no-equal-sign").is_none());
    assert!(parse_attack_spec("").is_none());
}

// ── CLI merge_cli_attacks ──

#[test]
fn test_merge_cli_attacks_groups_multi() {
    let attacks = vec![
        AttackConfig {
            name: Some("scan".to_string()),
            attack_type: "multi_ordered".to_string(),
            template: None,
            sequence: Some(vec!["step1".to_string()]),
            count: Some(10),
            interleave: false,
            weight: 0.5,
            repeat: "loop".to_string(),
            threshold: None,
            vars: None,
            common: None,
        },
        AttackConfig {
            name: Some("scan".to_string()),
            attack_type: "multi_ordered".to_string(),
            template: None,
            sequence: Some(vec!["step2".to_string()]),
            count: None,
            interleave: false,
            weight: 0.5,
            repeat: "loop".to_string(),
            threshold: None,
            vars: None,
            common: None,
        },
    ];
    let merged = merge_cli_attacks(attacks);
    assert_eq!(merged.len(), 1);
    let seq = merged[0].sequence.as_ref().unwrap();
    assert_eq!(seq.len(), 2);
    assert_eq!(seq[0], "step1");
    assert_eq!(seq[1], "step2");
}

// ── apply_cli_args with attack params ──

#[test]
fn test_apply_cli_args_with_attacks() {
    let attack = AttackConfig {
        name: Some("cli-attack".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("cli-{{ index }}".to_string()),
        sequence: None,
        count: Some(5),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let config = apply_cli_args(
        Config::default(),
        None,
        Some(10),
        Some("WARN".into()),
        Some("msg".into()),
        HashMap::new(),
        None,
        vec![attack],
        true,
    );
    assert_eq!(config.count, 10);
    assert_eq!(config.log_level, "WARN");
    assert_eq!(config.message, "msg");
    assert!(config.attack_only);
    let attacks = config.attacks.unwrap();
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].name.as_deref(), Some("cli-attack"));
}

// ── Test that example attack YAML files deserialize ──

#[test]
fn test_attack_example_brute_force() {
    let config: Config = loggen::read_yaml_file("examples/attack-brute-force.yaml").unwrap();
    let attacks = config.attacks.unwrap();
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].attack_type, "single_event");
    assert_eq!(attacks[0].count, Some(50));
}

#[test]
fn test_attack_example_port_scan() {
    let config: Config = loggen::read_yaml_file("examples/attack-port-scan.yaml").unwrap();
    let attacks = config.attacks.unwrap();
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].attack_type, "multi_ordered");
    assert_eq!(attacks[0].sequence.as_ref().unwrap().len(), 5);
}

#[test]
fn test_attack_example_ddos() {
    let config: Config = loggen::read_yaml_file("examples/attack-ddos.yaml").unwrap();
    let attacks = config.attacks.unwrap();
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].attack_type, "threshold_field");
}

#[test]
fn test_attack_example_sqli() {
    let config: Config = loggen::read_yaml_file("examples/attack-sqli-probe.yaml").unwrap();
    let attacks = config.attacks.unwrap();
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].attack_type, "multi_ordered");
}

#[test]
fn test_attack_example_credential_stuffing() {
    let config: Config = loggen::read_yaml_file("examples/attack-credential-stuffing.yaml").unwrap();
    let attacks = config.attacks.unwrap();
    assert_eq!(attacks.len(), 1);
    assert_eq!(attacks[0].attack_type, "single_event");
}

// ── Attack with random_intensity < 1.0 ──

#[test]
fn test_attack_with_random_intensity_below_one() {
    let attack = AttackConfig {
        name: Some("test".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("{{ status }}".to_string()),
        sequence: None,
        count: Some(50),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let config = Config {
        count: 50,
        attacks: Some(vec![attack]),
        attack_only: true,
        random_intensity: 0.5,
        seed: Some(42),
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    assert_eq!(entries.len(), 50);
    // All entries should have a valid status value rendered
    for entry in &entries {
        assert!(!entry.message.is_empty(), "entry should have a status rendered");
    }
}

// ── Legacy mode with attacks (no templates, no logs) ──

#[test]
fn test_attack_with_legacy_normal() {
    // No template configured: normal entries use legacy mode
    // Attack entries are appended after normal entries
    let attack = AttackConfig {
        name: Some("a1".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("attack-entry-{{ index }}".to_string()),
        sequence: None,
        count: Some(5),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let config = Config {
        count: 10,
        attacks: Some(vec![attack]),
        seed: Some(42),
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    // 10 normal (legacy) + 5 attack = 15
    assert_eq!(entries.len(), 15);
    for i in 0..10 {
        assert!(entries[i].message.contains("test #"), "normal entry {} should be legacy", i);
    }
    for i in 10..15 {
        assert!(entries[i].message.starts_with("attack-entry-"), "attack entry {} should be attack", i);
    }
}

// ── Parallel fallback when interleave/multi_ordered attacks exist ──

#[test]
fn test_attack_parallel_fallback() {
    // With interleaving attacks and random_intensity >= 1.0 the generator
    // falls back to serial path (no rayon) and produces entries correctly
    let config = Config {
        count: 50,
        logs: Some(vec!["normal-{{ index }}".to_string()]),
        attacks: Some(vec![AttackConfig {
            name: Some("interleaved".to_string()),
            attack_type: "single_event".to_string(),
            template: Some("attack-{{ index }}".to_string()),
            sequence: None,
            count: Some(10),
            interleave: true,
            weight: 0.5,
            repeat: "loop".to_string(),
            threshold: None,
            vars: None,
            common: None,
        }]),
        seed: Some(42),
        random_intensity: 1.0,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    // 50 normal + 10 attack = 60
    assert_eq!(entries.len(), 60);
    let normal_count = entries.iter().filter(|e| e.message.starts_with("normal-")).count();
    let attack_count = entries.iter().filter(|e| e.message.starts_with("attack-")).count();
    assert_eq!(normal_count, 50);
    assert_eq!(attack_count, 10);
}

#[test]
fn test_attack_parallel_fallback_multi_ordered() {
    // With multi_ordered (non-interleaved) attacks and random_intensity >= 1.0,
    // normal entries still use parallel path, entries produced correctly
    let config = Config {
        count: 50,
        logs: Some(vec!["normal-{{ index }}".to_string()]),
        attacks: Some(vec![AttackConfig {
            name: Some("ordered".to_string()),
            attack_type: "multi_ordered".to_string(),
            template: None,
            sequence: Some(vec!["step-A".to_string(), "step-B".to_string()]),
            count: Some(6),
            interleave: false,
            weight: 0.5,
            repeat: "once".to_string(),
            threshold: None,
            vars: None,
            common: None,
        }]),
        seed: Some(42),
        random_intensity: 1.0,
        ..base_config()
    };
    let gen = Generator::new(config);
    let entries = gen.generate();
    // 50 normal + 2 attack (sequence exhausted at repeat=once) = 52
    assert_eq!(entries.len(), 52);
    let normal_count = entries.iter().filter(|e| e.message.starts_with("normal-")).count();
    let attack_count = entries.iter().filter(|e| e.message.starts_with("step-")).count();
    assert_eq!(normal_count, 50);
    assert_eq!(attack_count, 2);
}

// ── Seeded attack reproducibility ──

#[test]
fn test_seeded_attack_reproducibility() {
    let attack = AttackConfig {
        name: Some("rep".to_string()),
        attack_type: "single_event".to_string(),
        template: Some("{{ ipv4 }}:{{ status }}".to_string()),
        sequence: None,
        count: Some(10),
        interleave: false,
        weight: 0.5,
        repeat: "loop".to_string(),
        threshold: None,
        vars: None,
        common: None,
    };
    let make_config = || Config {
        count: 10,
        attacks: Some(vec![attack.clone()]),
        attack_only: true,
        seed: Some(99),
        random_intensity: 1.0,
        ..base_config()
    };
    let gen1 = Generator::new(make_config());
    let gen2 = Generator::new(make_config());
    let e1 = gen1.generate();
    let e2 = gen2.generate();
    assert_eq!(e1.len(), e2.len());
    for (i, (a, b)) in e1.iter().zip(e2.iter()).enumerate() {
        assert_eq!(a.message, b.message, "entry {} mismatch", i);
    }
}
