use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// Strong newtype for an EVM chain ID (e.g. 1 for Ethereum Mainnet, 421614 for Arbitrum Sepolia).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
pub struct ChainId(pub u64);

impl ChainId {
    pub const ETHEREUM: Self = Self(1);
    pub const OPTIMISM: Self = Self(10);
    pub const BNB: Self = Self(56);
    pub const POLYGON: Self = Self(137);
    pub const BASE: Self = Self(8453);
    pub const ARBITRUM_ONE: Self = Self(42161);
    pub const SEPOLIA: Self = Self(11155111);
    pub const ARBITRUM_SEPOLIA: Self = Self(421614);
    pub const BASE_SEPOLIA: Self = Self(84532);

    #[must_use]
    pub const fn new(val: u64) -> Self {
        Self(val)
    }

    #[must_use]
    pub const fn as_u64(&self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn is_mainnet(&self) -> bool {
        matches!(
            *self,
            Self::ETHEREUM
                | Self::OPTIMISM
                | Self::BNB
                | Self::POLYGON
                | Self::BASE
                | Self::ARBITRUM_ONE
        )
    }

    #[must_use]
    pub fn is_testnet(&self) -> bool {
        matches!(
            *self,
            Self::SEPOLIA | Self::ARBITRUM_SEPOLIA | Self::BASE_SEPOLIA
        )
    }
}

impl From<u64> for ChainId {
    fn from(val: u64) -> Self {
        Self(val)
    }
}

impl From<ChainId> for u64 {
    fn from(id: ChainId) -> Self {
        id.0
    }
}

impl FromStr for ChainId {
    type Err = std::num::ParseIntError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let val = s.trim().parse::<u64>()?;
        Ok(Self(val))
    }
}

impl fmt::Display for ChainId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
