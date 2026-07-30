use crate::store::Store;
use crate::services::{auth, ajo, bills, wallet};
use shared::*;

fn test_store() -> Store { Store::new() }

fn register_user(store: &Store, phone: &str, name: &str) -> uuid::Uuid {
    std::env::set_var("JWT_SECRET", "test-secret-that-is-long-enough-32b");
    let req = RegisterRequest {
        name: name.into(), phone: phone.into(),
        email: None, pin: "1234".into(),
    };
    auth::register(store, req).unwrap().user.id
}

// ── Auth ──────────────────────────────────────────────────────────────────────

#[test]
fn register_and_login_ok() {
    let store = test_store();
    let uid = register_user(&store, "08011111111", "Alice");
    let res = auth::login(&store, LoginRequest { phone: "08011111111".into(), pin: "1234".into() });
    assert!(res.is_ok());
    assert_eq!(res.unwrap().user.id, uid);
}

#[test]
fn duplicate_phone_rejected() {
    let store = test_store();
    register_user(&store, "08022222222", "Bob");
    let res = auth::register(&store, RegisterRequest {
        name: "Bob2".into(), phone: "08022222222".into(), email: None, pin: "5678".into(),
    });
    assert!(res.is_err());
    assert!(res.unwrap_err().error.contains("already registered"));
}

#[test]
fn wrong_pin_rejected() {
    let store = test_store();
    register_user(&store, "08033333333", "Carol");
    let res = auth::login(&store, LoginRequest { phone: "08033333333".into(), pin: "9999".into() });
    assert!(res.is_err());
}

#[test]
fn rate_limit_locks_after_5_failures() {
    let store = test_store();
    register_user(&store, "08044444444", "Dave");
    for _ in 0..5 {
        let _ = auth::login(&store, LoginRequest { phone: "08044444444".into(), pin: "0000".into() });
    }
    let res = auth::login(&store, LoginRequest { phone: "08044444444".into(), pin: "1234".into() });
    assert!(res.is_err());
    assert!(res.unwrap_err().error.contains("Too many"));
}

#[test]
fn refresh_token_single_use() {
    let store = test_store();
    register_user(&store, "08055555555", "Eve");
    let tokens = auth::login(&store, LoginRequest { phone: "08055555555".into(), pin: "1234".into() }).unwrap();
    let (_, new_refresh) = auth::rotate_refresh_token(&store, &tokens.refresh_token).unwrap();
    // Old token must be rejected
    let res = auth::rotate_refresh_token(&store, &tokens.refresh_token);
    assert!(res.is_err());
    // New token must work
    assert!(auth::rotate_refresh_token(&store, &new_refresh).is_ok());
}

// ── Wallet ────────────────────────────────────────────────────────────────────

#[test]
fn debit_insufficient_balance_rejected() {
    let store = test_store();
    let uid = register_user(&store, "08066666666", "Frank");
    let res = wallet::debit_wallet(&store, uid, 100_00, "ref", "test");
    assert!(res.is_err());
    assert!(res.unwrap_err().error.contains("Insufficient"));
}

#[test]
fn credit_then_debit_updates_balance() {
    let store = test_store();
    let uid = register_user(&store, "08077777777", "Grace");
    wallet::credit_wallet(&store, uid, 500_00, "ref1", "top-up");
    wallet::debit_wallet(&store, uid, 200_00, "ref2", "spend").unwrap();
    let w = wallet::get_wallet(&store, uid).unwrap();
    assert_eq!(w.available_kobo, 300_00);
}

// ── Ajo ───────────────────────────────────────────────────────────────────────

#[test]
fn ajo_cycle_advances_after_all_contribute() {
    let store = test_store();
    let admin = register_user(&store, "08088888881", "Admin");
    let member = register_user(&store, "08088888882", "Member");

    // Fund both wallets
    wallet::credit_wallet(&store, admin,  1000_00, "r1", "fund");
    wallet::credit_wallet(&store, member, 1000_00, "r2", "fund");

    let group = ajo::create_group(&store, admin, CreateAjoRequest {
        name: "Test Ajo".into(), contribution_kobo: 100_00,
        frequency: AjoFrequency::Monthly, member_count: 2,
    }).unwrap();

    ajo::join_group(&store, group.id, member).unwrap();

    // Both contribute — cycle should advance
    ajo::contribute(&store, group.id, admin).unwrap();
    ajo::contribute(&store, group.id, member).unwrap();

    let updated = store.ajo_groups.lock().unwrap().get(&group.id).cloned().unwrap();
    assert_eq!(updated.current_cycle, 1);
}

