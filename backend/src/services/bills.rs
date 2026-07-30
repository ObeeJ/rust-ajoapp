use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::Store;
use crate::services::wallet::debit_wallet;

pub fn create_bill(store: &Store, creator_id: Uuid, req: CreateBillRequest) -> Result<Bill, ApiError> {
    let participant_count = req.participant_phones.len() as i64 + 1; // +1 for creator
    let share_kobo = req.total_kobo / participant_count;

    let bill = Bill {
        id: Uuid::new_v4(),
        title: req.title,
        creator_id,
        total_kobo: req.total_kobo,
        status: BillStatus::Pending,
        created_at: Utc::now(),
    };

    store.bills.lock().unwrap().insert(bill.id, bill.clone());

    // Add creator as participant (auto-paid)
    let mut participants = store.bill_participants.lock().unwrap();
    participants.push(BillParticipant {
        id: Uuid::new_v4(),
        bill_id: bill.id,
        user_id: creator_id,
        share_kobo,
        paid: true,
    });

    // Add other participants by phone lookup
    let phones = store.phone_index.lock().unwrap();
    for phone in &req.participant_phones {
        if let Some(&user_id) = phones.get(phone) {
            participants.push(BillParticipant {
                id: Uuid::new_v4(),
                bill_id: bill.id,
                user_id,
                share_kobo,
                paid: false,
            });
        }
    }

    Ok(bill)
}

pub fn pay_bill_share(store: &Store, bill_id: Uuid, user_id: Uuid) -> Result<(), ApiError> {
    let share_kobo = {
        let mut participants = store.bill_participants.lock().unwrap();
        let p = participants.iter_mut()
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
    debit_wallet(store, user_id, share_kobo, &reference, "Bill split payment")?;

    // Credit creator
    let creator_id = store.bills.lock().unwrap().get(&bill_id).map(|b| b.creator_id)
        .ok_or(ApiError { error: "Bill not found".into() })?;

    crate::services::wallet::credit_wallet(store, creator_id, share_kobo, &reference, "Bill split received");

    // Update bill status
    let all_paid = store.bill_participants.lock().unwrap()
        .iter()
        .filter(|p| p.bill_id == bill_id)
        .all(|p| p.paid);

    if all_paid {
        if let Some(bill) = store.bills.lock().unwrap().get_mut(&bill_id) {
            bill.status = BillStatus::Settled;
        }
    } else {
        if let Some(bill) = store.bills.lock().unwrap().get_mut(&bill_id) {
            bill.status = BillStatus::PartiallyPaid;
        }
    }

    Ok(())
}

pub fn list_bills(store: &Store, user_id: Uuid) -> Vec<Bill> {
    let participants = store.bill_participants.lock().unwrap();
    let bill_ids: Vec<Uuid> = participants.iter()
        .filter(|p| p.user_id == user_id)
        .map(|p| p.bill_id)
        .collect();
    drop(participants);

    let bills = store.bills.lock().unwrap();
    bill_ids.iter().filter_map(|id| bills.get(id).cloned()).collect()
}
