use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use chrono::{DateTime, Utc};
use shared::*;
use uuid::Uuid;
use sqlx::PgPool;

#[derive(Clone, Default)]
pub struct Store {
    pub users:              Arc<Mutex<HashMap<Uuid, User>>>,
    pub pins:               Arc<Mutex<HashMap<Uuid, String>>>,
    pub wallets:            Arc<Mutex<HashMap<Uuid, Wallet>>>,
    pub wallet_locks:       Arc<dashmap::DashMap<Uuid, Arc<Mutex<()>>>>,
    pub ledger:             Arc<Mutex<Vec<LedgerEntry>>>,
    pub transactions:       Arc<Mutex<Vec<Transaction>>>,
    pub outbox:             Arc<Mutex<Vec<OutboxEvent>>>,
    pub ajo_groups:         Arc<Mutex<HashMap<Uuid, AjoGroup>>>,
    pub ajo_members:        Arc<Mutex<HashMap<(Uuid, Uuid), AjoMember>>>,
    pub ajo_contributions:  Arc<Mutex<HashSet<(Uuid, Uuid, u32)>>>,
    pub bills:              Arc<Mutex<HashMap<Uuid, Bill>>>,
    pub bill_participants:  Arc<Mutex<HashMap<(Uuid, Uuid), BillParticipant>>>,
    pub bill_participant_index: Arc<Mutex<HashMap<Uuid, Vec<Uuid>>>>,
    pub phone_index:        Arc<Mutex<HashMap<String, Uuid>>>,
    pub refresh_tokens:     Arc<Mutex<HashMap<String, RefreshEntry>>>,
    pub login_attempts:     Arc<Mutex<HashMap<String, LoginAttempts>>>,
    pub denied_jtis:        Arc<Mutex<HashSet<String>>>,
    pub idempotency_cache:  Arc<Mutex<HashMap<String, IdempotencyEntry>>>,
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

