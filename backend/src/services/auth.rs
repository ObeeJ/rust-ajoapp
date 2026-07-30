use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use shared::*;
use uuid::Uuid;

use crate::store::Store;

const ACCESS_TOKEN_SECS: i64  = 60 * 15;       // 15 minutes
const REFRESH_TOKEN_SECS: i64 = 60 * 60 * 24 * 7; // 7 days
const MAX_LOGIN_ATTEMPTS: u32 = 5;
const LOCKOUT_SECS: i64       = 60 * 15;

fn jwt_secret() -> String {
    std::env::var("JWT_SECRET").expect("JWT_SECRET env var must be set")
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub jti: String, // unique token id
    pub exp: usize,
}

pub fn make_access_token(user_id: Uuid) -> Result<String, ApiError> {
    let secret = jwt_secret();
    let claims = Claims {
        sub: user_id.to_string(),
        jti: Uuid::new_v4().to_string(),
        exp: (Utc::now().timestamp() + ACCESS_TOKEN_SECS) as usize,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|_| ApiError { error: "Token generation failed".into() })
}

pub fn make_refresh_token(store: &Store, user_id: Uuid) -> String {
    let token = Uuid::new_v4().to_string();
    let expires_at = Utc::now() + chrono::Duration::seconds(REFRESH_TOKEN_SECS);
    store.refresh_tokens.lock().unwrap().insert(
        token.clone(),
        crate::store::RefreshEntry { user_id, expires_at, used: false },
    );
    token
}

pub fn verify_token(store: &Store, token: &str) -> Option<Uuid> {
    let secret = jwt_secret();
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).ok()?;

    // Check JTI deny-list (logout invalidation)
    if store.denied_jtis.lock().unwrap().contains(&data.claims.jti) {
        return None;
    }

    Uuid::parse_str(&data.claims.sub).ok()
}

/// Extract JTI from a token without full verification (for logout)
pub fn extract_jti(token: &str) -> Option<String> {
    let secret = jwt_secret();
    decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).ok().map(|d| d.claims.jti)
}

/// Rotate refresh token — single-use, returns new pair
pub fn rotate_refresh_token(store: &Store, old_token: &str) -> Result<(String, String), ApiError> {
    let user_id = {
        let mut tokens = store.refresh_tokens.lock().unwrap();
        let entry = tokens
            .get_mut(old_token)
            .ok_or(ApiError { error: "Invalid refresh token".into() })?;

        if entry.used {
            return Err(ApiError { error: "Refresh token already used".into() });
        }
        if entry.expires_at < Utc::now() {
            return Err(ApiError { error: "Refresh token expired".into() });
        }
        entry.used = true;
        entry.user_id
    };

    let access  = make_access_token(user_id)?;
    let refresh = make_refresh_token(store, user_id);
    Ok((access, refresh))
}

fn check_rate_limit(store: &Store, key: &str) -> Result<(), ApiError> {
    let mut attempts = store.login_attempts.lock().unwrap();
    let entry = attempts.entry(key.to_string()).or_insert(crate::store::LoginAttempts {
        count: 0,
        locked_until: None,
    });

    if let Some(locked_until) = entry.locked_until {
        if Utc::now() < locked_until {
            return Err(ApiError { error: "Too many attempts. Try again later.".into() });
        }
        // Lockout expired — reset
        entry.count = 0;
        entry.locked_until = None;
    }
    Ok(())
}

fn record_failed_attempt(store: &Store, key: &str) {
    let mut attempts = store.login_attempts.lock().unwrap();
    let entry = attempts.entry(key.to_string()).or_insert(crate::store::LoginAttempts {
        count: 0,
        locked_until: None,
    });
    entry.count += 1;
    if entry.count >= MAX_LOGIN_ATTEMPTS {
        entry.locked_until = Some(Utc::now() + chrono::Duration::seconds(LOCKOUT_SECS));
    }
}

