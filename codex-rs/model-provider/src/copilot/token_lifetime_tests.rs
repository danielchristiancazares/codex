use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use pretty_assertions::assert_eq;
use serde_json::json;

use super::*;

#[test]
fn refresh_delay_uses_eighty_percent_of_credential_lifetime() {
    assert_eq!(
        refresh_delay(Some(100), Some(1_100), 100),
        Some(Duration::from_secs(800))
    );
    assert_eq!(
        refresh_delay(Some(100), Some(1_100), 500),
        Some(Duration::from_secs(400))
    );
    assert_eq!(
        refresh_delay(/*issued_at*/ None, Some(1_100), 100),
        Some(Duration::from_secs(800))
    );
    assert_eq!(
        refresh_delay(Some(100), Some(103), 100),
        Some(Duration::from_secs(3))
    );
}

#[test]
fn reads_lifetime_from_copilot_jwt() {
    let claims = json!({"iat": 100, "exp": 1_100});
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).expect("encode claims"));
    let token = format!("header.{payload}.signature");

    assert_eq!(
        jwt_lifetime(&token),
        Some(JwtLifetime {
            iat: Some(100),
            exp: Some(1_100),
        })
    );
}
