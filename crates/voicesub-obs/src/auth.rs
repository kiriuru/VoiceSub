use base64::{Engine as _, engine::general_purpose::STANDARD as B64};
use sha2::{Digest, Sha256};

pub fn build_auth_response(password: &str, salt: &str, challenge: &str) -> String {
    let secret = Sha256::digest(format!("{password}{salt}"));
    let secret_b64 = B64.encode(secret);
    let challenge_digest = Sha256::digest(format!("{secret_b64}{challenge}"));
    B64.encode(challenge_digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_response_is_deterministic() {
        let first = build_auth_response("secret", "salt", "challenge");
        let second = build_auth_response("secret", "salt", "challenge");
        assert_eq!(first, second);
        assert!(!first.is_empty());
    }
}
