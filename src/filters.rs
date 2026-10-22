use chrono::{DateTime, FixedOffset, Utc};
use tera::{Error, Kwargs, State, Tera, TeraResult, Value};

/// Registers the filters and functions that Tera 2 no longer ships with.
///
/// Tera 2 moved a couple of the built-in filters of Tera 1 into `tera-contrib`
/// and dropped the rest. loggen templates rely on the Tera 1 names, so they are
/// re-registered here under the same names to keep existing templates working.
pub fn register_tera_extensions(tera: &mut Tera) {
    tera.register_filter("date", date_filter);
    tera.register_filter("addslashes", addslashes_filter);
    tera.register_filter("json_encode", tera_contrib::json::json_encode);
    tera.register_filter("urlencode", tera_contrib::urlencode::urlencode);
    tera.register_filter("slugify", tera_contrib::slug::slug);
    tera.register_filter("striptags", tera_contrib::regex::striptags);
    tera.register_filter("spaceless", tera_contrib::regex::spaceless);
    tera.register_filter("filesizeformat", tera_contrib::filesize_format::filesize_format);

    tera.register_function("get_random", tera_contrib::rand::get_random);
    tera.register_function("now", now_function);
}

/// Formats a RFC 3339 timestamp with a strftime-like format string.
///
/// `{{ timestamp | date(format="%Y-%m-%d") }}`
fn date_filter(val: &Value, kwargs: Kwargs, _state: &State) -> TeraResult<String> {
    let format = kwargs
        .get::<String>("format")?
        .unwrap_or_else(|| "%Y-%m-%d".to_string());
    let dt = parse_datetime(val)?;
    Ok(dt.format(&format).to_string())
}

/// Returns the current time as RFC 3339 timestamp.
///
/// `{{ now() }}`
fn now_function(_kwargs: Kwargs, _state: &State) -> TeraResult<Value> {
    Ok(Value::from(Utc::now().to_rfc3339()))
}

/// Escapes quotes, backslashes and NUL bytes.
///
/// `{{ value | addslashes }}`
fn addslashes_filter(val: &str, _kwargs: Kwargs, _state: &State) -> String {
    let mut out = String::with_capacity(val.len());
    for c in val.chars() {
        match c {
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\0' => out.push_str("\\0"),
            _ => out.push(c),
        }
    }
    out
}

/// Parses a timestamp that is either a RFC 3339 string or a unix timestamp.
fn parse_datetime(val: &Value) -> TeraResult<DateTime<FixedOffset>> {
    if let Some(s) = val.as_str() {
        return DateTime::parse_from_rfc3339(s).map_err(|e| {
            Error::message(format!(
                "date filter: '{}' is not a valid RFC 3339 timestamp: {}",
                s, e
            ))
        });
    }
    if let Some(ts) = val.as_i64() {
        return DateTime::from_timestamp(ts, 0)
            .map(|dt| dt.fixed_offset())
            .ok_or_else(|| Error::message(format!("date filter: invalid unix timestamp {}", ts)));
    }
    Err(Error::message(
        "date filter: expected a RFC 3339 timestamp or a unix timestamp",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tera::Context;

    fn render(template: &str, ctx: &Context) -> String {
        let mut tera = Tera::default();
        register_tera_extensions(&mut tera);
        tera.add_raw_template("t", template).unwrap();
        tera.render("t", ctx).unwrap()
    }

    #[test]
    fn test_date_filter_default_format() {
        let mut ctx = Context::new();
        ctx.insert("timestamp", "2024-05-17T08:09:10+00:00");
        assert_eq!(render("{{ timestamp | date }}", &ctx), "2024-05-17");
    }

    #[test]
    fn test_date_filter_custom_format() {
        let mut ctx = Context::new();
        ctx.insert("timestamp", "2024-05-17T08:09:10+00:00");
        let out = render("{{ timestamp | date(format=\"%d/%b/%Y:%H:%M:%S %z\") }}", &ctx);
        assert_eq!(out, "17/May/2024:08:09:10 +0000");
    }

    #[test]
    fn test_date_filter_invalid_input() {
        let mut ctx = Context::new();
        ctx.insert("timestamp", "not-a-date");
        let mut tera = Tera::default();
        register_tera_extensions(&mut tera);
        tera.add_raw_template("t", "{{ timestamp | date }}").unwrap();
        assert!(tera.render("t", &ctx).is_err());
    }

    #[test]
    fn test_addslashes_filter() {
        let mut ctx = Context::new();
        ctx.insert("value", "it's a \"test\"");
        assert_eq!(
            render("{{ value | addslashes }}", &ctx),
            "it\\'s a \\\"test\\\""
        );
    }

    #[test]
    fn test_tera1_filter_names_are_available() {
        let mut tera = Tera::default();
        register_tera_extensions(&mut tera);
        for name in [
            "date",
            "addslashes",
            "json_encode",
            "urlencode",
            "slugify",
            "striptags",
            "spaceless",
            "filesizeformat",
        ] {
            let template = format!("{{{{ value | {}(value=\"x\") }}}}", name);
            assert!(
                tera.add_raw_template(name, &template).is_ok(),
                "filter '{}' is not registered",
                name
            );
        }
    }
}
