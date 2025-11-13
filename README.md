# Payments Engine

Rust payments processing engine for deposits, withdrawals, disputes, and chargebacks.

## Usage

```bash
cargo run -- transactions.csv > accounts.csv
```

## Testing

- engine test cases in tests/integration_tests.rs
- manual testing with test csv files: test_*.csv

## Assumptions

- implemented with strict parsing (eg negative withdrawals treated as malformed)
- logs per-row errors via stderr and continues on error/parse error (might not do this in an IRL application, but this seemed to be required by the rubric)
- disputes/chargebacks only applicable to deposits
- available balance can be negative after dispute

## Choices

- uses fastnum for non-float decimals
- uses enum for error cases instead of failing with string/panic
- uses csv-async for future extensibility w/ streaming data

## LLM use disclosure

Used Claude Sonnet 4.5 as smart autocomplete, main human value add from chosing strategies/techniques (encoding invariants at type level, etc)
