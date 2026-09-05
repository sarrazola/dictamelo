//! Verify one local preview artifact without creating or publishing an updater manifest.
use base64::{engine::general_purpose::STANDARD, Engine};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "Usage: verify_artifact ARTIFACT SIGNATURE");
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).expect("Valid Tauri config");
    let key_text = String::from_utf8(
        STANDARD.decode(config["plugins"]["updater"]["pubkey"].as_str().unwrap()).unwrap(),
    ).unwrap();
    let key = minisign_verify::PublicKey::from_base64(key_text.lines().nth(1).unwrap()).unwrap();
    let encoded = std::fs::read_to_string(&args[2]).expect("Read signature");
    let signature_text = String::from_utf8(STANDARD.decode(encoded.trim()).unwrap()).unwrap();
    let signature = minisign_verify::Signature::decode(&signature_text).unwrap();
    let bytes = std::fs::read(&args[1]).expect("Read artifact");
    key.verify(&bytes, &signature, false).expect("Invalid updater signature");
    println!("Updater signature verified ({} bytes)", bytes.len());
}