    /// Hydrate in-memory cache from Postgres.
    /// Called once on startup — makes the store a warm read-through cache.
    pub async fn load_from_db(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let store = Self::new();

        // ── Users + pins ──────────────────────────────────────────────────────
        let rows: Vec<(Uuid, String, String, Option<String>, String, DateTime<Utc>)> =
            sqlx::query_as(
                "SELECT u.id, u.name, u.phone, u.email, u.role, u.created_at
                 FROM users u ORDER BY u.created_at"
            )
            .fetch_all(pool).await?;

        let pin_rows: Vec<(Uuid, String)> =
            sqlx::query_as("SELECT user_id, hash FROM pins")
            .fetch_all(pool).await?;

        {
            let mut users   = store.users.lock().unwrap();
            let mut phones  = store.phone_index.lock().unwrap();
            let mut pins    = store.pins.lock().unwrap();

            for (id, name, phone, email, role, created_at) in rows {
                let user = User {
                    id, name, phone: phone.clone(), email,
                    role: if role == "admin" { UserRole::Admin } else { UserRole::User },
                    created_at,
                };
                phones.insert(phone, id);
                users.insert(id, user);
            }
            for (user_id, hash) in pin_rows {
                pins.insert(user_id, hash);
            }
        }

        // ── Wallets ───────────────────────────────────────────────────────────
        let wallet_rows: Vec<(Uuid, Uuid, i64, i64, i64)> =
            sqlx::query_as(
                "SELECT id, user_id, available_kobo, ledger_kobo, version FROM wallets"
            )
            .fetch_all(pool).await?;

        {
            let mut wallets = store.wallets.lock().unwrap();
            for (id, user_id, available_kobo, ledger_kobo, version) in wallet_rows {
                wallets.insert(user_id, Wallet {
                    id, user_id, available_kobo, ledger_kobo,
                    version: version as u64,
                });
            }
        }

        // ── Transactions (last 1000 per wallet — enough for hot cache) ────────
        let txn_rows: Vec<(Uuid, Uuid, String, i64, String, String, String, DateTime<Utc>)> =
            sqlx::query_as(
                "SELECT id, wallet_id, kind, amount_kobo, reference, description, status, created_at
                 FROM transactions ORDER BY created_at DESC LIMIT 1000"
            )
            .fetch_all(pool).await?;

        {
            let mut txns = store.transactions.lock().unwrap();
            for (id, wallet_id, kind, amount_kobo, reference, description, status, created_at) in txn_rows {
                txns.push(Transaction {
                    id, wallet_id,
                    kind: if kind == "credit" { TransactionKind::Credit } else { TransactionKind::Debit },
                    amount_kobo, reference, description,
                    status: match status.as_str() {
                        "success" => TransactionStatus::Success,
                        "failed"  => TransactionStatus::Failed,
                        _         => TransactionStatus::Pending,
                    },
                    created_at,
                });
            }
        }

        // ── Ajo groups ────────────────────────────────────────────────────────
        let group_rows: Vec<(Uuid, String, Uuid, i64, String, i32, i32, String, DateTime<Utc>)> =
            sqlx::query_as(
                "SELECT id, name, admin_id, contribution_kobo, frequency,
                        member_count, current_cycle, status, created_at
                 FROM ajo_groups"
            )
            .fetch_all(pool).await?;

        {
            let mut groups = store.ajo_groups.lock().unwrap();
            for (id, name, admin_id, contribution_kobo, frequency, member_count, current_cycle, status, created_at) in group_rows {
                groups.insert(id, AjoGroup {
                    id, name, admin_id, contribution_kobo,
                    frequency: match frequency.as_str() {
                        "daily"  => AjoFrequency::Daily,
                        "weekly" => AjoFrequency::Weekly,
                        _        => AjoFrequency::Monthly,
                    },
                    member_count: member_count as u32,
                    current_cycle: current_cycle as u32,
                    status: match status.as_str() {
                        "completed" => AjoStatus::Completed,
                        "paused"    => AjoStatus::Paused,
                        _           => AjoStatus::Active,
                    },
                    created_at,
                });
            }
        }

        // ── Ajo members ───────────────────────────────────────────────────────
        let member_rows: Vec<(Uuid, Uuid, i32, bool)> =
            sqlx::query_as(
                "SELECT group_id, user_id, payout_position, has_received FROM ajo_members"
            )
            .fetch_all(pool).await?;

        {
            let mut members = store.ajo_members.lock().unwrap();
            for (group_id, user_id, payout_position, has_received) in member_rows {
                members.insert((group_id, user_id), AjoMember {
                    id: Uuid::new_v4(), // synthetic — PK is (group_id, user_id) in DB
                    group_id, user_id,
                    payout_position: payout_position as u32,
                    has_received,
                });
            }
        }

        // ── Ajo contributions ─────────────────────────────────────────────────
        let contrib_rows: Vec<(Uuid, Uuid, i32)> =
            sqlx::query_as("SELECT group_id, user_id, cycle FROM ajo_contributions")
            .fetch_all(pool).await?;

        {
            let mut contribs = store.ajo_contributions.lock().unwrap();
            for (group_id, user_id, cycle) in contrib_rows {
                contribs.insert((group_id, user_id, cycle as u32));
            }
        }

        // ── Bills ─────────────────────────────────────────────────────────────
        let bill_rows: Vec<(Uuid, String, Uuid, i64, String, DateTime<Utc>)> =
            sqlx::query_as(
                "SELECT id, title, creator_id, total_kobo, status, created_at FROM bills"
            )
            .fetch_all(pool).await?;

        {
            let mut bills = store.bills.lock().unwrap();
            for (id, title, creator_id, total_kobo, status, created_at) in bill_rows {
                bills.insert(id, Bill {
                    id, title, creator_id, total_kobo,
                    status: match status.as_str() {
                        "settled"        => BillStatus::Settled,
                        "partially_paid" => BillStatus::PartiallyPaid,
                        _                => BillStatus::Pending,
                    },
                    created_at,
                });
            }
        }

        // ── Bill participants ─────────────────────────────────────────────────
        let part_rows: Vec<(Uuid, Uuid, i64, bool)> =
            sqlx::query_as(
                "SELECT bill_id, user_id, share_kobo, paid FROM bill_participants ORDER BY bill_id"
            )
            .fetch_all(pool).await?;

        {
            let mut parts = store.bill_participants.lock().unwrap();
            let mut index = store.bill_participant_index.lock().unwrap();
            for (bill_id, user_id, share_kobo, paid) in part_rows {
                parts.insert((bill_id, user_id), BillParticipant {
                    id: Uuid::new_v4(),
                    bill_id, user_id, share_kobo, paid,
                });
                index.entry(bill_id).or_default().push(user_id);
            }
        }

        tracing::info!(
            users  = store.users.lock().unwrap().len(),
            wallets = store.wallets.lock().unwrap().len(),
            groups = store.ajo_groups.lock().unwrap().len(),
            bills  = store.bills.lock().unwrap().len(),
            "Store hydrated from Postgres"
        );

        Ok(store)
    }
}
