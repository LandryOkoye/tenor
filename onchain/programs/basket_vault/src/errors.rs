use anchor_lang::prelude::*;

#[error_code]
pub enum VaultError {
    #[msg("a basket needs 2 to 4 legs")]
    LegCount,
    #[msg("leg weights must be positive and sum to 10000 bps")]
    Weights,
    #[msg("side must be 0 (YES) or 1 (NO)")]
    Side,
    #[msg("the same market appears twice")]
    DuplicateLeg,
    #[msg("fee too high")]
    Fee,
    #[msg("basket is not open for deposits")]
    NotOpen,
    #[msg("basket is not settled yet")]
    NotSettled,
    #[msg("amount below minimum")]
    AmountTooSmall,
    #[msg("leg accounts do not match the basket")]
    LegAccounts,
    #[msg("panta account layout not recognised")]
    PantaLayout,
    #[msg("minted fewer basket tokens than min_tokens_out")]
    Slippage,
    #[msg("nothing to mint or redeem")]
    Zero,
    #[msg("every leg's market must be resolved")]
    NotResolved,
    #[msg("the claim grace period after resolution has not passed")]
    GracePeriod,
    #[msg("arithmetic overflow")]
    Overflow,
}
