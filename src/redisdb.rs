use anyhow::Result; // bail may be used.
use std::net::IpAddr;
use std::net::IpAddr::V4;
use std::net::IpAddr::V6;

use ipnet::{IpAdd, IpSub};

use crate::kube_crd::NetworkIP;
use crate::kube_crd::*;
use redis::Commands;
use thiserror::Error;

#[derive(Debug, Error)]
enum AddressIndexError {
    #[error("failed to cast to u32: address range may be too big!: {0}")]
    FailedCast(std::num::TryFromIntError),
    #[error("address type mismatch: {0}")]
    FailedAnyhow(anyhow::Error),
}

impl From<std::num::TryFromIntError> for AddressIndexError {
    fn from(err: std::num::TryFromIntError) -> AddressIndexError {
        AddressIndexError::FailedCast(err)
    }
}

impl From<anyhow::Error> for AddressIndexError {
    fn from(err: anyhow::Error) -> AddressIndexError {
        AddressIndexError::FailedAnyhow(err)
    }
}

fn get_address_index(baseip: &IpAddr, ip: &IpAddr) -> Result<usize, AddressIndexError> {
    if let V4(v4addr) = ip {
        if let V4(v4base) = baseip {
            match usize::try_from(v4addr.saturating_sub(*v4base)) {
                Ok(v) => return Ok(v),
                Err(e) => return Err(AddressIndexError::FailedCast(e)),
            }
        }
    } else if let V6(v6addr) = ip {
        if let V6(v6base) = baseip {
            match usize::try_from(v6addr.saturating_sub(*v6base)) {
                Ok(v) => return Ok(v),
                Err(e) => return Err(AddressIndexError::FailedCast(e)),
            }
        }
    };
    Err(AddressIndexError::FailedAnyhow(anyhow::anyhow!("address type mismatch!")))
}

fn get_bitmap_key_name(networkip: &NetworkIP, alloc_name: &str) -> String {
    let networkip_namespace = networkip.metadata.namespace.clone().unwrap();
    let networkip_name = networkip.metadata.name.clone().unwrap();

    format!(
            "{}/{}/{}/bitmap",
            networkip_namespace,
            networkip_name,
            alloc_name.clone()
        )
}

fn get_baseip_key_name(networkip: &NetworkIP, alloc_name: &str) -> String {
    let networkip_namespace = networkip.metadata.namespace.clone().unwrap();
    let networkip_name = networkip.metadata.name.clone().unwrap();

    format!(
            "{}/{}/{}/baseip",
            networkip_namespace,
            networkip_name,
            alloc_name.clone()
        )
}

fn get_pod_info_key_name(networkip: &NetworkIP, alloc_name: &str, ip: &IpAddr) -> String {
    let networkip_namespace = networkip.metadata.namespace.clone().unwrap();
    let networkip_name = networkip.metadata.name.clone().unwrap();

    format!(
            "{}/{}/{}/{}",
            networkip_namespace,
            networkip_name,
            alloc_name.clone(),
            ip.to_string()
        )
}

pub fn return_ip(
    con: &mut redis::Connection,
    networkip: &NetworkIP,
    alloc: &NetworkIPAllocations,
    ip: IpAddr) -> redis::RedisResult<()> {
    let bitmap_key = get_bitmap_key_name(networkip, &alloc.name);
    let baseip_key = get_baseip_key_name(networkip, &alloc.name);

    let baseip_str:String = con.get(baseip_key)?;
    let baseip: IpAddr = baseip_str.parse().unwrap(); //XXX: may need to change error type, but we
                                                      //may assume that baseip should be valid.
    let index = get_address_index(&baseip, &ip).unwrap();
    redis::transaction(con, &[bitmap_key.clone()], |con, pipe| {
        pipe.setbit(bitmap_key.clone(), index, false).ignore().query(con)
    })
}

pub fn add_pod_information(
    con: &mut redis::Connection,
    networkip: &NetworkIP,
    alloc: &NetworkIPAllocations,
    ip: &IpAddr, pod_info: String) -> redis::RedisResult<()> {
    let pod_key = get_pod_info_key_name(networkip, &alloc.name, ip);
    con.set(pod_key, pod_info)
}

pub fn del_pod_information(
    con: &mut redis::Connection,
    networkip: &NetworkIP,
    alloc: &NetworkIPAllocations,
    ip: &IpAddr) -> redis::RedisResult<()> {
    let pod_key = get_pod_info_key_name(networkip, &alloc.name, ip);
    con.del(pod_key)
}

pub fn get_first_available_ip(
    con: &mut redis::Connection,
    networkip: &NetworkIP,
    alloc: &NetworkIPAllocations,
    ) -> redis::RedisResult<IpAddr> {
    let bitmap_key = get_bitmap_key_name(networkip, &alloc.name);
    let baseip_key = get_baseip_key_name(networkip, &alloc.name);

    let baseip_str:String = con.get(baseip_key)?;
    let baseip: IpAddr = baseip_str.parse().unwrap(); //XXX: may need to change error type, but we
                                                      //may assume that baseip should be valid.
    loop {
        redis::cmd("WATCH").arg(bitmap_key.clone()).query(con)?;

        let index_u: usize = redis::cmd("BITPOS").arg(bitmap_key.clone()).arg(0u8).query(con)?;
        let response: Option<(usize,)> = redis::pipe()
            .atomic()
            .cmd("SETBIT")
            .arg(bitmap_key.clone())
            .arg(index_u)
            .arg(1u8)
            .query(con)?;
        match response {
            None => {
                continue;
            },
            Some(_response) => {
                return Ok(match baseip {
                    V4(v4addr) => {
                        let index: u32 = usize::try_from(index_u).unwrap().try_into().unwrap();
                        V4(v4addr.saturating_add(index))
                    },
                    V6(v6addr) => {
                        let index: u128 = usize::try_from(index_u).unwrap().try_into().unwrap();
                        V6(v6addr.saturating_add(index))
                    }
                })
            }
        }
    };

}

pub fn create_network_bitmap(
    con: &mut redis::Connection,
    networkip: &NetworkIP,
) -> redis::RedisResult<()> {
    for alloc in networkip.spec.ip_allocations.clone() {
        let bitmap_key_name = get_bitmap_key_name(networkip, &alloc.name);
        let baseip_key_name = get_baseip_key_name(networkip, &alloc.name);
        let first_ip = get_ipallocation_baseip(&alloc);
        let _: () = con.set(baseip_key_name, first_ip.to_string())?;

        for exclude_ip in alloc.exclude.clone() {
            let idx = get_address_index(&first_ip, &exclude_ip).unwrap();
            //eprintln!("set bit {}", idx);
            let _: () = con.setbit(bitmap_key_name.clone(), idx, true)?;
        }
    };
    Ok(())
}

pub fn check_network_bitmap(
    con: &mut redis::Connection,
    networkip: &NetworkIP,
) -> redis::RedisResult<isize> {
    let keys = get_ipallocation_names(networkip);
    //eprintln!("XXX keys: {:?} ", keys);
    con.exists(keys)
    //con.exists(get_ipallocation_names(networkip))
}

