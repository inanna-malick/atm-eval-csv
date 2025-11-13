use fastnum::D128; // Signed decimal with 128-bit precision
use serde::Serialize;

type Decimal = D128;

/// Account balance and status information
#[derive(Debug, Clone, Serialize)]
pub struct Account {
    pub client: u16,
    #[serde(serialize_with = "serialize_decimal")]
    pub available: Decimal,
    #[serde(serialize_with = "serialize_decimal")]
    pub held: Decimal,
    #[serde(serialize_with = "serialize_decimal")]
    pub total: Decimal,
    pub locked: bool,
}

fn serialize_decimal<S>(decimal: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    // Format with 4 decimal places precision
    serializer.serialize_str(&format!("{:.4}", decimal))
}

impl Account {
    pub fn new(client_id: u16) -> Self {
        Account {
            client: client_id,
            available: Decimal::ZERO,
            held: Decimal::ZERO,
            total: Decimal::ZERO,
            locked: false,
        }
    }

    pub fn deposit(&mut self, amount: Decimal) {
        if !self.locked {
            self.available += amount;
            self.total += amount;
        }
    }

    pub fn withdraw(&mut self, amount: Decimal) -> bool {
        if !self.locked && self.available >= amount {
            self.available -= amount;
            self.total -= amount;
            true
        } else {
            false
        }
    }

    pub fn hold_funds(&mut self, amount: Decimal) {
        if !self.locked {
            self.available -= amount;
            self.held += amount;
        }
    }

    pub fn release_funds(&mut self, amount: Decimal) {
        if !self.locked && self.held >= amount {
            self.held -= amount;
            self.available += amount;
        }
    }

    pub fn chargeback(&mut self, amount: Decimal) {
        if self.held >= amount {
            self.held -= amount;
            self.total -= amount;
            self.locked = true;
        }
    }
}

/// Type of a stored transaction (for dispute tracking)
#[derive(Debug, Clone, PartialEq)]
pub enum TransactionType {
    Deposit,
    Withdrawal,
}

/// Internal representation of a stored transaction for dispute tracking
#[derive(Debug, Clone)]
pub struct StoredTransaction {
    #[allow(dead_code)]
    pub tx_id: u32,
    pub client_id: u16,
    pub tx_type: TransactionType,
    pub amount: Decimal,
    pub disputed: bool,
}

impl StoredTransaction {
    pub fn new(tx_id: u32, client_id: u16, tx_type: TransactionType, amount: Decimal) -> Self {
        StoredTransaction {
            tx_id,
            client_id,
            tx_type,
            amount,
            disputed: false,
        }
    }
}
