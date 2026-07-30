use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::Store;
use crate::services::wallet::{credit_wallet, debit_wallet};

pub fn create_bill(store: &Store, creator_id: Uuid, req: CreateBillRequest) -> Result<Bill, ApiError> {
    if req.title.trim().is_empty() || req.title.len() > 200 {
        return Err(ApiError { error: "Title must be 1–200 characters".into() });
    }
    if req.total_kobo < 100 {
        return Err(ApiError { error: "Minimum bill amount is ₦1".into() });
    }
    if req.participant_phones.len() > 49 {
        return Err(ApiError { error: "Maximum 49 additional participants".into() });
    }

    // Resolve participant phones to user IDs (skip unknowns)
    let mut participant_ids: Vec<Uuid> = {
        let phones = store.phone_index.lock().unwrap();
        req.participant_phones
            .iter()
            .filter_map(|p| phones.get(p.trim()).copied())
            .filter(|&id| id != creator_id) // deduplicate creator
            .collect()
    };
    participant_ids.dedup();

    let total_participants = participant_ids.len() as i64 + 1; // +1 for creator
    let share_kobo = req.total_kobo / total_participants;

    let bill = Bill {
        id: Uuid::new_v4(),
        title: req.title.trim().to_string(),
        creator_id,
        total_kobo: req.total_kobo,
        status: BillStatus::Pending,
        created_at: Utc::now(),
    };

    store.bills.lock().unwrap().insert(bill.id, bill.clone());

    let mut participants = store.bill_participants.lock().unwrap();

    // Creator's share — NOT auto-paid, they must pay like everyone else
    participants.push(BillParticipant {
        id: Uuid::new_v4(),
        bill_id: bill.id,
        user_id: creator_id,
        share_kobo,
        paid: false,
    });

    for user_id in participant_ids {
        participants.push(BillParticipant {
            id: Uuid::new_v4(),
            bill_id: bill.id,
            user_id,
            share_kobo,
            paid: false,
        });
    }

    Ok(bill)
}

pub fn pay_bill_share(store: &Store, bill_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    let share_kobo = {
        let mut participants = store.bill_participants.lock().unwrap();
        let p = participants
            .iter_mut()
            .find(|p| p.bill_id == bill_id && p.user_id == user_id)
            .ok_or(ApiError { error: "Not a participant".into() })?;

        if p.paid {
            return Err(ApiError { error: "Already paid".into() });
        }
        let share = p.share_kobo;
        p.paid = true;
        share
    };

    let reference = format!("bill-{}-{}", bill_id, user_id);

    // Debit payer
    debit_wallet(store, user_id, share_kobo, &reference, "Bill split payment")?;

    // Credit creator
    let creator_id = store
        .bills
        .lock()
        .unwrap()
        .get(&bill_id)
        .map(|b| b.creator_id)
        .ok_or(ApiError { error: "Bill not found".into() })?;

    // Don't credit creator for their own share
    if user_id != creator_id {
        credit_wallet(store, creator_id, share_kobo, &reference, "Bill split received");
    }

    // Update bill status
    let all_paid = store
        .bill_participants
        .lock()
        .unwrap()
        .iter()
        .filter(|p| p.bill_id == bill_id)
        .all(|p| p.paid);

    if let Some(bill) = store.bills.lock().unwrap().get_mut(&bill_id) {
        bill.status = if all_paid {
            BillStatus::Settled
        } else {
            BillStatus::PartiallyPaid
        };
    }

    Ok(())
}

pub fn list_bills(store: &Store, user_id: Uuid, page: usize, per_page: usize) -> Vec<Bill> {
    let participants = store.bill_participants.lock().unwrap();
    let bill_ids: Vec<Uuid> = participants
        .iter()
        .filter(|p| p.user_id == user_id)
        .map(|p| p.bill_id)
        .collect();
    drop(participants);

    let bills = store.bills.lock().unwrap();
    bill_ids
        .iter()
        .filter_map(|id| bills.get(id).cloned())
        .rev()
        .skip(page * per_page)
        .take(per_page)
        .collect()
}
