use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::Store;
use crate::services::wallet::{credit_wallet, debit_wallet};

pub fn create_group(store: &Store, admin_id: Uuid, req: CreateAjoRequest) -> Result<AjoGroup, ApiError> {
    if req.name.trim().is_empty() || req.name.len() > 100 {
        return Err(ApiError { error: "Group name must be 1–100 characters".into() });
    }
    if req.contribution_kobo < 10000 {
        return Err(ApiError { error: "Minimum contribution is ₦100".into() });
    }
    if req.member_count < 2 || req.member_count > 50 {
        return Err(ApiError { error: "Member count must be 2–50".into() });
    }

    let group = AjoGroup {
        id: Uuid::new_v4(),
        name: req.name.trim().to_string(),
        admin_id,
        contribution_kobo: req.contribution_kobo,
        frequency: req.frequency,
        member_count: req.member_count,
        current_cycle: 0,
        status: AjoStatus::Active,
        created_at: Utc::now(),
    };
    store.ajo_groups.lock().unwrap().insert(group.id, group.clone());

    store.ajo_members.lock().unwrap().insert((group.id, admin_id), AjoMember {
        id: Uuid::new_v4(),
        group_id: group.id,
        user_id: admin_id,
        payout_position: 0,
        has_received: false,
    });

    Ok(group)
}

pub fn join_group(store: &Store, group_id: Uuid, user_id: Uuid) -> Result<AjoMember, ApiError> {
    let groups = store.ajo_groups.lock().unwrap();
    let group = groups.get(&group_id)
        .ok_or(ApiError { error: "Group not found".into() })?;

    if group.status != AjoStatus::Active {
        return Err(ApiError { error: "Group is not active".into() });
    }

    let mut members = store.ajo_members.lock().unwrap();

    if members.contains_key(&(group_id, user_id)) {
        return Err(ApiError { error: "Already a member".into() });
    }

    let current_count = members.keys().filter(|(g, _)| *g == group_id).count() as u32;
    if current_count >= group.member_count {
        return Err(ApiError { error: "Group is full".into() });
    }

    let member = AjoMember {
        id: Uuid::new_v4(),
        group_id,
        user_id,
        payout_position: current_count,
        has_received: false,
    };
    members.insert((group_id, user_id), member.clone());
    Ok(member)
}

pub fn contribute(store: &Store, group_id: Uuid, contributor_id: Uuid) -> Result<(), ApiError> {
    let group = store.ajo_groups.lock().unwrap()
        .get(&group_id).cloned()
        .ok_or(ApiError { error: "Group not found".into() })?;

    if group.status != AjoStatus::Active {
        return Err(ApiError { error: "Group is not active".into() });
    }

    // O(1) membership check
    if !store.ajo_members.lock().unwrap().contains_key(&(group_id, contributor_id)) {
        return Err(ApiError { error: "Not a member of this group".into() });
    }

    // O(1) duplicate contribution check
    if store.ajo_contributions.lock().unwrap()
        .contains(&(group_id, contributor_id, group.current_cycle))
    {
        return Err(ApiError { error: "Already contributed this cycle".into() });
    }

    // Find receiver for this cycle — O(n) over members but n ≤ 50
    let receiver_id = store.ajo_members.lock().unwrap()
        .iter()
        .find(|((g, _), m)| *g == group_id && m.payout_position == group.current_cycle)
        .map(|((_, u), _)| *u)
        .ok_or(ApiError { error: "No receiver for this cycle".into() })?;

    let reference = format!("ajo-{}-{}-{}", group_id, contributor_id, group.current_cycle);

    debit_wallet(store, contributor_id, group.contribution_kobo, &reference,
        &format!("Ajo contribution: {}", group.name))?;

    // Platform fee: 0.5% of contribution, deducted from payout (integer math only)
    let fee_kobo    = group.contribution_kobo / 200; // 0.5% — integer division, no floats
    let payout_kobo = group.contribution_kobo - fee_kobo;

    credit_wallet(store, receiver_id, payout_kobo, &reference,
        &format!("Ajo payout: {}", group.name));

    store.ajo_contributions.lock().unwrap()
        .insert((group_id, contributor_id, group.current_cycle));

    // Notify receiver that a contribution arrived
    let receiver_email = store.users.lock().unwrap()
        .get(&receiver_id).and_then(|u| u.email.clone()).unwrap_or_default();
    let receiver_name = store.users.lock().unwrap()
        .get(&receiver_id).map(|u| u.name.clone()).unwrap_or_default();
    let contributor_name = store.users.lock().unwrap()
        .get(&contributor_id).map(|u| u.name.clone()).unwrap_or_else(|| "A member".into());

    crate::services::wallet::stage_outbox_event(store, "ajo.contribution", serde_json::json!({
        "receiver_email":    receiver_email,
        "receiver_name":     receiver_name,
        "contributor_name":  contributor_name,
        "amount_kobo":       payout_kobo,
        "group_name":        group.name,
        "cycle":             group.current_cycle,
    }));

    // Count contributions this cycle — advance if all members contributed
    let member_count = store.ajo_members.lock().unwrap()
        .keys().filter(|(g, _)| *g == group_id).count() as u32;

    let contributions_this_cycle = store.ajo_contributions.lock().unwrap()
        .iter().filter(|(g, _, c)| *g == group_id && *c == group.current_cycle).count() as u32;

    if contributions_this_cycle >= member_count {
        let mut groups = store.ajo_groups.lock().unwrap();
        if let Some(g) = groups.get_mut(&group_id) {
            let next = g.current_cycle + 1;
            g.status = if next >= g.member_count { AjoStatus::Completed } else { AjoStatus::Active };
            if g.status == AjoStatus::Active { g.current_cycle = next; }
        }
        if let Some(m) = store.ajo_members.lock().unwrap().get_mut(&(group_id, receiver_id)) {
            m.has_received = true;
        }
    }

    Ok(())
}

pub fn list_groups(store: &Store, user_id: Uuid) -> Vec<AjoGroup> {
    let members = store.ajo_members.lock().unwrap();
    let group_ids: Vec<Uuid> = members.keys()
        .filter(|(_, u)| *u == user_id)
        .map(|(g, _)| *g)
        .collect();
    drop(members);

    let groups = store.ajo_groups.lock().unwrap();
    group_ids.iter().filter_map(|id| groups.get(id).cloned()).collect()
}

pub fn get_group(store: &Store, group_id: Uuid, user_id: Uuid) -> Result<serde_json::Value, ApiError> {
    let groups = store.ajo_groups.lock().unwrap();
    let group = groups.get(&group_id)
        .ok_or(ApiError { error: "Group not found".into() })?.clone();
    drop(groups);

    // Only members can view
    if !store.ajo_members.lock().unwrap().contains_key(&(group_id, user_id)) {
        return Err(ApiError { error: "Not a member".into() });
    }

    let members: Vec<_> = store.ajo_members.lock().unwrap()
        .iter()
        .filter(|((g, _), _)| *g == group_id)
        .map(|((_, u), m)| serde_json::json!({
            "user_id": u,
            "payout_position": m.payout_position,
            "has_received": m.has_received,
        }))
        .collect();

    let contributions_this_cycle = store.ajo_contributions.lock().unwrap()
        .iter()
        .filter(|(g, _, c)| *g == group_id && *c == group.current_cycle)
        .count();

    Ok(serde_json::json!({
        "group": group,
        "members": members,
        "contributions_this_cycle": contributions_this_cycle,
        "members_total": members.len(),
    }))
}
