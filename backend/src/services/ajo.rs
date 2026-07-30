use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::Store;
use crate::services::wallet::{credit_wallet, debit_wallet};

pub fn create_group(store: &Store, admin_id: Uuid, req: CreateAjoRequest) -> AjoGroup {
    let group = AjoGroup {
        id: Uuid::new_v4(),
        name: req.name,
        admin_id,
        contribution_kobo: req.contribution_kobo,
        frequency: req.frequency,
        member_count: req.member_count,
        current_cycle: 0,
        status: AjoStatus::Active,
        created_at: Utc::now(),
    };
    store.ajo_groups.lock().unwrap().insert(group.id, group.clone());

    // Admin is first member, position 0
    store.ajo_members.lock().unwrap().push(AjoMember {
        id: Uuid::new_v4(),
        group_id: group.id,
        user_id: admin_id,
        payout_position: 0,
        has_received: false,
    });

    group
}

pub fn join_group(store: &Store, group_id: Uuid, user_id: Uuid) -> Result<AjoMember, ApiError> {
    let groups = store.ajo_groups.lock().unwrap();
    let group = groups.get(&group_id)
        .ok_or(ApiError { error: "Group not found".into() })?;

    let members = store.ajo_members.lock().unwrap();
    let current_count = members.iter().filter(|m| m.group_id == group_id).count() as u32;

    if current_count >= group.member_count {
        return Err(ApiError { error: "Group is full".into() });
    }
    if members.iter().any(|m| m.group_id == group_id && m.user_id == user_id) {
        return Err(ApiError { error: "Already a member".into() });
    }
    drop(members);
    drop(groups);

    let member = AjoMember {
        id: Uuid::new_v4(),
        group_id,
        user_id,
        payout_position: current_count,
        has_received: false,
    };
    store.ajo_members.lock().unwrap().push(member.clone());
    Ok(member)
}

/// Contribute for current cycle — debits wallet, credits pot holder
pub fn contribute(store: &Store, group_id: Uuid, contributor_id: Uuid) -> Result<(), ApiError> {
    let groups = store.ajo_groups.lock().unwrap();
    let group = groups.get(&group_id)
        .ok_or(ApiError { error: "Group not found".into() })?.clone();
    drop(groups);

    // Find who receives this cycle
    let members = store.ajo_members.lock().unwrap();
    let receiver = members.iter()
        .find(|m| m.group_id == group_id && m.payout_position == group.current_cycle)
        .cloned()
        .ok_or(ApiError { error: "No receiver for this cycle".into() })?;
    drop(members);

    let reference = format!("ajo-{}-{}-{}", group_id, contributor_id, group.current_cycle);

    debit_wallet(store, contributor_id, group.contribution_kobo, &reference,
        &format!("Ajo contribution: {}", group.name))?;

    credit_wallet(store, receiver.user_id, group.contribution_kobo, &reference,
        &format!("Ajo payout: {}", group.name));

    Ok(())
}

pub fn list_groups(store: &Store, user_id: Uuid) -> Vec<AjoGroup> {
    let members = store.ajo_members.lock().unwrap();
    let group_ids: Vec<Uuid> = members.iter()
        .filter(|m| m.user_id == user_id)
        .map(|m| m.group_id)
        .collect();
    drop(members);

    let groups = store.ajo_groups.lock().unwrap();
    group_ids.iter().filter_map(|id| groups.get(id).cloned()).collect()
}
