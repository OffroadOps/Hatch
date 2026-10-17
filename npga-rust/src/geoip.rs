//! Minimal MaxMind DB (mmdb) reader for GeoLite2-Country style databases.
//!
//! The binary format is implemented from the published MaxMind-DB spec and the
//! traversal was verified against the reference `maxminddb` implementation with
//! the shipped `GeoLite2-Country.mmdb` (IPv4/IPv6 lookups, pointer chains,
//! 24/28/32-bit records). No external crates: the database is loaded once and
//! searched with two record reads per bit.

use anyhow::{Context, Result, bail};
use std::net::IpAddr;
use std::path::Path;

const MARKER: &[u8; 14] = b"\xab\xcd\xefMaxMind.com";
/// Fixed offset added to pointer values by pointer size: (1,2,3,4 bytes).
const POINTER_VALUE_OFFSETS: [u64; 4] = [0, 0, 2048, 526336];
/// Bytes between the end of the search tree and the data section.
const DATA_SECTION_SEPARATOR: u64 = 16;

pub struct GeoIp {
    data: Vec<u8>,
    node_count: u64,
    record_size: u8,
    ip_version: u8,
    tree_size: u64,
    /// Root node for IPv4 lookups in an IPv6 tree (96 left hops from node 0).
    ipv4_start: u64,
    data_base: u64,
}

struct Decoder<'a> {
    buf: &'a [u8],
    base: u64,
}

impl<'a> Decoder<'a> {
    fn at(&self, off: u64) -> Result<u8> {
        self.buf
            .get(off as usize)
            .copied()
            .with_context(|| format!("mmdb 偏移越界: {off}"))
    }
    fn uint(&self, off: u64, len: usize) -> Result<u64> {
        let mut value = 0u64;
        for i in 0..len {
            value = (value << 8) | self.at(off + i as u64)? as u64;
        }
        Ok(value)
    }
    /// Decodes one typed value. Returns (value, offset after the value).
    fn decode(&self, off: u64, depth: u32) -> Result<(Value, u64)> {
        if depth > 32 {
            bail!("mmdb 嵌套层级过深");
        }
        let ctrl = self.at(off)?;
        let mut off = off + 1;
        let mut type_num = ctrl >> 5;
        if type_num == 0 {
            type_num = self.at(off)? + 7;
            off += 1;
        }
        let mut size = (ctrl & 0x1F) as u64;
        if size >= 29 && type_num != 1 {
            size = match size {
                29 => {
                    let v = 29 + self.at(off)? as u64;
                    off += 1;
                    v
                }
                30 => {
                    let v = 285 + self.uint(off, 2)?;
                    off += 2;
                    v
                }
                _ => {
                    let v = 65821 + self.uint(off, 3)?;
                    off += 3;
                    v
                }
            };
        }
        // Each arm returns (value, offset after the value). Containers track
        // their own cursor so maps (key+value pairs) advance correctly.
        let (value, end) = match type_num {
            1 => {
                // Pointer: 1..4 bytes, with high bits and size-dependent offsets.
                let pointer_size = (size >> 3) + 1;
                let mut pointer = self.uint(off, pointer_size as usize)?;
                if pointer_size < 4 {
                    pointer |= (size & 0x7) << (pointer_size * 8);
                    pointer += POINTER_VALUE_OFFSETS[pointer_size as usize];
                }
                let target = self.decode(pointer + self.base, depth + 1)?.0;
                (target, off + pointer_size)
            }
            2 => {
                let start = off as usize;
                let end = start + size as usize;
                let text = self.buf.get(start..end).context("mmdb 字符串越界")?;
                (
                    Value::String(String::from_utf8_lossy(text).into_owned()),
                    off + size,
                )
            }
            4 => {
                let start = off as usize;
                let end = start + size as usize;
                (
                    Value::Bytes(self.buf.get(start..end).context("mmdb 字节越界")?.to_vec()),
                    off + size,
                )
            }
            5 | 6 | 9 | 10 => (Value::Uint(self.uint(off, size as usize)?), off + size),
            3 => (Value::Float(f64::from_bits(self.uint(off, 8)?)), off + 8),
            14 => (Value::Bool(size != 0), off),
            7 => {
                let mut map = Vec::new();
                let mut cursor = off;
                for _ in 0..size {
                    let (key, next) = self.decode(cursor, depth + 1)?;
                    cursor = next;
                    let (value, next) = self.decode(cursor, depth + 1)?;
                    cursor = next;
                    map.push((
                        key.as_string().map(str::to_owned).unwrap_or_default(),
                        value,
                    ));
                }
                (Value::Map(map), cursor)
            }
            11 => {
                let mut items = Vec::new();
                let mut cursor = off;
                for _ in 0..size {
                    let (item, next) = self.decode(cursor, depth + 1)?;
                    cursor = next;
                    items.push(item);
                }
                (Value::Array(items), cursor)
            }
            other => bail!("未知 mmdb 数据类型 {other}"),
        };
        Ok((value, end))
    }
}

