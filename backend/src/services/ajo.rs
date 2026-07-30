use chrono::Utc;
use shared::*;
use uuid::Uuid;

use crate::store::{AjoContribution, Store};
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

    store.ajo_members.lock().unwrap().push(AjoMember {
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
    let group = groups
        .get(&group_id)
        .ok_or(ApiError { error: "Group not found".into() })?;

    if group.status != AjoStatus::Active {
        return Err(ApiError { error: "Group is not active".into() });
    }

    let mut members = store.ajo_members.lock().unwrap();
    let current_count = members.iter().filter(|m| m.group_id == group_id).count() as u32;

    if current_count >= group.member_count {
        return Err(ApiError { error: "Group is full".into() });
    }
    if members.iter().any(|m| m.group_id == group_id && m.user_id == user_id) {
        return Err(ApiError { error: "Already a member".into() });
    }

    let member = AjoMember {
        id: Uuid::new_v4(),
        group_id,
        user_id,
        payout_position: current_count,
        has_received: false,
    };
    members.push(member.clone());
    Ok(member)
}

/// Contribute for current cycle:
/// - Verifies contributor is a member
/// - Prevents duplicate contribution in same cycle
/// - Debits contributor, credits cycle receiver
/// - Advances cycle when all members have contributed
pub fn contribute(store: &Store, group_id: Uuid, contributor_id: Uuid) -> Result<(), ApiError> {
    let group = store
        .ajo_groups
        .lock()
        .unwrap()
        .get(&group_id)
        .cloned()
        .ok_or(ApiError { error: "Group not found".into() })?;

    if group.status != AjoStatus::Active {
        return Err(ApiError { error: "Group is not active".into() });
    }

    // Verify contributor is a member
    let is_member = store
        .ajo_members
        .lock()
        .unwrap()
        .iter()
        .any(|m| m.group_id == group_id && m.user_id == contributor_id);

    if !is_member {
        return Err(ApiError { error: "Not a member of this group".into() });
    }

    // Prevent duplicate contribution this cycle
    let already_contributed = store
        .ajo_contributions
        .lock()
        .unwrap()
        .iter()
        .any(|c| c.group_id == group_id && c.user_id == contributor_id && c.cycle == group.current_cycle);

    if already_contributed {
        return Err(ApiError { error: "Already contributed this cycle".into() });
    }

    // Find receiver for this cycle
    let receiver_id = store
        .ajo_members
        .lock()
        .unwrap()
        .iter()
        .find(|m| m.group_id == group_id && m.payout_position == group.current_cycle)
        .map(|m| m.user_id)
        .ok_or(ApiError { error: "No receiver for this cycle".into() })?;

    let reference = format!("ajo-{}-{}-{}", group_id, contributor_id, group.current_cycle);

    debit_wallet(
        store,
        contributor_id,
        group.contribution_kobo,
        &reference,
        &format!("Ajo contribution: {}", group.name),
    )?;

    credit_wallet(
        store,
        receiver_id,
        group.contribution_kobo,
        &reference,
        &format!("Ajo payout: {}", group.name),
    );

    // Record contribution
    store.ajo_contributions.lock().unwrap().push(AjoContribution {
        group_id,
        user_id: contributor_id,
        cycle: group.current_cycle,
    });

    // Count contributions this cycle — advance if all members contributed
    let contributions_this_cycle = store
        .ajo_contributions
        .lock()
        .unwrap()
        .iter()
        .filter(|c| c.group_id == group_id && c.cycle == group.current_cycle)
        .count() as u32;

    let member_count = store
        .ajo_members
        .lock()
        .unwrap()
        .iter()
        .filter(|m| m.group_id == group_id)
        .count() as u32;

    if contributions_this_cycle >= member_count {
        let mut groups = store.ajo_groups.lock().unwrap();
        if let Some(g) = groups.get_mut(&group_id) {
            let next_cycle = g.current_cycle + 1;
            if next_cycle >= g.member_count {
                g.status = AjoStatus::Completed;
            } else {
                g.current_cycle = next_cycle;
            }
        }

        // Mark receiver as having received
        let mut members = store.ajo_members.lock().unwrap();
        if let Some(m) = members
            .iter_mut()
            .find(|m| m.group_id == group_id && m.user_id == receiver_id)
        {
            m.has_received = true;
        }
    }

    Ok(())
}

pub fn list_groups(store: &Store, user_id: Uuid) -> Vec<AjoGroup> {
    let members = store.ajo_members.lock().unwrap();
    let group_ids: Vec<Uuid> = members
        .iter()
        .filter(|m| m.user_id == user_id)
        .map(|m| m.group_id)
        .collect();
    drop(members);

    let groups = store.ajo_groups.lock().unwrap();
    group_ids
        .iter()
        .filter_map(|id| groups.get(id).cloned())
        .collect()
}
