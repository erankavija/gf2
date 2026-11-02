use std::collections::BTreeMap;

pub fn arguments(allowed: &[&str], required: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut args = std::env::args().skip(1);
    let mut values = BTreeMap::new();
    while let Some(key) = args.next() {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("unknown argument {key}"));
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {key}"))?;
        if values.insert(key.clone(), value).is_some() {
            return Err(format!("duplicate argument {key}"));
        }
    }
    for &key in required {
        if !values.contains_key(key) {
            return Err(format!("missing {key}"));
        }
    }
    Ok(values)
}
pub fn count(values: &BTreeMap<String, String>, key: &str) -> Result<Option<usize>, String> {
    values
        .get(key)
        .map(|v| {
            v.parse::<usize>()
                .map_err(|_| format!("invalid {key}"))
                .and_then(|n| {
                    if n > 0 {
                        Ok(n)
                    } else {
                        Err(format!("{key} must be positive"))
                    }
                })
        })
        .transpose()
}
