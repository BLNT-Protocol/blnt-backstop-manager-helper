use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum HelperError {
    InvalidConfig = 1700,
    InvalidManager = 1701,
    InvalidBackstop = 1702,
    InvalidPool = 1703,
    InvalidTierToken = 1704,
    InvalidPair = 1705,
    InvalidAmount = 1706,
    InvalidState = 1707,
    InsufficientBalance = 1708,
    BalanceMismatch = 1709,
    Overflow = 1710,
}
