use std::fmt;
use std::net::IpAddr;
use std::str::FromStr;
use std::net::IpAddr::{V4, V6};
use std::net::{Ipv4Addr,Ipv6Addr};

use schemars::JsonSchema;
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serializer};
use thiserror::Error;

#[derive(Clone, PartialEq, Eq, JsonSchema)]
pub struct IPNet {
    pub ip: IpAddr,
    pub netmask_len: u8,
}

impl IPNet {
    pub fn get_network_ip(&self) -> IpAddr {
        match self.ip {
            V4(v4addr) => {
                let num_addr:u32 = u32::from(v4addr);
                let shift_bit = 32 - self.netmask_len;
                V4(Ipv4Addr::from(num_addr >> shift_bit << shift_bit))
            },
            V6(v6addr) => {
                let num_addr:u128 = u128::from(v6addr);
                let shift_bit = 128 - self.netmask_len;
                V6(Ipv6Addr::from(num_addr >> shift_bit << shift_bit))
            },
        }
    }
}

impl fmt::Debug for IPNet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("{0}/{1}", self.ip, self.netmask_len))
    }
}

impl fmt::Display for IPNet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_fmt(format_args!("{0}/{1}", self.ip, self.netmask_len))
    }
}

#[test]
fn test_ipnet_debug_v4() {
    assert_eq!(
        &format!(
            "{:?}",
            IPNet {
                ip: "10.1.1.1".parse().unwrap(),
                netmask_len: 24,
            }
        ),
        "10.1.1.1/24"
    );
}

#[test]
fn test_ipnet_debug_v6() {
    assert_eq!(
        &format!(
            "{:?}",
            IPNet {
                ip: "10::1".parse().unwrap(),
                netmask_len: 64,
            }
        ),
        "10::1/64"
    );
}

#[test]
fn test_ipnet_display_v4() {
    assert_eq!(
        &format!(
            "{}",
            IPNet {
                ip: "10.1.1.1".parse().unwrap(),
                netmask_len: 24,
            }
        ),
        "10.1.1.1/24"
    );
}

#[test]
fn test_ipnet_display_v6() {
    assert_eq!(
        &format!(
            "{}",
            IPNet {
                ip: "10::1".parse().unwrap(),
                netmask_len: 64,
            }
        ),
        "10::1/64"
    );
}

impl serde::Serialize for IPNet {
    fn serialize<S>(&self, serializer: S) -> Result<<S as Serializer>::Ok, <S as Serializer>::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(format!("{0}/{1}", self.ip, self.netmask_len).as_str())
    }
}

#[test]
fn test_serde_json_serialize_v4() {
    let test_var_v4 = IPNet {
        ip: "10.1.1.1".parse().unwrap(),
        netmask_len: 24,
    };
    assert_eq!(
        serde_json::to_string(&test_var_v4).unwrap(),
        "\"10.1.1.1/24\""
    );
}

#[test]
fn test_serde_json_serialize_6() {
    let test_var_v6 = IPNet {
        ip: "10::1".parse().unwrap(),
        netmask_len: 64,
    };
    assert_eq!(serde_json::to_string(&test_var_v6).unwrap(), "\"10::1/64\"");
}

// Deserialize IPNet struct

struct IPNetVisitor;
impl<'de> Visitor<'de> for IPNetVisitor {
    type Value = IPNet;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("`ip` and `netmask_len`")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        let ipnet_val: Vec<&str> = v.split('/').collect();
        if ipnet_val.len() != 2 {
            return Err(E::custom(format!("invalid ip/mask: {}", v)));
        }
        Ok(IPNet {
            ip: match ipnet_val[0].parse() {
                Ok(i) => i,
                Err(_) => {
                    return Err(E::custom(format!(
                        "failed to parse IP: {} from {}",
                        ipnet_val[0], v
                    )))
                }
            },
            netmask_len: match ipnet_val[1].parse::<u8>() {
                Ok(n) => n,
                Err(_) => {
                    return Err(E::custom(format!(
                        "failed to parse len: {} from {}",
                        ipnet_val[1], v
                    )))
                }
            },
        })
    }
}

impl<'de> Deserialize<'de> for IPNet {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(IPNetVisitor)
    }
}

#[test]
fn deserialize_ipnet_v4() {
    let deserialize_str = "\"1.1.1.1/24\"";
    let deserialize_ipnet: IPNet = serde_json::from_str(deserialize_str).unwrap();

    assert_eq!(
        deserialize_ipnet,
        IPNet {
            ip: "1.1.1.1".parse().unwrap(),
            netmask_len: 24,
        }
    );
}

#[test]
fn deserialize_ipnet_v6() {
    let deserialize_str = "\"1::1/64\"";
    let deserialize_ipnet: IPNet = serde_json::from_str(deserialize_str).unwrap();

    assert_eq!(
        deserialize_ipnet,
        IPNet {
            ip: "1::1".parse().unwrap(),
            netmask_len: 64,
        }
    );
}

#[derive(Debug, Error)]
pub enum ParseIPNetError {
    #[error("failed to parse IPNet: {0}")]
    ParseIPNetError(String),
}

impl FromStr for IPNet {
    type Err = ParseIPNetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ipnet_val: Vec<&str> = s.split('/').collect();
        if ipnet_val.len() != 2 {
            return Err(ParseIPNetError::ParseIPNetError(format!(
                "invalid ip/mask: {}",
                s
            )));
        }
        Ok(IPNet {
            ip: match ipnet_val[0].parse() {
                Ok(i) => i,
                Err(_) => {
                    return Err(ParseIPNetError::ParseIPNetError(format!(
                        "failed to parse IP: {} from {}",
                        ipnet_val[0], s
                    )))
                }
            },
            netmask_len: match ipnet_val[1].parse::<u8>() {
                Ok(n) => n,
                Err(_) => {
                    return Err(ParseIPNetError::ParseIPNetError(format!(
                        "failed to parse len: {} from {}",
                        ipnet_val[1], s
                    )))
                }
            },
        })
    }
}

#[test]
fn parse_ipnet_fromstr_v4() {
    let target_str = "1.1.1.1/24";
    let parsed_ipnet = IPNet::from_str(target_str).unwrap();

    assert_eq!(
        parsed_ipnet,
        IPNet {
            ip: "1.1.1.1".parse().unwrap(),
            netmask_len: 24,
        }
    );
}

#[test]
fn parse_ipnet_fromstr_v6() {
    let target_str = "ff02::1/64";
    let parsed_ipnet = IPNet::from_str(target_str).unwrap();

    assert_eq!(
        parsed_ipnet,
        IPNet {
            ip: "ff02::1".parse().unwrap(),
            netmask_len: 64,
        }
    );
}
