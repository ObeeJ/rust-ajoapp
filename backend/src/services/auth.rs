use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use shared::*;
use uuid::Uuid;

use crate::store::Store;

const JWT_SECRET: &str = "ajo_secret_change_in_prod";

#[derive(Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: usize,
}

pub fn make_token(user_id: Uuid) -> String {
    let claims = Claims {
        sub: user_id.to_string(),
        exp: (Utc::now().timestamp() + 86400 * 7) as usize, // 7 days
    };
    encode(&Header::default(), &claims, &EncodingKey::from_secret(JWT_SECRET.as_bytes())).unwrap()
}

pub fn verify_token(token: &str) -> Option<Uuid> {
    decode::<Claims>(token, &DecodingKey::from_secret(JWT_SECRET.as_bytes()), &Validation::default())
        .ok()
        .and_then(|d| Uuid::parse_str(&d.claims.sub).ok())
}

pub fn register(store: &Store, req: RegisterRequest) -> Result<AuthResponse, ApiError> {
    let mut phones = store.phone_index.lock().unwrap();
    if phones.contains_key(&req.phone) {
        return Err(ApiError { error: "Phone already registered".into() });
    }

    let user_id = Uuid::new_v4();
    let now = Utc::now();

    let user = User {
        id: user_id,
        name: req.name,
        phone: req.phone.clone(),
        email: req.email,
        created_at: now,
    };

    let wallet = Wallet {
        id: Uuid::new_v4(),
        user_id,
        balance_kobo: 0,
        ledger_balance_kobo: 0,
    };

    let hashed = bcrypt::hash(&req.pin, 10).unwrap();

    phones.insert(req.phone, user_id);
    drop(phones);

    store.users.lock().unwrap().insert(user_id, user.clone());
    store.pins.lock().unwrap().insert(user_id, hashed);
    store.wallets.lock().unwrap().insert(user_id, wallet.clone());

    Ok(AuthResponse { token: make_token(user_id), user, wallet })
}

pub fn login(store: &Store, req: LoginRequest) -> Result<AuthResponse, ApiError> {
    let phones = store.phone_index.lock().unwrap();
    let user_id = phones.get(&req.phone).copied()
        .ok_or(ApiError { error: "Invalid phone or PIN".into() })?;
    drop(phones);

    let pins = store.pins.lock().unwrap();
    let hashed = pins.get(&user_id).cloned().unwrap();
    drop(pins);

    if !bcrypt::verify(&req.pin, &hashed).unwrap_or(false) {
        return Err(ApiError { error: "Invalid phone or PIN".into() });
    }

    let user = store.users.lock().unwrap().get(&user_id).cloned().unwrap();
    let wallet = store.wallets.lock().unwrap().get(&user_id).cloned().unwrap();

    Ok(AuthResponse { token: make_token(user_id), user, wallet })
}
