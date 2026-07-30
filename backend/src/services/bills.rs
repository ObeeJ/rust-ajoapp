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

    let mut participant_ids: Vec<Uuid> = {
        let phones = store.phone_index.lock().unwrap();
        req.participant_phones.iter()
            .filter_map(|p| phones.get(p.trim()).copied())
            .filter(|&id| id != creator_id)
            .collect()
    };
    participant_ids.dedup();

    let total_participants = participant_ids.len() as i64 + 1;
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
    let mut index = store.bill_participant_index.lock().unwrap();

    let mut ordered = vec![creator_id];
    participants.insert((bill.id, creator_id), BillParticipant {
        id: Uuid::new_v4(), bill_id: bill.id, user_id: creator_id, share_kobo, paid: false,
    });

    for user_id in participant_ids {
        participants.insert((bill.id, user_id), BillParticipant {
            id: Uuid::new_v4(), bill_id: bill.id, user_id, share_kobo, paid: false,
        });
        ordered.push(user_id);
    }
    index.insert(bill.id, ordered);

    Ok(bill)
}

pub fn pay_bill_share(store: &Store, bill_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    let share_kobo = {
        let mut participants = store.bill_participants.lock().unwrap();
        let p = participants.get_mut(&(bill_id, user_id))
            .ok_or(ApiError { error: "Not a participant".into() })?;
        if p.paid { return Err(ApiError { error: "Already paid".into() }); }
        p.paid = true;
        p.share_kobo
    };

    let reference = format!("bill-{}-{}", bill_id, user_id);
    debit_wallet(store, user_id, share_kobo, &reference, "Bill split payment")?;

    let creator_id = store.bills.lock().unwrap().get(&bill_id)
        .map(|b| b.creator_id)
        .ok_or(ApiError { error: "Bill not found".into() })?;

    if user_id != creator_id {
        credit_wallet(store, creator_id, share_kobo, &reference, "Bill split received");
    }

    // O(n) over participants for this bill — n ≤ 50
    let participant_ids = store.bill_participant_index.lock().unwrap()
        .get(&bill_id).cloned().unwrap_or_default();

    let all_paid = {
        let participants = store.bill_participants.lock().unwrap();
        participant_ids.iter().all(|uid| {
            participants.get(&(bill_id, *uid)).map(|p| p.paid).unwrap_or(false)
        })
    };

    if let Some(bill) = store.bills.lock().unwrap().get_mut(&bill_id) {
        bill.status = if all_paid { BillStatus::Settled } else { BillStatus::PartiallyPaid };
    }

    Ok(())
}

pub fn list_bills(store: &Store, user_id: Uuid, page: usize, per_page: usize) -> Vec<Bill> {
    let participants = store.bill_participants.lock().unwrap();
    let bill_ids: Vec<Uuid> = participants.keys()
        .filter(|(_, u)| *u == user_id)
        .map(|(b, _)| *b)
        .collect();
    drop(participants);

    let bills = store.bills.lock().unwrap();
    let mut result: Vec<Bill> = bill_ids.iter()
        .filter_map(|id| bills.get(id).cloned())
        .collect();
    result.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    result.into_iter().skip(page * per_page).take(per_page).collect()
}
