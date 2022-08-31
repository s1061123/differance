// types
use crate::ipnet;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

/*
 * Note: in this directory, for each cni version, result type is defined and
 * each version's result, except latest version, have conversion function to
 * latest and vice versa. For example, types040 structure (for cniVersion
 * 0.3.0/0.3.1/0.4.0) has 'convert_to_latest()' and 'convert_from_latest()'.
 *
 * In usual cni plugin, once read previous CNI result and convert to latest,
 * then modify (e.g. add interface, ip address), at last return to the
 * further CNI pluign/runtime with the version defined in 'cniVersion' in CNI
 * config.
 */

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct DNS {
    #[serde(rename = "nameservers", default, skip_serializing_if = "Vec::is_empty")]
    pub nameservers: Vec<String>,
    #[serde(rename = "domain", default)]
    pub domain: String,
    #[serde(rename = "search", default, skip_serializing_if = "Vec::is_empty")]
    pub search: Vec<String>,
    #[serde(rename = "options", default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

#[allow(unused)] // XXX to be removed
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Route {
    #[serde(rename = "dst")]
    pub dst: ipnet::IPNet,
    #[serde(rename = "gw")]
    pub gw: IpAddr,
}