fn clear_attempts(store: &Store, key: &str) {
    store.login_attempts.lock().unwrap().remove(key);
}

fn validate_phone(phone: &str) -> Result<(), ApiError> {
    let digits: String = phone.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() < 10 || digits.len() > 15 {
        return Err(ApiError { error: "Invalid phone number".into() });
    }
    Ok(())
}

fn validate_pin(pin: &str) -> Result<(), ApiError> {
    if pin.len() < 4 || pin.len() > 6 || !pin.chars().all(|c| c.is_ascii_digit()) {
        return Err(ApiError { error: "PIN must be 4–6 digits".into() });
    }
    Ok(())
}

fn validate_name(name: &str) -> Result<(), ApiError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return Err(ApiError { error: "Name must be 1–100 characters".into() });
    }
    Ok(())
}

#[derive(Debug)]
pub struct AuthTokens {
    pub access_token:  String,
    pub refresh_token: String,
    pub user:          User,
    pub wallet:        Wallet,
}

pub fn register(store: &Store, req: RegisterRequest) -> Result<AuthTokens, ApiError> {
    validate_name(&req.name)?;
    validate_phone(&req.phone)?;
    validate_pin(&req.pin)?;

    let phone = req.phone.trim().to_string();

    let mut phones = store.phone_index.lock().unwrap();
    if phones.contains_key(&phone) {
        return Err(ApiError { error: "Phone already registered".into() });
    }

    let user_id = Uuid::new_v4();
    let now = Utc::now();

    let user = User {
        id: user_id,
        name: req.name.trim().to_string(),
        phone: phone.clone(),
        email: req.email.filter(|e| !e.is_empty()),
        created_at: now,
    };

    let wallet = Wallet {
        id: Uuid::new_v4(),
        user_id,
        balance_kobo: 0,
        ledger_balance_kobo: 0,
    };

    // bcrypt cost 12 — OWASP minimum recommendation
    let hashed = bcrypt::hash(&req.pin, 12)
        .map_err(|_| ApiError { error: "Registration failed".into() })?;

    phones.insert(phone, user_id);
    drop(phones);

    store.users.lock().unwrap().insert(user_id, user.clone());
    store.pins.lock().unwrap().insert(user_id, hashed);
    store.wallets.lock().unwrap().insert(user_id, wallet.clone());

    let access_token  = make_access_token(user_id)?;
    let refresh_token = make_refresh_token(store, user_id);

    Ok(AuthTokens { access_token, refresh_token, user, wallet })
}

pub fn login(store: &Store, req: LoginRequest) -> Result<AuthTokens, ApiError> {
    validate_phone(&req.phone)?;
    validate_pin(&req.pin)?;

    let phone = req.phone.trim().to_string();

    // Rate limit keyed on phone
    check_rate_limit(store, &phone)?;

    let user_id = {
        let phones = store.phone_index.lock().unwrap();
        // Return same error whether phone or PIN is wrong — no enumeration
        phones.get(&phone).copied()
            .ok_or(ApiError { error: "Invalid phone or PIN".into() })?
    };

    let hashed = store.pins.lock().unwrap().get(&user_id).cloned()
        .ok_or(ApiError { error: "Invalid phone or PIN".into() })?;

    if !bcrypt::verify(&req.pin, &hashed).unwrap_or(false) {
        record_failed_attempt(store, &phone);
        return Err(ApiError { error: "Invalid phone or PIN".into() });
    }

    clear_attempts(store, &phone);

    let user   = store.users.lock().unwrap().get(&user_id).cloned()
        .ok_or(ApiError { error: "User not found".into() })?;
    let wallet = store.wallets.lock().unwrap().get(&user_id).cloned()
        .ok_or(ApiError { error: "Wallet not found".into() })?;

    let access_token  = make_access_token(user_id)?;
    let refresh_token = make_refresh_token(store, user_id);

    Ok(AuthTokens { access_token, refresh_token, user, wallet })
}
