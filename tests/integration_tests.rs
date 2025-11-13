use payments_engine::transaction::InputTransaction;
use payments_engine::PaymentsEngine;
use fastnum::D128;

type Decimal = D128;

// Helper function to create decimals from strings
fn decimal(s: &str) -> Decimal {
    s.parse().unwrap()
}

#[test]
fn test_basic_deposit() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });

    let accounts = engine.get_accounts();
    assert_eq!(accounts.len(), 1);
    assert_eq!(accounts[0].client, 1);
    assert_eq!(accounts[0].available, decimal("100.0"));
    assert_eq!(accounts[0].total, decimal("100.0"));
    assert_eq!(accounts[0].held, Decimal::ZERO);
}

#[test]
fn test_basic_withdrawal() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: decimal("30.0"),
    });

    let accounts = engine.get_accounts();
    assert_eq!(accounts[0].available, decimal("70.0"));
    assert_eq!(accounts[0].total, decimal("70.0"));
}

#[test]
fn test_insufficient_funds_withdrawal() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("50.0"),
    });
    engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: decimal("100.0"),
    });

    let accounts = engine.get_accounts();
    // Withdrawal should fail, balance should remain 50.0
    assert_eq!(accounts[0].available, decimal("50.0"));
    assert_eq!(accounts[0].total, decimal("50.0"));
}

#[test]
fn test_dispute() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });

    let accounts = engine.get_accounts();
    assert_eq!(accounts[0].available, Decimal::ZERO);
    assert_eq!(accounts[0].held, decimal("100.0"));
    assert_eq!(accounts[0].total, decimal("100.0"));
    assert!(!accounts[0].locked);
}

#[test]
fn test_dispute_with_negative_available() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: decimal("60.0"),
    });
    engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });

    let accounts = engine.get_accounts();
    // available: 40 - 100 = -60
    assert_eq!(accounts[0].available, decimal("-60.0"));
    assert_eq!(accounts[0].held, decimal("100.0"));
    assert_eq!(accounts[0].total, decimal("40.0"));
}

#[test]
fn test_resolve() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
    engine.process_transaction(InputTransaction::Resolve { client: 1, tx: 1 });

    let accounts = engine.get_accounts();
    assert_eq!(accounts[0].available, decimal("100.0"));
    assert_eq!(accounts[0].held, Decimal::ZERO);
    assert_eq!(accounts[0].total, decimal("100.0"));
    assert!(!accounts[0].locked);
}

#[test]
fn test_chargeback() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
    engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });

    let accounts = engine.get_accounts();
    assert_eq!(accounts[0].available, Decimal::ZERO);
    assert_eq!(accounts[0].held, Decimal::ZERO);
    assert_eq!(accounts[0].total, Decimal::ZERO);
    assert!(accounts[0].locked);
}

#[test]
fn test_locked_account_prevents_deposits() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
    engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });
    // Try to deposit after account is locked
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 2,
        amount: decimal("50.0"),
    });

    let accounts = engine.get_accounts();
    // Balance should still be 0, deposit should be rejected
    assert_eq!(accounts[0].total, Decimal::ZERO);
    assert!(accounts[0].locked);
}

#[test]
fn test_multiple_clients() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Deposit {
        client: 2,
        tx: 2,
        amount: decimal("200.0"),
    });

    let accounts = engine.get_accounts();
    assert_eq!(accounts.len(), 2);
    assert_eq!(accounts[0].client, 1);
    assert_eq!(accounts[0].total, decimal("100.0"));
    assert_eq!(accounts[1].client, 2);
    assert_eq!(accounts[1].total, decimal("200.0"));
}

#[test]
fn test_dispute_only_affects_deposits() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: decimal("30.0"),
    });
    // Try to dispute the withdrawal
    engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 2 });

    let accounts = engine.get_accounts();
    // Dispute should be ignored, balance unchanged
    assert_eq!(accounts[0].available, decimal("70.0"));
    assert_eq!(accounts[0].held, Decimal::ZERO);
}

#[test]
fn test_precision() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("1.2345"),
    });

    let accounts = engine.get_accounts();
    assert_eq!(accounts[0].available, decimal("1.2345"));
}

#[test]
fn test_cannot_resolve_without_dispute() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Resolve { client: 1, tx: 1 });

    let accounts = engine.get_accounts();
    // Resolve should be ignored without prior dispute
    assert_eq!(accounts[0].available, decimal("100.0"));
    assert_eq!(accounts[0].held, Decimal::ZERO);
}

#[test]
fn test_cannot_chargeback_without_dispute() {
    let mut engine = PaymentsEngine::new();
    engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: decimal("100.0"),
    });
    engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });

    let accounts = engine.get_accounts();
    // Chargeback should be ignored without prior dispute
    assert_eq!(accounts[0].available, decimal("100.0"));
    assert_eq!(accounts[0].total, decimal("100.0"));
    assert!(!accounts[0].locked);
}
