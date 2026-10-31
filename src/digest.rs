use sha2::{Digest, Sha256};

pub fn bytes(payload: impl AsRef<[u8]>) -> String {
    let mut h = Sha256::new();
    h.update(payload.as_ref());
    format!("sha256:{}", hex(&h.finalize()))
}

pub fn tagged(tag: &str, payload: impl AsRef<[u8]>) -> String {
    let mut h = Sha256::new();
    h.update(tag.as_bytes());
    h.update(b":");
    h.update(payload.as_ref());
    format!("sha256:{}", hex(&h.finalize()))
}

pub fn canonical_json(value: &serde_json::Value) -> String {
    // Minimal canonical form: sorted keys, no whitespace. serde_json::to_string
    // already sorts? No: use Value's ordering. We sort object keys manually.
    canonical_value(value)
}

fn canonical_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => serde_json::to_string(s).unwrap_or_default(),
        serde_json::Value::Array(a) => {
            format!(
                "[{}]",
                a.iter().map(canonical_value).collect::<Vec<_>>().join(",")
            )
        }
        serde_json::Value::Object(m) => {
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let parts: Vec<String> = keys
                .iter()
                .map(|k| {
                    format!(
                        "{}:{}",
                        serde_json::to_string(k).unwrap_or_default(),
                        canonical_value(&m[*k])
                    )
                })
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

pub fn normalize_text(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for line in input.lines() {
        out.push_str(line.trim_end());
        out.push('\n');
    }
    // collapse trailing blank lines to a single newline
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') && !out.is_empty() {
        out.push('\n');
    }
    out
}

pub fn short(digest: &str) -> &str {
    &digest[..digest.len().min(12)]
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}
