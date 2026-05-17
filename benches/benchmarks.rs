use std::collections::HashMap;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use loggen::{Config, Generator};

fn bench_config_legacy() -> Config {
    Config {
        count: 100_000,
        message: "Benchmark log entry".to_string(),
        ..Config::default()
    }
}

fn bench_config_template_simple() -> Config {
    Config {
        count: 100_000,
        logs: Some(vec![
            "{{ level }} | {{ message }} | app={{ app_name }} host={{ host }}".to_string(),
        ]),
        template_vars: Some(HashMap::from([
            ("app_name".to_string(), "loggen-bench".to_string()),
            ("host".to_string(), "bench-01".to_string()),
        ])),
        seed: Some(42),
        ..Config::default()
    }
}

fn bench_config_template_random() -> Config {
    Config {
        count: 100_000,
        logs: Some(vec![
            "{{ ipv4 }} - {{ email }} [{{ timestamp | date(format=\"%d/%b/%Y:%H:%M:%S %z\") }}] \"{{ url }} HTTP/1.1\" {{ status }} {{ port }} \"-\" \"{{ user_agent }}\"".to_string(),
        ]),
        seed: Some(42),
        ..Config::default()
    }
}

fn bench_config_parallel() -> Config {
    Config {
        count: 500_000,
        logs: Some(vec![
            "{{ ipv4 }} - {{ email }} [{{ timestamp }}] \"GET {{ url }} HTTP/1.1\" {{ status }} {{ port }}".to_string(),
            "{{ ipv4 }} - {{ email }} [{{ timestamp }}] \"POST {{ url }} HTTP/1.1\" {{ status }} {{ port }}".to_string(),
            "{{ ipv4 }} - {{ email }} [{{ timestamp }}] \"PUT {{ url }} HTTP/1.1\" {{ status }} {{ port }}".to_string(),
            "{{ ipv4 }} - {{ email }} [{{ timestamp }}] \"DELETE {{ url }} HTTP/1.1\" {{ status }} {{ port }}".to_string(),
        ]),
        seed: Some(42),
        ..Config::default()
    }
}

fn bench_legacy(c: &mut Criterion) {
    let config = bench_config_legacy();
    let gen = Generator::new(config);
    c.bench_function("legacy_100k", |b| {
        b.iter(|| black_box(gen.generate()));
    });
}

fn bench_template_simple(c: &mut Criterion) {
    let config = bench_config_template_simple();
    let gen = Generator::new(config);
    c.bench_function("template_simple_100k", |b| {
        b.iter(|| black_box(gen.generate()));
    });
}

fn bench_template_random(c: &mut Criterion) {
    let config = bench_config_template_random();
    let gen = Generator::new(config);
    c.bench_function("template_random_100k", |b| {
        b.iter(|| black_box(gen.generate()));
    });
}

fn bench_parallel(c: &mut Criterion) {
    let config = bench_config_parallel();
    let gen = Generator::new(config);
    c.bench_function("parallel_500k", |b| {
        b.iter(|| black_box(gen.generate()));
    });
}

criterion_group!(
    benches,
    bench_legacy,
    bench_template_simple,
    bench_template_random,
    bench_parallel,
);
criterion_main!(benches);
