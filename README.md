# Payments Engine

A high-performance, type-safe payments processing engine written in Rust that handles deposits, withdrawals, disputes, and chargebacks.

## Features

- **Async CSV Processing**: Efficient streaming of large transaction files using `csv-async` and `tokio`
- **Precise Decimal Arithmetic**: Uses `rust_decimal` for accurate financial calculations (4 decimal places)
- **Type-Safe Transactions**: Strongly-typed enum for transaction types prevents invalid operations
- **Complete Dispute Workflow**: Full support for disputes, resolutions, and chargebacks
- **Memory Efficient**: Streams transactions through memory rather than loading entire dataset
- **Comprehensive Testing**: 13 unit tests covering all transaction scenarios

## Building and Running

```bash
# Build the project
cargo build --release

# Run with a CSV file
cargo run --release -- transactions.csv > accounts.csv

# Run tests
cargo test
```

## Usage

```bash
$ cargo run -- transactions.csv > accounts.csv
```

The program reads a CSV file with transactions and outputs account states to stdout.

### Input Format

The input CSV should have the following columns:
- `type`: Transaction type (deposit, withdrawal, dispute, resolve, chargeback)
- `client`: Client ID (u16)
- `tx`: Transaction ID (u32)
- `amount`: Transaction amount (Decimal, required for deposit/withdrawal, optional for others)

Example:
```csv
type, client, tx, amount
deposit, 1, 1, 1.0
deposit, 2, 2, 2.0
deposit, 1, 3, 2.0
withdrawal, 1, 4, 1.5
withdrawal, 2, 5, 3.0
```

### Output Format

The output CSV contains:
- `client`: Client ID
- `available`: Available funds for trading/withdrawal
- `held`: Funds held due to disputes
- `total`: Total funds (available + held)
- `locked`: Whether the account is locked (boolean)

Example:
```csv
client,available,held,total,locked
1,1.5,0,1.5,false
2,2.0,0,2.0,false
```

## Architecture

### Data Structures

#### InputTransaction Enum
Strongly-typed enum representing different transaction types:
- **Deposit**: Credits client account with amount
- **Withdrawal**: Debits client account (if sufficient funds available)
- **Dispute**: Holds funds from a referenced transaction
- **Resolve**: Releases disputed funds
- **Chargeback**: Reverses disputed transaction and locks account

#### Account
Tracks client account state:
- `available`: Funds available for operations
- `held`: Funds held due to disputes
- `total`: Total funds (available + held)
- `locked`: Account lock status

#### StoredTransaction
Internal representation for tracking transaction history needed for disputes.

### Design Decisions

1. **Async CSV Processing**
   - Uses `csv-async` with tokio for non-blocking I/O
   - Enables efficient processing of large files
   - Suitable for server environments with concurrent streams

2. **Precise Decimal Arithmetic**
   - Uses `rust_decimal` instead of f64 to avoid floating-point errors
   - Maintains 4 decimal places precision as specified
   - Suitable for financial calculations

3. **Dispute Handling**
   - Only deposits can be disputed (per banking conventions)
   - Withdrawals cannot be disputed (reversing would require re-crediting)
   - Disputes move funds from available to held
   - Chargebacks remove held funds and lock the account

4. **Transaction Storage**
   - Only successful deposits and withdrawals are stored
   - Failed withdrawals (insufficient funds) are not tracked
   - Necessary for dispute/resolve/chargeback operations

5. **Account Locking**
   - Accounts lock immediately upon chargeback
   - Locked accounts reject all future transactions
   - Prevents further fraud after chargeback

6. **Negative Available Balance**
   - Disputes can result in negative available balance
   - Example: Deposit $100, withdraw $60, dispute the deposit → available = -$60, held = $100, total = $40
   - Represents that client owes the disputed amount

## Assumptions

1. **Transaction Chronology**: Transactions in the CSV are processed in chronological order
2. **Client Creation**: Clients are created automatically on first transaction
3. **Transaction IDs**: Globally unique (u32), not guaranteed to be sequential
4. **Client IDs**: Valid u16 values, not guaranteed to be ordered
5. **Dispute Scope**: Only deposits can be disputed
6. **Error Handling**: Invalid transactions are silently skipped to handle partner errors gracefully
7. **Whitespace**: CSV parser handles flexible spacing and trimming

## Safety and Robustness

### Error Handling
- Gracefully skips malformed CSV rows
- Validates transaction amounts (must be > 0)
- Checks sufficient funds before withdrawals
- Validates dispute/resolve/chargeback preconditions

### Safety Guarantees
- No unsafe code blocks
- Rust's ownership system prevents data races
- Type system prevents invalid transaction states
- Decimal arithmetic prevents floating-point errors

### Correctness Verification
- 13 comprehensive unit tests
- Tests cover edge cases:
  - Insufficient funds
  - Disputes with negative available balance
  - Multiple clients
  - Account locking
  - Invalid operations (resolve/chargeback without dispute)
  - Precision handling

## Efficiency

### Memory Usage
- Streams CSV records one at a time
- Stores only necessary transaction history (for disputes)
- Account data stored in HashMap for O(1) lookups
- Transaction history stored in HashMap for O(1) dispute lookups

### Performance
- Async I/O prevents blocking on file operations
- Efficient CSV parsing with zero-copy deserialization where possible
- Minimal allocations using stack-allocated structs
- Suitable for processing millions of transactions

### Scalability
- Can handle transaction files larger than available RAM
- Suitable for server environments with concurrent TCP streams
- Can be extended to process multiple files in parallel

## Testing

Run the test suite:
```bash
cargo test
```

Test with sample data:
```bash
cargo run -- test_simple.csv > output.csv
cargo run -- test_disputes.csv > output.csv
cargo run -- test_chargeback.csv > output.csv
cargo run -- test_resolve.csv > output.csv
```

## Dependencies

- `csv-async`: Async CSV parsing
- `rust_decimal`: Precise decimal arithmetic
- `serde`: Serialization/deserialization
- `tokio`: Async runtime
- `futures`: Async stream utilities

## Future Improvements

1. **Parallel Processing**: Process multiple files concurrently
2. **Database Backend**: Store accounts and transactions in a database
3. **Metrics**: Add prometheus metrics for monitoring
4. **Logging**: Add structured logging with tracing
5. **CLI Options**: Add flags for verbose output, validation mode, etc.
6. **Withdrawal Disputes**: Support withdrawal disputes if business logic requires it
