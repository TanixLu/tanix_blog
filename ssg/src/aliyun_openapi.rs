use anyhow::bail;
use hmac::{Hmac, KeyInit as _, Mac};
use sha2::{Digest, Sha256};

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("hmac key");
    mac.update(data);
    mac.finalize().into_bytes().to_vec()
}

/// RFC3986 编码：仅 A-Z a-z 0-9 - _ . ~ 保持原样，其余转 %XX（大写）。
fn pe(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

/// 规范化 URI：按 `/` 分段，每段单独编码，斜杠本身保留。
/// FC 是 ROA/FC 风格，CanonicalURI 取 path 的值（不是 "/"）。
fn canonical_uri(path: &str) -> String {
    path.split('/').map(pe).collect::<Vec<_>>().join("/")
}

/// 规范化查询串：按参数名升序，名/值分别编码，= 连接，& 拼接。无则空串。
fn canonical_query(query: &[(&str, &str)]) -> String {
    let mut q: Vec<(String, String)> = query.iter().map(|(k, v)| (pe(k), pe(v))).collect();
    q.sort();
    q.iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect::<Vec<_>>()
        .join("&")
}

/// 计算 Authorization 头。headers 里必须已含 x-acs-content-sha256。
fn build_authorization(
    access_key_id: &str,
    access_key_secret: &str,
    method: &str,
    path: &str,
    query: &[(&str, &str)],
    headers: &[(&str, &str)],
    body: &[u8],
) -> String {
    let hashed_payload = sha256_hex(body);

    // 只挑 host / content-type / x-acs-* 参与签名；名小写、值 trim、按名排序。
    let mut hs: Vec<(String, String)> = headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), v.trim().to_string()))
        .filter(|(k, _)| k == "host" || k == "content-type" || k.starts_with("x-acs-"))
        .collect();
    hs.sort();
    let canonical_headers: String = hs.iter().map(|(k, v)| format!("{}:{}\n", k, v)).collect();
    let signed_headers = hs
        .iter()
        .map(|(k, _)| k.clone())
        .collect::<Vec<_>>()
        .join(";");

    let canonical_request = format!(
        "{}\n{}\n{}\n{}\n{}\n{}",
        method,
        canonical_uri(path),
        canonical_query(query),
        canonical_headers,
        signed_headers,
        hashed_payload
    );

    let hashed_canonical_request = sha256_hex(canonical_request.as_bytes());
    let string_to_sign = format!("ACS3-HMAC-SHA256\n{}", hashed_canonical_request);
    let signature = hex::encode(hmac_sha256(
        access_key_secret.as_bytes(),
        string_to_sign.as_bytes(),
    ));

    format!(
        "ACS3-HMAC-SHA256 Credential={},SignedHeaders={},Signature={}",
        access_key_id, signed_headers, signature
    )
}

fn nonce_hex() -> String {
    let b: [u8; 16] = rand::random();
    hex::encode(b)
}

/// 对应 python 的 fc_upload：UpdateFunction，上传新的 zip 代码包。
pub fn fc_upload(
    aliyun_id: &str,
    aliyun_access_key_id: &str,
    aliyun_access_key_secret: &str,
    region: &str,
    function_name: &str,
    zip_base64: &str,
) -> anyhow::Result<()> {
    let host = format!("{}.{}.fc.aliyuncs.com", aliyun_id, region);
    let path = format!("/2023-03-30/functions/{}", function_name);

    let body = format!(r#"{{"code": {{"zipFile": "{}"}}}}"#, zip_base64);
    let body_bytes = body.as_bytes();
    let content_sha = sha256_hex(body_bytes);

    let date = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let nonce = nonce_hex();

    // 这些 header 都会进签名（host/content-type/x-acs-*）。
    let headers: Vec<(&str, &str)> = vec![
        ("host", host.as_str()),
        ("x-acs-action", "UpdateFunction"),
        ("x-acs-version", "2023-03-30"),
        ("x-acs-date", date.as_str()),
        ("x-acs-content-sha256", content_sha.as_str()),
        ("x-acs-signature-nonce", nonce.as_str()),
        ("content-type", "application/json"),
        // STS 临时凭证时再加： ("x-acs-security-token", token)
    ];

    let authorization = build_authorization(
        aliyun_access_key_id,
        aliyun_access_key_secret,
        "PUT",
        &path,
        &[],
        &headers,
        body_bytes,
    );

    let url = format!("https://{}{}", host, path);
    let client = reqwest::blocking::Client::new();
    let mut req = client.put(&url).body(body.clone());
    for (k, v) in &headers {
        req = req.header(*k, *v);
    }
    req = req.header("Authorization", authorization);

    let resp = req.send()?;
    let status = resp.status();
    if !status.is_success() {
        let text = resp.text()?;
        bail!("UpdateFunction failed: HTTP {} - {}", status, text);
    }
    Ok(())
}