#[test]
fn ajo_duplicate_contribution_rejected() {
    let store = test_store();
    let admin  = register_user(&store, "08099999991", "Admin2");
    let member = register_user(&store, "08099999992", "Member2");
    wallet::credit_wallet(&store, admin,  1000_00, "r1", "fund");
    wallet::credit_wallet(&store, member, 1000_00, "r2", "fund");

    let group = ajo::create_group(&store, admin, CreateAjoRequest {
        name: "Solo".into(), contribution_kobo: 100_00,
        frequency: AjoFrequency::Monthly, member_count: 2,
    }).unwrap();
    ajo::join_group(&store, group.id, member).unwrap();

    // First contribution succeeds
    ajo::contribute(&store, group.id, admin).unwrap();
    // Second contribution in same cycle rejected
    let res = ajo::contribute(&store, group.id, admin);
    assert!(res.is_err());
    assert!(res.unwrap_err().error.contains("Already contributed"));
}

#[test]
fn ajo_non_member_cannot_contribute() {
    let store = test_store();
    let admin   = register_user(&store, "08011100001", "Admin3");
    let outsider = register_user(&store, "08011100002", "Outsider");
    wallet::credit_wallet(&store, outsider, 1000_00, "r", "fund");

    let group = ajo::create_group(&store, admin, CreateAjoRequest {
        name: "Private".into(), contribution_kobo: 100_00,
        frequency: AjoFrequency::Monthly, member_count: 2,
    }).unwrap();

    let res = ajo::contribute(&store, group.id, outsider);
    assert!(res.is_err());
    assert!(res.unwrap_err().error.contains("Not a member"));
}

// ── Bills ─────────────────────────────────────────────────────────────────────

#[test]
fn bill_creator_must_pay_own_share() {
    let store = test_store();
    let creator = register_user(&store, "08022200001", "Creator");
    let payer   = register_user(&store, "08022200002", "Payer");
    wallet::credit_wallet(&store, creator, 500_00, "r1", "fund");
    wallet::credit_wallet(&store, payer,   500_00, "r2", "fund");

    let bill = bills::create_bill(&store, creator, CreateBillRequest {
        title: "Dinner".into(), total_kobo: 200_00,
        participant_phones: vec!["08022200002".into()],
    }).unwrap();

    // Creator's share is NOT auto-paid
    let p = store.bill_participants.lock().unwrap()
        .get(&(bill.id, creator)).cloned().unwrap();
    assert!(!p.paid);

    // Creator pays
    bills::pay_bill_share(&store, bill.id, creator).unwrap();
    let w = wallet::get_wallet(&store, creator).unwrap();
    assert_eq!(w.available_kobo, 400_00); // 500 - 100
}

#[test]
fn bill_settles_when_all_paid() {
    let store = test_store();
    let creator = register_user(&store, "08033300001", "C");
    let p2      = register_user(&store, "08033300002", "P2");
    wallet::credit_wallet(&store, creator, 500_00, "r1", "fund");
    wallet::credit_wallet(&store, p2,      500_00, "r2", "fund");

    let bill = bills::create_bill(&store, creator, CreateBillRequest {
        title: "Lunch".into(), total_kobo: 200_00,
        participant_phones: vec!["08033300002".into()],
    }).unwrap();

    bills::pay_bill_share(&store, bill.id, creator).unwrap();
    bills::pay_bill_share(&store, bill.id, p2).unwrap();

    let updated = store.bills.lock().unwrap().get(&bill.id).cloned().unwrap();
    assert_eq!(updated.status, BillStatus::Settled);
}

