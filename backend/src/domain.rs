use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
        pub enum $name {
            $(#[serde(rename = $value)] $variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }
    };
}

string_enum!(AccountType {
    Brokerage => "brokerage",
    Bank => "bank",
    FundPlatform => "fund_platform",
    Pension => "pension",
    CryptoExchange => "crypto_exchange",
    SelfCustodyWallet => "self_custody_wallet",
    Other => "other",
});

string_enum!(AssetType {
    Stock => "stock",
    Etf => "etf",
    Fund => "fund",
    Bond => "bond",
    Cash => "cash",
    Deposit => "deposit",
    Gold => "gold",
    Crypto => "crypto",
    Stablecoin => "stablecoin",
    Other => "other",
});

string_enum!(TransactionType {
    Buy => "buy",
    Sell => "sell",
    Deposit => "deposit",
    Withdrawal => "withdrawal",
    Transfer => "transfer",
    Dividend => "dividend",
    Interest => "interest",
    ReturnOfCapital => "return_of_capital",
    Fee => "fee",
    Tax => "tax",
    StakingReward => "staking_reward",
    Airdrop => "airdrop",
    CorporateAction => "corporate_action",
    Adjustment => "adjustment",
    Valuation => "valuation",
});

string_enum!(LegType {
    Asset => "asset",
    Cash => "cash",
    Fee => "fee",
    Tax => "tax",
    Income => "income",
    Other => "other",
});
