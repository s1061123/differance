// types_020
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::types_100::*;
use super::types_common::Route;
use super::types_common::DNS;
use crate::ipnet;

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CNI020IPAddress {
    #[serde(rename = "ip")]
    pub address: ipnet::IPNet,
    #[serde(rename = "gateway", default)]
    pub gateway: Option<IpAddr>,
    #[serde(rename = "routes", default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
}

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI020Result {
    #[serde(rename = "cniVersion", default)]
    pub cni_version: String,
    #[serde(rename = "ip4", default)]
    pub ip4: Option<CNI020IPAddress>,
    #[serde(rename = "ip6", default)]
    pub ip6: Option<CNI020IPAddress>,
    #[serde(rename = "dns", default)]
    pub dns: DNS,
}

/*
*/
impl CNI020Result {
    #[allow(unused)] // XXX to be removed
    pub fn convert_to_latest(&self) -> CNI100Result {
        CNI100Result {
            cni_version: "1.0.0".to_string(),
            interfaces: vec![],
            ips: vec![self.ip4.clone(), self.ip6.clone()]
                .into_iter()
                .filter_map(|x| match x {
                    Some(y) => Some(CNI100IPAddress {
                        interface: None,
                        address: y.address.clone(),
                        gateway: y.gateway,
                    }),
                    None => None,
                })
                .collect::<Vec<CNI100IPAddress>>(),
            routes: vec![self.ip4.clone(), self.ip6.clone()]
                .into_iter()
                .filter_map(|x| match x {
                    Some(y) => Some(y.routes),
                    None => None,
                })
                .flatten()
                .collect::<Vec<Route>>(),
            dns: self.dns.clone(),
        }
    }

    #[allow(unused)] // XXX to be removed
    pub fn convert_from_latest(latest: &CNI100Result, cni_version: &str) -> CNI020Result {
        let v4addr_opt = latest.ips.iter().fold(None, |v4addr, addr| {
            if v4addr.is_some() {
                v4addr
            } else if addr.address.ip.is_ipv4() {
                Some((addr.address.clone(), addr.gateway))
            } else {
                None
            }
        });
        let v6addr_opt = latest.ips.iter().fold(None, |v6addr, addr| {
            if v6addr.is_some() {
                v6addr
            } else if addr.address.ip.is_ipv6() {
                Some((addr.address.clone(), addr.gateway))
            } else {
                None
            }
        });
        let v4_routes = latest
            .routes
            .iter()
            .fold(vec![], |mut v4_result: Vec<Route>, route| {
                if route.dst.ip.is_ipv4() {
                    v4_result.push(route.clone())
                }
                v4_result
            });
        let v6_routes = latest
            .routes
            .iter()
            .fold(vec![], |mut v6_result: Vec<Route>, route| {
                if route.dst.ip.is_ipv6() {
                    v6_result.push(route.clone())
                }
                v6_result
            });
        let v4_addr = v4addr_opt.map(|(v4_address, v4_gateway)| CNI020IPAddress {
            address: v4_address,
            gateway: v4_gateway,
            routes: v4_routes,
        });
        let v6_addr = v6addr_opt.map(|(v6_address, v6_gateway)| CNI020IPAddress {
            address: v6_address,
            gateway: v6_gateway,
            routes: v6_routes,
        });
        CNI020Result {
            cni_version: cni_version.to_string(),
            ip4: v4_addr,
            ip6: v6_addr,
            dns: latest.dns.clone(),
        }
    }
}

/*
#[test]
fn test() -> Result<()> {
    Ok(())
}
*/
