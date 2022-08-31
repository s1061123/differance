// types_040
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::types_100::*;
use super::types_common::Route;
use super::types_common::DNS;
use crate::ipnet;

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI040Interface {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "mac", default)]
    pub mac: String,
    #[serde(rename = "sandbox", default)]
    pub sandbox: String,
}

impl CNI040Interface {
    #[allow(unused)] // XXX to be removed
    pub fn convert_to_latest(&self) -> CNI100Interface {
        CNI100Interface {
            name: self.name.clone(),
            mac: self.mac.clone(),
            sandbox: self.sandbox.clone(),
        }
    }
    #[allow(unused)] // XXX to be removed
    pub fn convert_from_latest(latest: &CNI100Interface) -> CNI040Interface {
        CNI040Interface {
            name: latest.name.clone(),
            mac: latest.mac.clone(),
            sandbox: latest.sandbox.clone(),
        }
    }
}

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI040IPAddress {
    #[serde(rename = "version")]
    pub version: String,
    #[serde(rename = "interface", default)]
    pub interface: Option<u8>,
    #[serde(rename = "address")]
    pub address: ipnet::IPNet,
    #[serde(rename = "gateway", default)]
    pub gateway: Option<IpAddr>,
}

impl CNI040IPAddress {
    #[allow(unused)] // XXX to be removed
    pub fn convert_to_latest(&self) -> CNI100IPAddress {
        CNI100IPAddress {
            interface: self.interface,
            address: self.address.clone(),
            gateway: self.gateway,
        }
    }
    #[allow(unused)] // XXX to be removed
    pub fn convert_from_latest(latest: &CNI100IPAddress) -> CNI040IPAddress {
        CNI040IPAddress {
            version: match latest.address.ip {
                std::net::IpAddr::V4(_) => "4".to_string(),
                std::net::IpAddr::V6(_) => "6".to_string(),
            },
            interface: latest.interface,
            address: latest.address.clone(),
            gateway: latest.gateway,
        }
    }
}

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI040Result {
    #[serde(rename = "cniVersion", default)]
    pub cni_version: String,
    #[serde(rename = "interfaces", default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<CNI040Interface>,
    #[serde(rename = "ips", default, skip_serializing_if = "Vec::is_empty")]
    pub ips: Vec<CNI040IPAddress>,
    #[serde(rename = "routes", default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
    #[serde(rename = "dns", default)]
    pub dns: DNS,
}

impl CNI040Result {
    #[allow(unused)] // XXX to be removed
    pub fn convert_to_latest(&self) -> CNI100Result {
        CNI100Result {
            cni_version: "1.0.0".to_string(),
            interfaces: self
                .interfaces
                .iter()
                .map(|x| x.convert_to_latest())
                .collect(),
            ips: self.ips.iter().map(|x| x.convert_to_latest()).collect(),
            routes: self.routes.clone(),
            dns: self.dns.clone(),
        }
    }
    #[allow(unused)] // XXX to be removed
    pub fn convert_from_latest(latest: &CNI100Result, cni_version: &str) -> CNI040Result {
        CNI040Result {
            cni_version: cni_version.to_string(),
            interfaces: latest
                .interfaces
                .iter()
                .map(CNI040Interface::convert_from_latest)
                .collect(),
            ips: latest
                .ips
                .iter()
                .map(CNI040IPAddress::convert_from_latest)
                .collect(),
            routes: latest.routes.clone(),
            dns: latest.dns.clone(),
        }
    }
}
