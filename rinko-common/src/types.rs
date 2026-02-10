use serde::{Deserialize, Serialize};

/// Platform enum matching proto definition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Platform {
    #[serde(rename = "qq")]
    QQ,
    #[serde(rename = "enterprise_wechat")]
    EnterpriseWechat,
    #[serde(rename = "telegram")]
    Telegram,
    #[serde(rename = "discord")]
    Discord,
    #[serde(rename = "llonebot")]
    LLOneBot,
}

impl Platform {
    pub fn to_proto(&self) -> crate::proto::Platform {
        match self {
            Platform::QQ => crate::proto::Platform::Qq,
            Platform::EnterpriseWechat => crate::proto::Platform::EnterpriseWechat,
            Platform::Telegram => crate::proto::Platform::Telegram,
            Platform::Discord => crate::proto::Platform::Discord,
            Platform::LLOneBot => crate::proto::Platform::Llonebot,
        }
    }

    pub fn from_proto(proto: crate::proto::Platform) -> Option<Self> {
        match proto {
            crate::proto::Platform::Unspecified => None,
            crate::proto::Platform::Qq => Some(Platform::QQ),
            crate::proto::Platform::EnterpriseWechat => Some(Platform::EnterpriseWechat),
            crate::proto::Platform::Telegram => Some(Platform::Telegram),
            crate::proto::Platform::Discord => Some(Platform::Discord),
            crate::proto::Platform::Llonebot => Some(Platform::LLOneBot),
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Platform::QQ => "qq",
            Platform::EnterpriseWechat => "enterprise_wechat",
            Platform::Telegram => "telegram",
            Platform::Discord => "discord",
            Platform::LLOneBot => "llonebot",
        }
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for Platform {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "qq" => Ok(Platform::QQ),
            "enterprise_wechat" | "enterprisewechat" => Ok(Platform::EnterpriseWechat),
            "telegram" => Ok(Platform::Telegram),
            "discord" => Ok(Platform::Discord),
            "llonebot" => Ok(Platform::LLOneBot),
            _ => Err(format!("Unknown platform: {}", s)),
        }
    }
}
