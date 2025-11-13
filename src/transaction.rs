use crate::types::UnsignedDecimal;
use serde::Deserialize;

#[derive(Debug, Clone)]
pub enum InputTransaction {
    Deposit {
        client: u16,
        tx: u32,
        amount: UnsignedDecimal,
    },
    Withdrawal {
        client: u16,
        tx: u32,
        amount: UnsignedDecimal,
    },
    Dispute {
        client: u16,
        tx: u32,
    },
    Resolve {
        client: u16,
        tx: u32,
    },
    Chargeback {
        client: u16,
        tx: u32,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum TransactionType {
    Deposit,
    Withdrawal,
    Dispute,
    Resolve,
    Chargeback,
}

#[derive(Deserialize)]
struct TransactionRecord {
    #[serde(rename = "type")]
    tx_type: TransactionType,
    client: u16,
    tx: u32,
    amount: Option<UnsignedDecimal>,
}

impl TryFrom<TransactionRecord> for InputTransaction {
    type Error = &'static str;

    fn try_from(record: TransactionRecord) -> Result<Self, Self::Error> {
        match record.tx_type {
            TransactionType::Deposit => {
                let amount = record.amount.ok_or("missing amount for deposit")?;
                if amount == UnsignedDecimal::ZERO {
                    return Err("deposit amount cannot be zero");
                }
                Ok(InputTransaction::Deposit {
                    client: record.client,
                    tx: record.tx,
                    amount,
                })
            }
            TransactionType::Withdrawal => {
                let amount = record.amount.ok_or("missing amount for withdrawal")?;
                if amount == UnsignedDecimal::ZERO {
                    return Err("withdrawal amount cannot be zero");
                }
                Ok(InputTransaction::Withdrawal {
                    client: record.client,
                    tx: record.tx,
                    amount,
                })
            }
            TransactionType::Dispute => Ok(InputTransaction::Dispute {
                client: record.client,
                tx: record.tx,
            }),
            TransactionType::Resolve => Ok(InputTransaction::Resolve {
                client: record.client,
                tx: record.tx,
            }),
            TransactionType::Chargeback => Ok(InputTransaction::Chargeback {
                client: record.client,
                tx: record.tx,
            }),
        }
    }
}

impl<'de> Deserialize<'de> for InputTransaction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let record = TransactionRecord::deserialize(deserializer)?;
        InputTransaction::try_from(record).map_err(serde::de::Error::custom)
    }
}
