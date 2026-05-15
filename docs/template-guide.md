# Template & Variable Guide

## Template Engine

loggen uses the **Tera** template engine (Jinja2-inspired). Templates are plain text with `{{ variable }}` placeholders and optional filters.

### Basic Syntax

```
{{ variable }}
{{ variable | filter }}
{{ variable | filter(args) }}
{% if condition %}...{% endif %}
{% for item in list %}...{% endfor %}
```

## Built-in Variables

These variables are automatically available in all templates:

| Variable | Type | Description |
|----------|------|-------------|
| `{{ timestamp }}` | `DateTime` | Current UTC timestamp. Use with `date` filter for formatting |
| `{{ level }}` | `String` | Log level from config (`log_level`) or CLI `--level` |
| `{{ index }}` | `u64` | 1-based entry counter within the generation run |
| `{{ message }}` | `String` | Message from config (`message`) or CLI `--message` |

## Auto-Random Variables

These variables auto-generate random values per entry when used in a template
(no configuration needed):

| Variable | Description | Example Output |
|----------|-------------|----------------|
| `{{ ip }}` / `{{ ipv4 }}` | Random IPv4 address | `192.168.1.42` |
| `{{ ipv6 }}` | Random IPv6 address | `fe80:dead:beef:cafe:0:1:2:3` |
| `{{ user_agent }}` | Random User-Agent string | `Mozilla/5.0 ... Chrome/120.0...` |
| `{{ email }}` | Random email address | `alice@example.com` |
| `{{ url }}` | Random URL path | `/api/v1/users` |
| `{{ port }}` | Random port number | `443` |
| `{{ status }}` | Random HTTP status code (weighted) | `200`, `404`, `500` |
| `{{ user }}` | Random username | `admin`, `bob`, `charlie` |

## Tera Filters

The most commonly used filter is `date` for timestamp formatting:

```
{{ timestamp | date(format="%Y-%m-%d") }}           → 2026-05-15
{{ timestamp | date(format="%d/%b/%Y:%H:%M:%S %z") }} → 15/May/2026:12:00:00 +0000
{{ timestamp | date(format="%b %d %H:%M:%S") }}     → May 15 12:00:00
```

Format specifiers follow the chrono `strftime` conventions:
`%Y` (year), `%m` (month), `%d` (day), `%H` (hour), `%M` (minute),
`%S` (second), `%b` (abbreviated month), `%z` (timezone offset).

## Randomization Intensity

The `random_intensity` field (0.0–1.0) controls how often auto-random variables
get fresh values:

- **1.0** (default): Every auto-variable gets a new random value per entry.
- **0.5**: ~50% chance per variable per entry that it changes (otherwise keeps
  the previous entry's value).
- **0.0**: No randomization — values are generated once and reused.

## Template Rotation

When multiple templates are configured (via `logs` or a template directory),
the `template_rotation` field controls selection:

| Mode | Behavior |
|------|----------|
| `"sequential"` (default) | Cycle through templates in order, repeat from start |
| `"random"` | Pick a random template per entry |
| `"round_robin"` | Cycle through templates in order, one per entry |

## Custom Random Pools

Define your own random variable pools in config:

```yaml
random_vars:
  codes: [200, 201, 404, 500]
  methods: [GET, POST, PUT, DELETE]
```

Then use them in templates as `{{ codes }}` or `{{ methods }}`.

## Custom Template Variables

Define static variables in config:

```yaml
template_vars:
  app_name: myapp
  host: web01
```

Or via CLI: `--var app_name=myapp --var host=web01`.

## Validation

Any `{{ variable }}` used in a template must be defined in one of:
1. `template_vars` in config
2. CLI `--var` arguments
3. Built-in variables (`timestamp`, `level`, `index`, `message`)
4. Auto-random variables (`ipv4`, `user_agent`, etc.)
5. Custom `random_vars` pools

Unknown variables cause a startup error (panic).
