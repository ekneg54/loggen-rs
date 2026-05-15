# Attack Scenario Gallery

loggen supports three attack pattern types for generating realistic security
event data. Below is a summary of the built-in example configurations.

## Attack Types

| Type | Description | Use Case |
|------|-------------|----------|
| `single_event` | Each entry is independently rendered from one template | Brute force, credential stuffing, any repeated pattern |
| `multi_ordered` | Templates rendered in sequence from an ordered list | Port scans, SQL injection probes, staged attacks |
| `threshold_field` | Rejection sampling to bias a numeric field into a bucket | DDoS ramp-up, error rate simulation |

## Example Configs

### Brute Force Login (`attack-brute-force.yaml`)

- **Type:** `single_event`
- **Simulates:** Repeated POST /login from a fixed IP
- **Key characteristics:** Mostly 401 responses, occasional 200 (breach)
- **Vars:** Weighted status (`401` x4, `200` x1), fixed attacker IP
- **Usage:** `cargo run -- generate -c examples/attack-brute-force.yaml`

### Port Scan (`attack-port-scan.yaml`)

- **Type:** `multi_ordered`
- **Simulates:** Sequential port scan with nmap User-Agent
- **Key characteristics:** CONNECT probes to ports 22, 80, 443, 8080, 3306
- **Vars:** Fixed scanner IP, repeat: `loop` for continuous scanning
- **Usage:** `cargo run -- generate -c examples/attack-port-scan.yaml`

### DDoS Ramp-up (`attack-ddos.yaml`)

- **Type:** `threshold_field`
- **Simulates:** DDoS attack with rotating IPs, biased toward 5xx errors
- **Key characteristics:** 70% of entries have `status >= 500`
- **Threshold:** `field: status, min: 500, proportion: 0.7`
- **Usage:** `cargo run -- generate -c examples/attack-ddos.yaml`

### SQL Injection Probe (`attack-sqli-probe.yaml`)

- **Type:** `multi_ordered`
- **Simulates:** Progressive SQL injection probing
- **Sequence stages:** Normal → OR 1=1 → UNION SELECT → DROP TABLE
- **Vars:** Fixed attacker IP, repeat: `loop`
- **Usage:** `cargo run -- generate -c examples/attack-sqli-probe.yaml`

### Credential Stuffing (`attack-credential-stuffing.yaml`)

- **Type:** `single_event`
- **Simulates:** Distributed credential stuffing from a single bot
- **Key characteristics:** `common` fields freeze `ipv4` and `port`, cycled usernames
- **Vars:** Weighted status (mostly 401/403), cycled usernames
- **Usage:** `cargo run -- generate -c examples/attack-credential-stuffing.yaml`

## Var Modes

Attack vars support three selection modes:

| Mode | Description | Example |
|------|-------------|---------|
| `random` (default) | Uniform random selection from pool | Equal chance of any value |
| `cycle` | Sequential wrap-around | `[a, b, c]` → a, b, c, a, b, c... |
| `weighted` | First values have higher probability | `[a, b, c]` → a: 50%, b: 33%, c: 17% |

## Common Fields

The `common` field freezes specified variables after the first entry, making
them persistent across the entire attack run. Useful for simulating a single
attacker identity (fixed IP, fixed port, etc.).

## Interleaving

When `interleave: true`, attack entries are randomly mixed with normal entries
during generation. The `weight` field (0.0–1.0) controls how often the attack
stream is selected relative to other active streams.