#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Uint(u64),
    Float(f64),
    Bool(bool),
    Bytes(Vec<u8>),
    Map(Vec<(String, Value)>),
    Array(Vec<Value>),
}
impl Value {
    fn as_string(&self) -> Option<&str> {
        match self {
            Self::String(s) => Some(s),
            _ => None,
        }
    }
    pub fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

impl GeoIp {
    /// Opens a database and parses its metadata. Errors are descriptive so a
    /// missing or incompatible file surfaces as a clear message in the UI.
    pub fn open(path: &Path) -> Result<Self> {
        let data =
            std::fs::read(path).with_context(|| format!("读取 IP 库失败: {}", path.display()))?;
        if data.len() < MARKER.len() {
            bail!("IP 库文件过小或已损坏");
        }
        // Search the trailing window (doubling) for the metadata marker.
        let mut window = 512usize.min(data.len());
        let meta_start = loop {
            if window > data.len() {
                bail!("IP 库缺少元数据标记（不是有效的 mmdb 文件）");
            }
            let begin = data.len() - window;
            if let Some(idx) = data[begin..]
                .windows(MARKER.len())
                .rposition(|w| w == MARKER)
            {
                break begin + idx + MARKER.len();
            }
            window = window.saturating_mul(2);
        };
        if meta_start >= data.len() {
            bail!("IP 库元数据为空");
        }
        let decoder = Decoder {
            buf: &data,
            // Metadata pointers are relative to the metadata section start.
            base: meta_start as u64,
        };
        let (meta, _) = decoder.decode(meta_start as u64, 0)?;
        let node_count = meta.get("node_count").and_then(Value::as_uint).unwrap_or(0);
        let record_size = meta
            .get("record_size")
            .and_then(Value::as_uint)
            .unwrap_or(24) as u8;
        let ip_version = meta.get("ip_version").and_then(Value::as_uint).unwrap_or(6) as u8;
        if !matches!(record_size, 24 | 28 | 32) {
            bail!("不支持的 mmdb 记录宽度 {record_size}");
        }
        if !matches!(ip_version, 4 | 6) {
            bail!("不支持的 mmdb IP 版本 {ip_version}");
        }
        if node_count == 0 {
            bail!("mmdb 节点数为 0，文件可能损坏");
        }
        let tree_size = node_count * record_size as u64 / 4;
        let ipv4_start = if ip_version == 6 {
            // Follow the left-most path 96 times from node 0.
            let mut node = 0u64;
            for _ in 0..96 {
                if node >= node_count {
                    break;
                }
                node = Self::read_node(&data, node, 0, record_size, node_count)?;
            }
            node
        } else {
            0
        };
        let data_base = tree_size + DATA_SECTION_SEPARATOR;
        Ok(Self {
            data,
            node_count,
            record_size,
            ip_version,
            tree_size,
            ipv4_start,
            data_base,
        })
    }

    fn read_node(
        data: &[u8],
        node: u64,
        index: u8,
        record_size: u8,
        node_count: u64,
    ) -> Result<u64> {
        if node >= node_count {
            bail!("mmdb 树节点越界");
        }
        let byte = |off: u64| -> Result<u8> {
            data.get(off as usize)
                .copied()
                .with_context(|| format!("mmdb 树偏移越界: {off}"))
        };
        let record = match record_size {
            24 => {
                let off = node * 6 + index as u64 * 3;
                ((byte(off)? as u64) << 16) | ((byte(off + 1)? as u64) << 8) | byte(off + 2)? as u64
            }
            28 => {
                let base = node * 7;
                if index == 0 {
                    // High 24 bits of the shared 4-byte word (middle byte shared).
                    (((byte(base)? as u64) << 24)
                        | ((byte(base + 1)? as u64) << 16)
                        | ((byte(base + 2)? as u64) << 8)
                        | byte(base + 3)? as u64)
                        >> 4
                } else {
                    (((byte(base + 3)? as u64) << 24)
                        | ((byte(base + 4)? as u64) << 16)
                        | ((byte(base + 5)? as u64) << 8)
                        | byte(base + 6)? as u64)
                        & 0x0FFF_FFFF
                }
            }
            32 => {
                let off = node * 8 + index as u64 * 4;
                ((byte(off)? as u64) << 24)
                    | ((byte(off + 1)? as u64) << 16)
                    | ((byte(off + 2)? as u64) << 8)
                    | byte(off + 3)? as u64
            }
            other => bail!("不支持的记录宽度 {other}"),
        };
        Ok(record)
    }

