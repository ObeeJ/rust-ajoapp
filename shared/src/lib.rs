use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── User ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: Uuid,
    pub name: String,
    pub phone: String, // primary identifier in Nigeria
    pub email: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Wallet ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallet {
    pub id: Uuid,
    pub user_id: Uuid,
    pub balance_kobo: i64, // store in kobo (smallest NGN unit)
    pub ledger_balance_kobo: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: Uuid,
    pub wallet_id: Uuid,
    pub kind: TransactionKind,
    pub amount_kobo: i64,
    pub reference: String,
    pub description: String,
    pub status: TransactionStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionKind {
    Credit,
    Debit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TransactionStatus {
    Pending,
    Success,
    Failed,
}

// ── Ajo Group ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AjoGroup {
    pub id: Uuid,
    pub name: String,
    pub admin_id: Uuid,
    pub contribution_kobo: i64, // fixed amount each member pays per cycle
    pub frequency: AjoFrequency,
    pub member_count: u32,
    pub current_cycle: u32,
    pub status: AjoStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AjoFrequency {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AjoStatus {
    Active,
    Completed,
    Paused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AjoMember {
    pub id: Uuid,
    pub group_id: Uuid,
    pub user_id: Uuid,
    pub payout_position: u32, // which cycle they receive the pot
    pub has_received: bool,
}

// ── Bill Split ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bill {
    pub id: Uuid,
    pub title: String,
    pub creator_id: Uuid,
    pub total_kobo: i64,
    pub status: BillStatus,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum BillStatus {
    Pending,
    PartiallyPaid,
    Settled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BillParticipant {
    pub id: Uuid,
    pub bill_id: Uuid,
    pub user_id: Uuid,
    pub share_kobo: i64,
    pub paid: bool,
}

// ── API DTOs ──────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub phone: String,
    pub email: Option<String>,
    pub pin: String, // 4-digit PIN
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginRequest {
    pub phone: String,
    pub pin: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: User,
    pub wallet: Wallet,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FundWalletRequest {
    pub amount_kobo: i64,
    pub email: String, // for Paystack
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaystackInitResponse {
    pub authorization_url: String,
    pub reference: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAjoRequest {
    pub name: String,
    pub contribution_kobo: i64,
    pub frequency: AjoFrequency,
    pub member_count: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBillRequest {
    pub title: String,
    pub total_kobo: i64,
    pub participant_phones: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
}
