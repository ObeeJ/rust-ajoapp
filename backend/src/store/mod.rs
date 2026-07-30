use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use shared::*;
use uuid::Uuid;

/// In-memory store — swap for Postgres in production
#[derive(Clone, Default)]
pub struct Store {
    pub users: Arc<Mutex<HashMap<Uuid, User>>>,
    pub pins: Arc<Mutex<HashMap<Uuid, String>>>,        // user_id → hashed pin
    pub wallets: Arc<Mutex<HashMap<Uuid, Wallet>>>,
    pub transactions: Arc<Mutex<Vec<Transaction>>>,
    pub ajo_groups: Arc<Mutex<HashMap<Uuid, AjoGroup>>>,
    pub ajo_members: Arc<Mutex<Vec<AjoMember>>>,
    pub bills: Arc<Mutex<HashMap<Uuid, Bill>>>,
    pub bill_participants: Arc<Mutex<Vec<BillParticipant>>>,
    pub phone_index: Arc<Mutex<HashMap<String, Uuid>>>, // phone → user_id
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }
}
