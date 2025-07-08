#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContentType {
    None = 0,
    MessagePack = 1,
}

impl ContentType {
    fn from_u16(value: u16) -> anyhow::Result<Self> {
        match value {
            0 => Ok(ContentType::None),
            1 => Ok(ContentType::MessagePack),
            _ => Err(anyhow::anyhow!("Unknown content type: {}", value)),
        }
    }
}

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Flag {
    None = 0,
    GetConfig = 1,
    GetAction = 2,
}
impl Flag {
    fn from_u16(value: u16) -> anyhow::Result<Self> {
        match value {
            1 => Ok(Flag::GetConfig),
            2 => Ok(Flag::GetAction),
            _ => anyhow::bail!("Unknown flag value: {}", value),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
#[repr(packed)]
pub struct RRCPHeader {
    magic: u32,
    version: u32,
    pub body_length: u64,
    server_timestamp_ms: u64,
    content_type: ContentType,
    flag: Flag,
}

pub fn now_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}

const MAGIC_NUMBER: u32 = 0x7312;
impl RRCPHeader {
    pub fn new_with_flag(flag: Flag) -> Self {
        Self {
            magic: MAGIC_NUMBER,
            version: 1,
            body_length: 0,
            server_timestamp_ms: now_timestamp_ms(),
            content_type: ContentType::MessagePack,
            flag,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(std::mem::size_of::<Self>());
        buf.extend_from_slice(&self.magic.to_le_bytes());
        buf.extend_from_slice(&self.version.to_le_bytes());
        buf.extend_from_slice(&self.body_length.to_le_bytes());
        buf.extend_from_slice(&self.server_timestamp_ms.to_le_bytes());
        buf.extend_from_slice(&(self.content_type as u16).to_le_bytes());
        buf.extend_from_slice(&(self.flag as u16).to_le_bytes());
        buf
    }

    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        if bytes.len() < std::mem::size_of::<Self>() {
            return Err(anyhow::anyhow!("Bytes too short for RRCPHeader"));
        }
        if bytes.len() < std::mem::size_of::<Self>() {
            return Err(anyhow::anyhow!("Bytes too short for RRCPHeader"));
        }
        let mut magic = [0; 4];
        let mut version = [0; 4];
        let mut body_length = [0; 8];
        let mut server_timestamp_ms = [0; 8];
        let mut content_type = [0; 2];
        let mut flag = [0; 2];

        magic.copy_from_slice(&bytes[0..4]);
        version.copy_from_slice(&bytes[4..8]);
        body_length.copy_from_slice(&bytes[8..16]);
        server_timestamp_ms.copy_from_slice(&bytes[16..24]);
        content_type.copy_from_slice(&bytes[24..26]);
        flag.copy_from_slice(&bytes[26..28]);

        let magic = u32::from_le_bytes(magic);
        if magic != MAGIC_NUMBER {
            return Err(anyhow::anyhow!("Invalid magic number: {}", magic));
        }
        Ok(Self {
            magic,
            version: u32::from_le_bytes(version),
            body_length: u64::from_le_bytes(body_length),
            server_timestamp_ms: u64::from_le_bytes(server_timestamp_ms),
            content_type: ContentType::from_u16(u16::from_le_bytes(content_type))?,
            flag: Flag::from_u16(u16::from_le_bytes(flag))?,
        })
    }
}
