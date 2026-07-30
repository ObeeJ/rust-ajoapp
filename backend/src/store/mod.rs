use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use shared::*;
use uuid::Uuid;

#[derive(Clone, Default)]
pub struct Store {
    pub users:             Arc<Mutex<HashMap<Uuid, User>>>,
    pub pins:              Arc<Mutex<HashMap<Uuid, String>>>,
    pub wallets:           Arc<Mutex<HashMap<Uuid, Wallet>>>,
    pub wallet_locks:      Arc<dashmap::DashMap<Uuid, Arc<Mutex<()>>>>,
    pub transactions:      Arc<Mutex<Vec<Transaction>>>,

    // Ajo — O(1) keyed lookups
    pub ajo_groups:        Arc<Mutex<HashMap<Uuid, AjoGroup>>>,
    /// (group_id, user_id) → AjoMember
    pub ajo_members:       Arc<Mutex<HashMap<(Uuid, Uuid), AjoMember>>>,
    /// (group_id, user_id, cycle) — contribution dedup
    pub ajo_contributions: Arc<Mutex<HashSet<(Uuid, Uuid, u32)>>>,

    // Bills — O(1) keyed lookups
    pub bills:             Arc<Mutex<HashMap<Uuid, Bill>>>,
    /// (bill_id, user_id) → BillParticipant
    pub bill_participants: Arc<Mutex<HashMap<(Uuid, Uuid), BillParticipant>>>,
    /// bill_id → Vec<user_id> (ordered participant list)
    pub bill_participant_index: Arc<Mutex<HashMap<Uuid, Vec<Uuid>>>>,

    pub phone_index:       Arc<Mutex<HashMap<String, Uuid>>>,
    pub refresh_tokens:    Arc<Mutex<HashMap<String, RefreshEntry>>>,
    pub login_attempts:    Arc<Mutex<HashMap<String, LoginAttempts>>>,

    /// JTI deny-list for invalidated access tokens
    pub denied_jtis:       Arc<Mutex<HashSet<String>>>,

    /// Idempotency keys: key → cached response body
    pub idempotency_cache: Arc<Mutex<HashMap<String, IdempotencyEntry>>>,
}

pub struct RefreshEntry {
    pub user_id:    Uuid,
    pub expires_at: DateTime<Utc>,
    pub used:       bool,
}

pub struct LoginAttempts {
    pub count:        u32,
    pub locked_until: Option<DateTime<Utc>>,
}

pub struct IdempotencyEntry {
    pub status:     u16,
    pub body:       String,
    pub created_at: DateTime<Utc>,
}

impl Store {
    pub fn new() -> Self { Self::default() }

    pub fn wallet_lock(&self, user_id: Uuid) -> Arc<Mutex<()>> {
        self.wallet_locks
            .entry(user_id)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }
}
