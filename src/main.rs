use std::fs::File;
use std::io::Write;

extern crate redis;
use libcni::skel::NetConf as CNINetConf;
use libcni::skel::*;
use libcni::ipnet::IPNet;
use libcni::types::types_100::*;
use libcni::types::types_common::DNS;

use anyhow::Result; // bail may be used.
use clap::{App, Arg, ArgAction};
use kube::{
    //api::{Api}, //, DeleteParams, ListParams, PostParams, ResourceExt},
    config::{KubeConfigOptions, Kubeconfig},
    //core::crd::CustomResourceExt,
    Client,
    Config, //CustomResource,
};
use redis::Client as RedisClient;
use serde::Deserialize;

//use crate::kube_crd::NetworkIP;
mod kube_crd;
mod redisdb;

#[derive(Deserialize, Debug)]
struct IPAMConfig {
    #[allow(dead_code)]
    #[serde(rename = "name", default)]
    name: String,
    #[allow(dead_code)]
    #[serde(rename = "type", default)]
    r#type: String,
    #[serde(rename = "redis_ip")]
    redis_ip: String,
    #[serde(rename = "kubeconfig")]
    kubeconfig: String,
    #[serde(rename = "network")]
    network: String,
    #[serde(rename = "debug_file", default)]
    debug_file: String,
}

#[derive(Deserialize, Debug)]
struct NetConf {
    #[serde(flatten)]
    netconf: CNINetConf,

    #[serde(rename = "ipam")]
    ipam: IPAMConfig,
}

#[tokio::main]
async fn main() -> Result<()> {
    let create_arg: Arg = Arg::new("create")
        .help("create CRDs for this app")
        .long("create")
        .takes_value(false)
        .required(false)
        .action(ArgAction::SetTrue);
    let kubeconfig_arg: Arg = Arg::new("kubeconfig")
        .help("kubeconfig path")
        .long("kubeconfig")
        .takes_value(true)
        .required(false);

    let app: App = App::new("differance-cni")
        .author("Tomofumi Hayashi")
        .version("0.1.0")
        .about("IPAM CNI plugin with redis backend")
        .arg(create_arg)
        .arg(kubeconfig_arg);

    // check '--create' flag for CRDs initialization
    let matches = app.try_get_matches()?;
    if *matches.get_one::<bool>("create").unwrap() {
        match matches.get_one::<String>("kubeconfig") {
            Some(s) => {
                println!("create CRDs");
                kube_crd::create_crd_kubeconfig(s).await?;
            }
            None => return Err(anyhow::anyhow!("no kubeconfig found")),
        }
        // '--create' flag case, just quit before CNI plugin code
        return Ok(());
    };

    let (command, cmd_args) = match get_cmdargs() {
        Ok(v) => v,
        Err(e) => {
            println!("Error: {}", e);
            return Err(e.into());
        }
    };

    let netconf: NetConf = match serde_json::from_str(cmd_args.stdin_data.as_str()) {
        Ok(v) => v,
        Err(err) => {
            println!("failed to : {}", err);
            return Err(err.into());
        }
    };
    let mut file = File::options().create(true).append(true).open(netconf.ipam.debug_file)?;

    // read kubeconfig
    let config = Config::from_custom_kubeconfig(
        Kubeconfig::read_from(&netconf.ipam.kubeconfig)?,
        &KubeConfigOptions::default(),
    )
    .await?;
    let client = Client::try_from(config)?;

    // if no target version CRD, then show error message!
    if !(kube_crd::check_crd(&client).await) {
        return Err(anyhow::anyhow!("no CRD {} found", kube_crd::CRD_NAME));
    };

    // read crds
    let networkip = kube_crd::get_crd(&client, netconf.ipam.network.as_str()).await?;
    //eprintln!("testoutput!: {:?}", networkip);

    match command.as_str() {
        "ADD" => {
            let redis_client = RedisClient::open(netconf.ipam.redis_ip)?;
            let mut con = redis_client.get_connection()?;
            // checck redis DB
            let key_exists = match redisdb::check_network_bitmap(&mut con, &networkip) {
                Ok(v) => v,
                Err(err) => return Err(err.into()),
            };
            // create redis DB if not exist
            match key_exists {
                0 => redisdb::create_network_bitmap(&mut con, &networkip)?,
                n => {
                    if networkip.spec.ip_allocations.len() != n.unsigned_abs() {
                        return Err(anyhow::anyhow!("database mismatch happen"));
                    }
                }
            };
            let prev_result = netconf.netconf.get_current_result().unwrap();
            let result = CNI100Result {
                cni_version: prev_result.cni_version,
                interfaces: prev_result.interfaces,
                ips: networkip.spec.ip_allocations.iter().filter_map(|alloc| {
                    let subnet: IPNet = alloc.subnet.parse().unwrap();
                    let address = IPNet{
                        ip: redisdb::get_first_available_ip(&mut con, &networkip, &alloc).unwrap(),
                        netmask_len: subnet.netmask_len,
                    };
                    let _ = redisdb::add_pod_information(
                        &mut con, &networkip, &alloc, 
                        &address.ip,
                        format!("{}/{} {}", 
                                match cmd_args.args.get(&"K8S_POD_NAMESPACE".to_string()) {
                                    Some(v) => v,
                                    None => "UnknownNamespace",
                                },
                                match cmd_args.args.get(&"K8S_POD_NAME".to_string()) {
                                    Some(v) => v,
                                    None => "UnknownPodName",
                                },
                                match cmd_args.args.get(&"K8S_POD_INFRA_CONTAINER_ID".to_string()) {
                                    Some(v) => v,
                                    None => "UnknownContainerID",
                                }));
                    Some(CNI100IPAddress {
                        interface: None,
                        address: address,
                        gateway: alloc.gateway,
                    })
                }).collect::<Vec<CNI100IPAddress>>(),
                routes: networkip.spec.ip_allocations
                    .iter().map(|alloc| alloc.get_cni_route()).flatten().collect(),
                    dns: DNS{
                        nameservers: vec![],
                        domain: "".to_string(),
                        search: vec![],
                        options: vec![],
                    },
            };
            // K8S_POD_NAME, K8S_POD_NAMESPACE, K8S_POD_INFRA_CONTAINER_ID, K8S_POD_UID
            println!("{}", netconf.netconf.get_result_output(&result).unwrap());
        },
        "CHECK" => {
            // XXX: implement check!
            let result = netconf.netconf.get_current_result().unwrap();
            println!("{}", netconf.netconf.get_result_output(&result).unwrap());
        },
        "DEL" => {
            let result = netconf.netconf.get_current_result().unwrap();
            let redis_client = RedisClient::open(netconf.ipam.redis_ip)?;
            let mut con = redis_client.get_connection()?;
            let _ = match redisdb::check_network_bitmap(&mut con, &networkip) {
                Ok(v) => v,
                Err(err) => return Err(err.into()),
            };
            for ip in result.ips.iter() {
                let network_ip = ip.address.get_network_ip();
                match networkip.spec.ip_allocations.iter().find(|x| x.get_network_ip() == network_ip) {
                    Some(alloc) => {
                        let _ = redisdb::del_pod_information(&mut con, &networkip, &alloc, &ip.address.ip);
                        let _ = redisdb::return_ip(&mut con, &networkip, &alloc, ip.address.ip);
                    },
                    None => {
                        eprintln!("not found!")
                    }
                }
            }
        },
        c => {
            println!("unknown command: {}", c);
        },
    };
    file.flush()?;
    Ok(())
}
