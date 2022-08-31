use async_std::task::block_on;
use std::net::IpAddr;
use std::time::Duration;

extern crate ipnet;
use libcni::ipnet::IPNet;
use ipnet::IpNet;

use libcni::types::types_common::Route as CNIRoute;

use anyhow::Result; // bail may be used.
use either::{Left, Right};
use k8s_openapi::apiextensions_apiserver::pkg::apis::apiextensions::v1::CustomResourceDefinition;
use kube::{
    api::{Api, DeleteParams, PostParams},
    config::{KubeConfigOptions, Kubeconfig},
    core::crd::CustomResourceExt,
    Client, Config, CustomResource,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;
use validator::Validate;

pub const CRD_NAME: &str = "networkips.xxxx.cni.cncf.io";
pub const CRD_VERSION: &str = "v1alpha1";

// NetworkIP CRD definition

#[derive(Deserialize, Serialize, Clone, Debug, Validate, JsonSchema)]
pub struct NetworkIPRange {
    /// start specifies the start IP address of the range
    start: IpAddr,
    /// end specifies the end IP address of the range
    #[serde(skip_serializing_if = "Option::is_none")]
    end: Option<IpAddr>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Validate, JsonSchema)]
pub struct Route {
    /// dst specifies target route destination
    #[serde(rename = "dst")]
    dst: String, //TODO: need to deserialize to libcni::IPNet directly
    /// gw specifies gateway address for the route
    #[serde(rename = "gw")]
    gw: IpAddr,
}

#[derive(Deserialize, Serialize, Clone, Debug, Default, Validate, JsonSchema)]
pub struct NetworkIPAllocations {
    /// name specifies identifier of the allocations
    pub name: String,
    /// subnet specifies ip(v4/v6) subnet
    pub subnet: String,
    /// gateway specifies gateway for the network
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<IpAddr>,
    /// range specifies network range
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<NetworkIPRange>,
    /// exclude specifies excluded ip address of the network allocations
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<IpAddr>,
    /// route specifies IP route information for the network
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub route: Vec<Route>,
}

impl NetworkIPAllocations {
    pub fn get_cni_route(&self) -> Vec<CNIRoute> {
        self.route.iter().map(|r| {
            let dst: IPNet = r.dst.parse().unwrap();
            CNIRoute{
                dst: dst,
                gw: r.gw,
            }
        }).collect()
    }
    
    pub fn get_network_ip(&self) -> IpAddr {
        let subnet: IPNet = self.subnet.parse().unwrap();
        subnet.get_network_ip()
    }
}

//#[kube(printcolumn = r#"{"name":"Namespace", "jsonPath": ".spec.metadata.namespace", "type": "string"}"#)]
//#[kube(status = "NetworkIPStatus")]
#[derive(CustomResource, Deserialize, Serialize, Clone, Debug, Validate, JsonSchema)]
#[kube(
    group = "xxxx.cni.cncf.io",
    version = "v1alpha1",
    kind = "NetworkIP",
    namespaced
)]
pub struct NetworkIPSpec {
    /// ipAllocations xxxx
    #[serde(rename = "ipAllocations")]
    pub ip_allocations: Vec<NetworkIPAllocations>,
}

pub fn get_ipallocation_names(networkip: &NetworkIP) -> Vec<String> {
    //<crd namespace>/<crd name>/<ipalloc name>/bitmap
    networkip
        .spec
        .ip_allocations
        .iter()
        .map(|allocations| {
            format!(
                "{}/{}/{}/bitmap",
                networkip.metadata.namespace.clone().unwrap(),
                networkip.metadata.name.clone().unwrap(),
                allocations.name.clone()
            )
        })
        .collect()
}

pub fn get_ipallocation_baseip(allocations: &NetworkIPAllocations) -> IpAddr {
    match &allocations.range {
        Some(r) => r.start,
        None => {
            let ip1: IpNet = allocations.subnet.parse().unwrap();
            ip1.hosts().next().unwrap()
        }
    }
}

pub async fn get_crd(client: &Client, networkip_namespacedname: &str) -> Result<NetworkIP> {
    let networkip_namevec: Vec<&str> = networkip_namespacedname.split('/').collect();
    let (networkip_namespace, networkip_name) = match networkip_namevec.len() {
        2 => (networkip_namevec[0], networkip_namevec[1]),
        1 => ("default", networkip_namevec[0]),
        _ => {
            return Err(anyhow::anyhow!(
                "cannot find networkip {}",
                networkip_namespacedname
            ))
        }
    };
    let network_ip_crd: Api<NetworkIP> = Api::namespaced(client.clone(), networkip_namespace);

    match network_ip_crd.get(networkip_name).await {
        Ok(c) => {
            //eprintln!("found CRD");
            Ok(c)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn check_crd(client: &Client) -> bool {
    // Manage CRDs first
    let crds: Api<CustomResourceDefinition> = Api::all(client.clone());

    // but ignore delete err if not exists
    match crds.get(CRD_NAME).await {
        Ok(c) => {
            let mut iter = c.spec.versions.iter();
            iter.any(|s| s.name == CRD_VERSION)
        }
        Err(_) => false,
    }
}

pub async fn delete_crd(client: &Client) -> Result<()> {
    // Manage CRDs first
    let crds: Api<CustomResourceDefinition> = Api::all(client.clone());

    // Delete any old versions of it first;
    let dp = DeleteParams::default();
    // but ignore delete err if not exists
    match crds.delete(CRD_NAME, &dp).await? {
        Left(_o) => {
            //info!("Deleting {}: ({:?})", o.name_any(), o.status.unwrap().conditions.unwrap().last());
            sleep(Duration::from_secs(1)).await;
        }
        Right(_status) => {
            //info!("Deleted {}: ({:?})", CRD_NAME, status);
        }
    }
    //info!("finish delete");
    Ok(())
}

// create CRDs from kubeconfig
pub async fn create_crd_kubeconfig(kubeconfig: &str) -> Result<()> {
    let config = Config::from_custom_kubeconfig(
        Kubeconfig::read_from(kubeconfig)?,
        &KubeConfigOptions::default(),
    )
    .await?;
    let client = Client::try_from(config)?;

    let _ = block_on(delete_crd(&client));
    //info!("end delete");
    // Manage CRDs first
    let crds: Api<CustomResourceDefinition> = Api::all(client.clone());

    // Create the CRD so we can create CRDs in kube
    let network_ip_crd = NetworkIP::crd();
    //info!("Creating NetworkIP CRD: {}", serde_json::to_string_pretty(&network_ip_crd)?);
    let pp = PostParams::default();
    match crds.create(&pp, &network_ip_crd).await {
        Ok(_o) => {
            //info!("Created {})", o.name_any());
            //debug!("Created CRD: {:?}", o.spec);
        }
        Err(kube::Error::Api(ae)) => {
            //info!("code: 409");
            assert_eq!(ae.code, 409)
        } // if you skipped delete, for instance any other case is propably bad
        Err(e) => return Err(e.into()),
    };
    Ok(())
}