#[test]
fn double_pay_rejected() {
    let store = test_store();
    let creator = register_user(&store, "08044400001", "D");
    wallet::credit_wallet(&store, creator, 500_00, "r", "fund");

    let bill = bills::create_bill(&store, creator, CreateBillRequest {
        title: "Solo".into(), total_kobo: 100_00,
        participant_phones: vec![],
    }).unwrap();

    bills::pay_bill_share(&store, bill.id, creator).unwrap();
    let res = bills::pay_bill_share(&store, bill.id, creator);
    assert!(res.is_err());
    assert!(res.unwrap_err().error.contains("Already paid"));
}

// ── Ledger Invariant ──────────────────────────────────────────────────────────

#[test]
fn ledger_invariant_holds_after_credit_and_debit() {
    let store = test_store();
    let uid = register_user(&store, "08055500001", "Ledger");
    wallet::credit_wallet(&store, uid, 1000_00, "ref-c1", "top-up");
    wallet::credit_wallet(&store, uid, 500_00,  "ref-c2", "top-up");
    wallet::debit_wallet(&store, uid, 300_00,   "ref-d1", "spend").unwrap();

    let wallet_id = store.wallets.lock().unwrap().get(&uid).unwrap().id;
    wallet::assert_ledger_invariant(&store, wallet_id).expect("ledger invariant violated");
}

#[test]
fn ledger_entries_are_append_only() {
    let store = test_store();
    let uid = register_user(&store, "08055500002", "Append");
    wallet::credit_wallet(&store, uid, 200_00, "r1", "fund");
    wallet::debit_wallet(&store, uid, 100_00, "r2", "spend").unwrap();

    let ledger = store.ledger.lock().unwrap();
    // Exactly 2 entries — no updates, no deletes
    let wallet_id = store.wallets.lock().unwrap().get(&uid).unwrap().id;
    let entries: Vec<_> = ledger.iter().filter(|e| e.wallet_id == wallet_id).collect();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].kind, shared::EntryKind::Credit);
    assert_eq!(entries[1].kind, shared::EntryKind::Debit);
}

#[test]
fn outbox_events_staged_not_delivered_inline() {
    let store = test_store();
    let uid = register_user(&store, "08055500003", "Outbox");
    wallet::credit_wallet(&store, uid, 500_00, "r1", "fund");
    wallet::debit_wallet(&store, uid, 200_00, "r2", "spend").unwrap();

    let outbox = store.outbox.lock().unwrap();
    // Both operations staged outbox events
    assert!(outbox.len() >= 2);
    // All must be Pending — none delivered inline
    assert!(outbox.iter().all(|e| e.status == shared::OutboxStatus::Pending));
}

// ── Concurrency Stress Test ───────────────────────────────────────────────────

#[test]
fn concurrent_debits_never_overdraft() {
    use std::sync::Arc;
    use std::thread;

    let store = Arc::new(test_store());
    let uid   = register_user(&store, "08066600001", "Concurrent");

    // Fund with exactly 1000 kobo
    wallet::credit_wallet(&store, uid, 1000, "fund", "seed");

    // 10 threads each try to debit 200 kobo — only 5 should succeed
    let handles: Vec<_> = (0..10).map(|i| {
        let s = store.clone();
        thread::spawn(move || {
            wallet::debit_wallet(&s, uid, 200, &format!("debit-{i}"), "concurrent")
        })
    }).collect();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    let successes = results.iter().filter(|r| r.is_ok()).count();
    let failures  = results.iter().filter(|r| r.is_err()).count();

    // Exactly 5 succeed (5 × 200 = 1000), 5 fail with insufficient balance
    assert_eq!(successes, 5, "expected exactly 5 successful debits");
    assert_eq!(failures,  5, "expected exactly 5 insufficient balance errors");

    // Balance must be exactly 0 — no overdraft
    let w = wallet::get_wallet(&store, uid).unwrap();
    assert_eq!(w.available_kobo, 0, "balance must be 0 after exact spend");

    // Ledger invariant must still hold
    let wallet_id = store.wallets.lock().unwrap().get(&uid).unwrap().id;
    wallet::assert_ledger_invariant(&store, wallet_id).expect("ledger invariant violated after concurrency");
}
