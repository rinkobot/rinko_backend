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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContentType {
    Unspecified,
    Text,
    Image,
    Video,
    File,
}

impl ContentType {
    pub fn to_proto(&self) -> crate::proto::ContentType {
        match self {
            ContentType::Unspecified => crate::proto::ContentType::Unspecified,
            ContentType::Text => crate::proto::ContentType::Text,
            ContentType::Image => crate::proto::ContentType::Image,
            ContentType::Video => crate::proto::ContentType::Video,
            ContentType::File => crate::proto::ContentType::File,
        }
    }

    pub fn from_proto(proto: crate::proto::ContentType) -> Option<Self> {
        match proto {
            crate::proto::ContentType::Unspecified => None,
            crate::proto::ContentType::Text => Some(ContentType::Text),
            crate::proto::ContentType::Image => Some(ContentType::Image),
            crate::proto::ContentType::Video => Some(ContentType::Video),
            crate::proto::ContentType::File => Some(ContentType::File),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum CommandType {
    #[default]
    Unspecified,
    SendMessage,
    Shutdown,
    GetStatus,
}

impl CommandType {
    pub fn to_proto(&self) -> crate::proto::CommandType {
        match self {
            CommandType::Unspecified => crate::proto::CommandType::Unspecified,
            CommandType::SendMessage => crate::proto::CommandType::SendMessage,
            CommandType::Shutdown => crate::proto::CommandType::Shutdown,
            CommandType::GetStatus => crate::proto::CommandType::GetStatus,
        }
    }

    pub fn from_proto(proto: crate::proto::CommandType) -> Self {
        match proto {
            crate::proto::CommandType::Unspecified => CommandType::Unspecified,
            crate::proto::CommandType::SendMessage => CommandType::SendMessage,
            crate::proto::CommandType::Shutdown => CommandType::Shutdown,
            crate::proto::CommandType::GetStatus => CommandType::GetStatus,
        }
    }
}
