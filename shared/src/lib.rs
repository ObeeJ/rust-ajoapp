use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── User ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum UserRole { User, Admin }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id:         Uuid,
    pub name:       String,
    pub phone:      String,
    pub email:      Option<String>,
    pub role:       UserRole,
    pub created_at: DateTime<Utc>,
}

// ── Wallet ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub id:                  Uuid,
    pub user_id:             Uuid,
    /// Settled balance — what the user can spend right now
    pub available_kobo:      i64,
    /// Ledger balance — includes pending holds not yet settled
    pub ledger_kobo:         i64,
    /// Optimistic lock version — incremented on every mutation
    pub version:             u64,
}

impl Wallet {
    /// The only correct way to read spendable balance
    pub fn spendable(&self) -> i64 { self.available_kobo }
}

// ── Double-Entry Ledger ───────────────────────────────────────────────────────
/// Immutable append-only. Never UPDATE or DELETE a row.
/// Every financial event produces exactly two entries: one Debit, one Credit.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id:                  Uuid,
    pub wallet_id:           Uuid,
    pub counterpart_wallet_id: Option<Uuid>, // None = external (Paystack / system)
    pub kind:                EntryKind,
    pub amount_kobo:         i64,            // always positive
    pub running_balance_kobo: i64,           // snapshot after this entry
    pub reference:           String,         // idempotency anchor
    pub description:         String,
    pub status:              EntryStatus,
    pub created_at:          DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind { Debit, Credit }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntryStatus { Pending, Settled, Failed }

// Keep Transaction as a public-facing summary (maps to LedgerEntry pairs)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id:           Uuid,
    pub wallet_id:    Uuid,
    pub kind:         TransactionKind,
    pub amount_kobo:  i64,
    pub reference:    String,
    pub description:  String,
    pub status:       TransactionStatus,
    pub created_at:   DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionKind { Credit, Debit }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus { Pending, Success, Failed }

// ── Outbox ────────────────────────────────────────────────────────────────────
/// Persisted atomically with the wallet mutation.
/// Background worker picks up and delivers — never in-flight inside a transaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEvent {
    pub id:          Uuid,
    pub event_type:  String,   // e.g. "wallet.credited", "ajo.payout"
    pub payload:     String,   // JSON
    pub status:      OutboxStatus,
    pub attempts:    u32,
    pub created_at:  DateTime<Utc>,
    pub next_retry:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum OutboxStatus { Pending, Delivered, Failed }

// ── Ajo ───────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AjoGroup {
    pub id:                Uuid,
    pub name:              String,
    pub admin_id:          Uuid,
    pub contribution_kobo: i64,
    pub frequency:         AjoFrequency,
    pub member_count:      u32,
    pub current_cycle:     u32,
    pub status:            AjoStatus,
    pub created_at:        DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AjoFrequency { Daily, Weekly, Monthly }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AjoStatus { Active, Completed, Paused }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AjoMember {
    pub id:              Uuid,
    pub group_id:        Uuid,
    pub user_id:         Uuid,
    pub payout_position: u32,
    pub has_received:    bool,
}

// ── Bills ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bill {
    pub id:          Uuid,
    pub title:       String,
    pub creator_id:  Uuid,
    pub total_kobo:  i64,
    pub status:      BillStatus,
    pub created_at:  DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BillStatus { Pending, PartiallyPaid, Settled }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillParticipant {
    pub id:          Uuid,
    pub bill_id:     Uuid,
    pub user_id:     Uuid,
    pub share_kobo:  i64,
    pub paid:        bool,
}

// ── API DTOs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub name:  String,
    pub phone: String,
    pub email: Option<String>,
    pub pin:   String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub phone: String,
    pub pin:   String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token:  String,
    pub user:   User,
    pub wallet: Wallet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundWalletRequest {
    pub amount_kobo: i64,
    pub email:       String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaystackInitResponse {
    pub authorization_url: String,
    pub reference:         String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAjoRequest {
    pub name:              String,
    pub contribution_kobo: i64,
    pub frequency:         AjoFrequency,
    pub member_count:      u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBillRequest {
    pub title:              String,
    pub total_kobo:         i64,
    pub participant_phones: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}
