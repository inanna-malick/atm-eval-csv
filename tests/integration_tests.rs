use payments_engine::transaction::InputTransaction;
use payments_engine::types::{SignedDecimal, UnsignedDecimal};
use payments_engine::PaymentsEngine;

fn signed_decimal(s: &str) -> SignedDecimal {
    s.parse()
        .unwrap_or_else(|_| panic!("failed to parse signed decimal: {}", s))
}

fn unsigned_decimal(s: &str) -> UnsignedDecimal {
    s.parse()
        .unwrap_or_else(|_| panic!("failed to parse unsigned decimal: {}", s))
}

#[test]
fn test_basic_deposit() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();

    assert_eq!(engine.accounts.len(), 1);
    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.client, 1);
    assert_eq!(account.available, signed_decimal("100.0"));
    assert_eq!(account.total, signed_decimal("100.0"));
    assert_eq!(account.held, UnsignedDecimal::ZERO);
}

#[test]
fn test_basic_withdrawal() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: unsigned_decimal("30.0"),
        })
        .unwrap();

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("70.0"));
    assert_eq!(account.total, signed_decimal("70.0"));
}

#[test]
fn test_insufficient_funds_withdrawal() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("50.0"),
        })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: unsigned_decimal("100.0"),
    });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("50.0"));
    assert_eq!(account.total, signed_decimal("50.0"));
}

#[test]
fn test_dispute() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, SignedDecimal::ZERO);
    assert_eq!(account.held, unsigned_decimal("100.0"));
    assert_eq!(account.total, signed_decimal("100.0"));
    assert!(!account.locked);
}

#[test]
fn test_dispute_with_negative_available() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: unsigned_decimal("60.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("-60.0"));
    assert_eq!(account.held, unsigned_decimal("100.0"));
    assert_eq!(account.total, signed_decimal("40.0"));
}

#[test]
fn test_resolve() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Resolve { client: 1, tx: 1 })
        .unwrap();

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("100.0"));
    assert_eq!(account.held, UnsignedDecimal::ZERO);
    assert_eq!(account.total, signed_decimal("100.0"));
    assert!(!account.locked);
}

#[test]
fn test_chargeback() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 })
        .unwrap();

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, SignedDecimal::ZERO);
    assert_eq!(account.held, UnsignedDecimal::ZERO);
    assert_eq!(account.total, SignedDecimal::ZERO);
    assert!(account.locked);
}

#[test]
fn test_locked_account_prevents_deposits() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 2,
        amount: unsigned_decimal("50.0"),
    });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.total, SignedDecimal::ZERO);
    assert!(account.locked);
}

#[test]
fn test_multiple_clients() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 2,
            tx: 2,
            amount: unsigned_decimal("200.0"),
        })
        .unwrap();

    assert_eq!(engine.accounts.len(), 2);
    let account1 = engine.accounts.get(&1).unwrap();
    assert_eq!(account1.client, 1);
    assert_eq!(account1.total, signed_decimal("100.0"));
    let account2 = engine.accounts.get(&2).unwrap();
    assert_eq!(account2.client, 2);
    assert_eq!(account2.total, signed_decimal("200.0"));
}

#[test]
fn test_dispute_only_affects_deposits() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: unsigned_decimal("30.0"),
        })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 2 });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("70.0"));
    assert_eq!(account.held, UnsignedDecimal::ZERO);
}

#[test]
fn test_precision() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("1.2345"),
        })
        .unwrap();

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("1.2345"));
}

#[test]
fn test_cannot_resolve_without_dispute() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Resolve { client: 1, tx: 1 });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("100.0"));
    assert_eq!(account.held, UnsignedDecimal::ZERO);
}

#[test]
fn test_cannot_chargeback_without_dispute() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("100.0"));
    assert_eq!(account.total, signed_decimal("100.0"));
    assert!(!account.locked);
}

#[test]
fn test_duplicate_deposit() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Deposit {
        client: 1,
        tx: 1,
        amount: unsigned_decimal("50.0"),
    });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("100.0"));
}

#[test]
fn test_duplicate_withdrawal() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Withdrawal {
            client: 1,
            tx: 2,
            amount: unsigned_decimal("30.0"),
        })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: unsigned_decimal("20.0"),
    });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, signed_decimal("70.0"));
}

#[test]
fn test_already_disputed_error() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Dispute { client: 1, tx: 1 });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.available, SignedDecimal::ZERO);
    assert_eq!(account.held, unsigned_decimal("100.0"));
}

#[test]
fn test_withdrawal_from_locked_account_error() {
    let mut engine = PaymentsEngine::default();
    engine
        .process_transaction(InputTransaction::Deposit {
            client: 1,
            tx: 1,
            amount: unsigned_decimal("100.0"),
        })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Dispute { client: 1, tx: 1 })
        .unwrap();
    engine
        .process_transaction(InputTransaction::Chargeback { client: 1, tx: 1 })
        .unwrap();
    let result = engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 2,
        amount: unsigned_decimal("50.0"),
    });
    assert!(result.is_err());

    let account = engine.accounts.get(&1).unwrap();
    assert_eq!(account.total, SignedDecimal::ZERO);
    assert!(account.locked);
}

#[test]
fn test_withdrawal_from_nonexistent_account() {
    let mut engine = PaymentsEngine::default();
    let result = engine.process_transaction(InputTransaction::Withdrawal {
        client: 1,
        tx: 1,
        amount: unsigned_decimal("50.0"),
    });
    assert!(result.is_err());
    assert_eq!(engine.accounts.len(), 0);
}