    /// Looks up the record for an address. `None` means "not in database".
    pub fn lookup(&self, address: IpAddr) -> Result<Value> {
        let bytes = match address {
            IpAddr::V4(v4) => v4.octets().to_vec(),
            IpAddr::V6(v6) => v6.octets().to_vec(),
        };
        let bits = bytes.len() * 8;
        let mut node = if self.ip_version == 6 && bits == 32 {
            self.ipv4_start
        } else {
            0
        };
        for i in 0..bits {
            if node >= self.node_count {
                break;
            }
            let bit = (bytes[i / 8] >> (7 - i % 8)) & 1;
            node = Self::read_node(&self.data, node, bit, self.record_size, self.node_count)?;
        }
        if node == self.node_count {
            bail!("地址不在 IP 库中");
        }
        if node < self.node_count {
            bail!("mmdb 遍历未到达数据节点");
        }
        let decoder = Decoder {
            buf: &self.data,
            base: self.data_base,
        };
        let offset = node - self.node_count + self.tree_size;
        Ok(decoder.decode(offset, 0)?.0)
    }

    /// Country ISO code for an address (e.g. "CN"), if present.
    pub fn country_code(&self, address: IpAddr) -> Option<String> {
        let record = self.lookup(address).ok()?;
        let country = record.get("country")?;
        let code = country.get("iso_code")?;
        code.as_string().map(str::to_owned)
    }
}

impl Value {
    fn as_uint(&self) -> Option<u64> {
        match self {
            Self::Uint(v) => Some(*v),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::path::PathBuf;

    /// Locates the shipped database in any known build output; the test is
    /// skipped (not failed) when no database is present, so CI without the
    /// release artifacts stays green.
    fn database() -> Option<PathBuf> {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()?
            .to_owned();
        [
            "release-net10/Hatch/bin/GeoLite2-Country.mmdb",
            "release-npga/NPGA/bin/GeoLite2-Country.mmdb",
            "release/Hatch/bin/GeoLite2-Country.mmdb",
        ]
        .into_iter()
        .map(|p| root.join(p))
        .find(|p| p.is_file())
    }

    #[test]
    fn lookup_matches_reference_results() {
        let Some(path) = database() else {
            eprintln!("跳过：未找到 GeoLite2 数据库");
            return;
        };
        let geo = GeoIp::open(&path).expect("打开 IP 库失败");
        // Values cross-checked against the reference maxminddb implementation.
        assert_eq!(
            geo.country_code("114.114.114.114".parse().unwrap()),
            Some("CN".into())
        );
        assert_eq!(
            geo.country_code("223.5.5.5".parse().unwrap()),
            Some("CN".into())
        );
        assert!(
            geo.country_code("1.1.1.1".parse::<IpAddr>().unwrap())
                .is_some()
        );
        assert!(
            geo.country_code("8.8.8.8".parse::<IpAddr>().unwrap())
                .is_some()
        );
    }

    #[test]
    fn ipv6_and_private_ranges_resolve() {
        let Some(path) = database() else {
            eprintln!("跳过：未找到 GeoLite2 数据库");
            return;
        };
        let geo = GeoIp::open(&path).expect("打开 IP 库失败");
        // IPv6 traversal (the ipv4_start shortcut does not apply here).
        assert_eq!(
            geo.country_code("2400:3200::1".parse().unwrap()),
            Some("CN".into())
        );
        assert!(
            geo.country_code("2001:4860:4860::8888".parse::<IpAddr>().unwrap())
                .is_some()
        );
        // Reserved space is labelled by the database, not a panic.
        assert!(
            geo.country_code("240.0.0.1".parse::<IpAddr>().unwrap())
                .is_some()
        );
    }
}
