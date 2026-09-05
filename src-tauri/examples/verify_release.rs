//! Verify all staged updater artifacts against the public key embedded in the app.
use std::path::PathBuf;
fn main() {
    let dir = PathBuf::from(std::env::args().nth(1).expect("Pass the release staging directory"));
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("latest.json")).unwrap()).unwrap();
    let config: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    let raw = String::from_utf8(base64_decode(config["plugins"]["updater"]["pubkey"].as_str().unwrap()).unwrap()).unwrap();
    let key = minisign_verify::PublicKey::from_base64(raw.lines().nth(1).unwrap()).unwrap();
    let platforms = manifest["platforms"].as_object().unwrap();
    for required in ["darwin-aarch64", "windows-x86_64", "windows-aarch64"] { assert!(platforms.contains_key(required), "Missing {required}"); }
    for (platform, artifact) in platforms {
        let name = artifact["url"].as_str().unwrap().rsplit('/').next().unwrap();
        let bytes = std::fs::read(dir.join(name)).unwrap();
        assert!(bytes.len() > 1_000_000, "Suspiciously small artifact");
        let sig = String::from_utf8(base64_decode(artifact["signature"].as_str().unwrap()).unwrap()).unwrap();
        key.verify(&bytes, &minisign_verify::Signature::decode(&sig).unwrap(), false).expect("Invalid updater signature");
        println!("{platform}: signature verified ({} bytes)", bytes.len());
    }
}
    fn base64_decode(input: &str) -> Option<Vec<u8>> {
        const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut buffer = 0u32;
        let mut bits = 0u32;
        let mut out = Vec::new();
        for byte in input.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=') {
            let value = TABLE.iter().position(|c| *c == byte)? as u32;
            buffer = (buffer << 6) | value;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buffer >> bits) as u8);
            }
        }
        Some(out)
    }
