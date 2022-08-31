// types_100
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

use super::types_common::*;
use crate::ipnet;

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI100Interface {
    #[serde(rename = "name")]
    pub name: String,
    #[serde(rename = "mac", default)]
    pub mac: String,
    #[serde(rename = "sandbox", default)]
    pub sandbox: String,
}

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI100IPAddress {
    #[serde(rename = "interface", default)]
    pub interface: Option<u8>,
    #[serde(rename = "address")]
    pub address: ipnet::IPNet,
    #[serde(rename = "gateway", default)]
    pub gateway: Option<IpAddr>,
}

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Debug)]
pub struct CNI100Result {
    #[serde(rename = "cniVersion", default)]
    pub cni_version: String,
    #[serde(rename = "interfaces", default, skip_serializing_if = "Vec::is_empty")]
    pub interfaces: Vec<CNI100Interface>,
    #[serde(rename = "ips", default, skip_serializing_if = "Vec::is_empty")]
    pub ips: Vec<CNI100IPAddress>,
    #[serde(rename = "routes", default, skip_serializing_if = "Vec::is_empty")]
    pub routes: Vec<Route>,
    #[serde(rename = "dns", default)]
    pub dns: DNS,
}
