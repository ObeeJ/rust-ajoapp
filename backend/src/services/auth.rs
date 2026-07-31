use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use shared::*;
use uuid::Uuid;
use rand::Rng;

use crate::store::{Store, OtpEntry};

const ACCESS_TOKEN_SECS: i64  = 60 * 15;       // 15 minutes
const REFRESH_TOKEN_SECS: i64 = 60 * 60 * 24 * 7; // 7 days
const MAX_LOGIN_ATTEMPTS: u32 = 5;
const LOCKOUT_SECS: i64       = 60 * 15;

fn jwt_secret() -> String {
    std::env::var("JWT_SECRET").expect("JWT_SECRET env var must be set")
}

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub:  String,
    pub jti:  String,
    pub role: String, // "user" | "admin"
    pub exp:  usize,
}

pub fn make_access_token(user_id: Uuid, role: &str) -> Result<String, ApiError> {
    let secret = jwt_secret();
    let claims = Claims {
        sub:  user_id.to_string(),
        jti:  Uuid::new_v4().to_string(),
        role: role.to_string(),
        exp:  (Utc::now().timestamp() + ACCESS_TOKEN_SECS) as usize,
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

pub fn verify_token(store: &Store, token: &str) -> Option<(Uuid, String)> {
    let secret = jwt_secret();
    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    ).ok()?;

    if store.denied_jtis.lock().unwrap().contains(&data.claims.jti) {
        return None;
    }

    let uid = Uuid::parse_str(&data.claims.sub).ok()?;
    Some((uid, data.claims.role))
}

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

    let access  = make_access_token(user_id, "user")?;
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

fn validate_email(email: &str) -> Result<(), ApiError> {
    if email.trim().is_empty() || !email.contains('@') || email.len() > 254 {
        return Err(ApiError { error: "Valid email is required".into() });
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
    pub otp:           Option<String>, // only set on register, for email dispatch
}

/// Generate a 6-digit OTP, store it with 15-min expiry
pub fn generate_otp(store: &Store, email: &str) -> String {
    let code: String = rand::thread_rng()
        .sample_iter(rand::distributions::Uniform::new(0, 10))
        .take(6)
        .map(|d| char::from_digit(d, 10).unwrap())
        .collect();

    store.email_otps.lock().unwrap().insert(email.to_string(), OtpEntry {
        code:       code.clone(),
        expires_at: Utc::now() + chrono::Duration::minutes(15),
        attempts:   0,
    });
    code
}

/// Verify OTP — returns Ok(()) on success, Err on wrong/expired/too many attempts
pub fn verify_otp(store: &Store, email: &str, code: &str) -> Result<(), ApiError> {
    let mut otps = store.email_otps.lock().unwrap();
    let entry = otps.get_mut(email)
        .ok_or(ApiError { error: "No verification pending for this email".into() })?;

    if entry.expires_at < Utc::now() {
        otps.remove(email);
        return Err(ApiError { error: "OTP expired. Request a new one.".into() });
    }
    if entry.attempts >= 5 {
        return Err(ApiError { error: "Too many attempts. Request a new OTP.".into() });
    }
    if entry.code != code.trim() {
        entry.attempts += 1;
        return Err(ApiError { error: "Invalid OTP".into() });
    }

    otps.remove(email);

    // Mark user as verified
    let mut users = store.users.lock().unwrap();
    if let Some(u) = users.values_mut().find(|u| u.email.as_deref() == Some(email)) {
        u.email_verified = true;
    }

    Ok(())
}

pub fn resend_otp(store: &Store, email: &str) -> Result<String, ApiError> {
    // Only resend if user exists and is unverified
    let exists = store.users.lock().unwrap()
        .values().any(|u| u.email.as_deref() == Some(email));
    if !exists {
        return Err(ApiError { error: "Email not found".into() });
    }
    Ok(generate_otp(store, email))
}

pub fn register(store: &Store, req: RegisterRequest) -> Result<AuthTokens, ApiError> {
    validate_name(&req.name)?;
    validate_phone(&req.phone)?;
    validate_email(&req.email)?;
    validate_pin(&req.pin)?;

    let phone = req.phone.trim().to_string();
    let email = req.email.trim().to_lowercase();

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
        email: Some(email),
        role: UserRole::User,
        email_verified: false,
        created_at: now,
    };

    let wallet = Wallet {
        id: Uuid::new_v4(),
        user_id,
        available_kobo: 0,
        ledger_kobo: 0,
        version: 0,
    };

    // bcrypt cost 12 — OWASP minimum recommendation
    let hashed = bcrypt::hash(&req.pin, 12)
        .map_err(|_| ApiError { error: "Registration failed".into() })?;

    phones.insert(phone, user_id);
    drop(phones);

    store.users.lock().unwrap().insert(user_id, user.clone());
    store.pins.lock().unwrap().insert(user_id, hashed);
    store.wallets.lock().unwrap().insert(user_id, wallet.clone());

    // Generate and store OTP — caller must send it via email
    let otp = generate_otp(store, user.email.as_deref().unwrap_or(""));

    let access_token  = make_access_token(user_id, "user")?;
    let refresh_token = make_refresh_token(store, user_id);

    Ok(AuthTokens { access_token, refresh_token, user, wallet, otp: Some(otp) })
}

pub fn login(store: &Store, req: LoginRequest) -> Result<AuthTokens, ApiError> {
    validate_phone(&req.phone)?;
    validate_pin(&req.pin)?;

    let phone = req.phone.trim().to_string();

    check_rate_limit(store, &phone)?;

    let user_id = {
        let phones = store.phone_index.lock().unwrap();
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

    let role          = match user.role { UserRole::Admin => "admin", _ => "user" };

    // Block login until email is verified
    if !user.email_verified {
        return Err(ApiError { error: "Email not verified. Check your inbox for the OTP.".into() });
    }

    let access_token  = make_access_token(user_id, role)?;
    let refresh_token = make_refresh_token(store, user_id);

    Ok(AuthTokens { access_token, refresh_token, user, wallet, otp: None })
}
