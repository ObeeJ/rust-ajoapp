use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use shared::*;
use uuid::Uuid;

/// In-memory store — swap for Postgres in production
#[derive(Clone, Default)]
pub struct Store {
    pub users:             Arc<Mutex<HashMap<Uuid, User>>>,
    pub pins:              Arc<Mutex<HashMap<Uuid, String>>>,
    pub wallets:           Arc<Mutex<HashMap<Uuid, Wallet>>>,
    pub wallet_locks:      Arc<dashmap::DashMap<Uuid, Arc<Mutex<()>>>>,
    pub transactions:      Arc<Mutex<Vec<Transaction>>>,
    pub ajo_groups:        Arc<Mutex<HashMap<Uuid, AjoGroup>>>,
    pub ajo_members:       Arc<Mutex<Vec<AjoMember>>>,
    pub ajo_contributions: Arc<Mutex<Vec<AjoContribution>>>, // cycle dedup
    pub bills:             Arc<Mutex<HashMap<Uuid, Bill>>>,
    pub bill_participants: Arc<Mutex<Vec<BillParticipant>>>,
    pub phone_index:       Arc<Mutex<HashMap<String, Uuid>>>,
    pub refresh_tokens:    Arc<Mutex<HashMap<String, RefreshEntry>>>,
    pub login_attempts:    Arc<Mutex<HashMap<String, LoginAttempts>>>,
}

pub struct RefreshEntry {
    pub user_id:    Uuid,
    pub expires_at: DateTime<Utc>,
    pub used:       bool,
}

pub struct LoginAttempts {
    pub count:      u32,
    pub locked_until: Option<DateTime<Utc>>,
}

/// Tracks which user contributed in which cycle to prevent duplicates
#[derive(Clone)]
pub struct AjoContribution {
    pub group_id: Uuid,
    pub user_id:  Uuid,
    pub cycle:    u32,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a per-user wallet lock to prevent double-spend races
    pub fn wallet_lock(&self, user_id: Uuid) -> Arc<Mutex<()>> {
        self.wallet_locks
            .entry(user_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
