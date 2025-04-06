use crate::MirrorError;
use futures::stream::TryStreamExt;
use rand::{
    distr::{Alphanumeric, SampleString},
    rng,
};
use rtnetlink::{new_connection, AddressHandle, Handle, LinkBridge, LinkUnspec, LinkVeth};
use std::net::Ipv4Addr;
use std::str::FromStr;

fn random_suffix(len: usize) -> String {
    Alphanumeric.sample_string(&mut rng(), len)
}

pub async fn get_bridge_idx(handle: &Handle, bridge_name: String) -> Result<u32, MirrorError> {
    // retrieve bridge index
    let bridge_idx = handle
        .link()
        .get()
        .match_name(bridge_name)
        .execute()
        .try_next()
        .await
        .unwrap()
        .ok_or_else(|| MirrorError::new("failed to get bridge index".to_string()))?
        .header
        .index;

    Ok(bridge_idx)
}

pub async fn create_bridge(name: String, bridge_ip: &str, subnet: u8) -> Result<u32, MirrorError> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    // Create a bridge
    handle
        .link()
        .add(LinkBridge::new(&name.clone()).build())
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "create bridge with name {} failed: {}",
                name.clone(),
                e
            ))
        })?;

    // Bring up the bridge
    let bridge_idx = handle
        .link()
        .get()
        .match_name(name)
        .execute()
        .try_next()
        .await
        .unwrap()
        .ok_or_else(|| MirrorError::new("failed to get bridge index".to_string()))?
        .header
        .index;

    // add ip address to bridge
    let bridge_addr = std::net::IpAddr::V4(Ipv4Addr::from_str(bridge_ip).unwrap());
    AddressHandle::new(handle.clone())
        .add(bridge_idx, bridge_addr, subnet)
        .execute()
        .await
        .map_err(|e| MirrorError::new(format!("add IP address to bridge failed: {}", e)))?;

    // set bridge up
    handle
        .link()
        .set(LinkUnspec::new_with_index(bridge_idx).up().build())
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "set bridge with idx {} to up failed: {}",
                bridge_idx, e
            ))
        })?;

    Ok(bridge_idx)
}

pub async fn create_veth_pair(bridge_idx: u32) -> Result<(u32, u32), MirrorError> {
    let (connection, handle, _) = new_connection().unwrap();

    tokio::spawn(connection);

    // create veth interfaces
    let veth: String = format!("veth{}", random_suffix(4));
    let veth_peer: String = format!("{}_peer", veth.clone());

    handle
        .link()
        .add(LinkVeth::new(&veth, &veth_peer).build())
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "create veth pair {} and {} failed: {}",
                veth, veth_peer, e
            ))
        })?;

    let veth_idx = handle
        .link()
        .get()
        .match_name(veth.clone())
        .execute()
        .try_next()
        .await
        .unwrap()
        .ok_or_else(|| MirrorError::new("failed to get veth index".to_string()))?
        .header
        .index;

    let veth_peer_idx = handle
        .link()
        .get()
        .match_name(veth_peer.clone())
        .execute()
        .try_next()
        .await
        .unwrap()
        .ok_or_else(|| MirrorError::new("failed to get veth_peer index".to_string()))?
        .header
        .index;

    // set master veth up
    handle
        .link()
        .set(LinkUnspec::new_with_index(veth_idx).up().build())
        .execute()
        .await
        .unwrap();

    // set master veth to bridge
    handle
        .link()
        .set(
            LinkUnspec::new_with_index(veth_idx)
                .controller(bridge_idx)
                .build(),
        )
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "set veth with idx {} to bridge with idx {} failed: {}",
                veth_idx, bridge_idx, e
            ))
        })?;

    Ok((veth_idx, veth_peer_idx))
}

pub async fn join_veth_to_ns(veth_idx: u32, pid: u32) -> Result<(), MirrorError> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    // set veth to the process network namespace
    handle
        .link()
        .set(
            LinkUnspec::new_with_index(veth_idx)
                .setns_by_pid(pid)
                .build(),
        )
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "set veth with idx {} to process with pid {} failed: {}",
                veth_idx, pid, e
            ))
        })?;

    Ok(())
}

pub async fn setup_veth_peer(veth_idx: u32, ns_ip: &String, subnet: u8) -> Result<(), MirrorError> {
    let (connection, handle, _) = new_connection().unwrap();
    tokio::spawn(connection);

    //info!("setup veth peer with ip: {}/{}", ns_ip, subnet);

    // set veth peer address
    let veth_2_addr = std::net::IpAddr::V4(Ipv4Addr::from_str(ns_ip).unwrap());
    AddressHandle::new(handle.clone())
        .add(veth_idx, veth_2_addr, subnet)
        .execute()
        .await
        .map_err(|e| MirrorError::new(format!("add IP address to veth peer failed: {}", e)))?;

    handle
        .link()
        .set(LinkUnspec::new_with_index(veth_idx).up().build())
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "set veth with idx {} to up failed: {}",
                veth_idx, e
            ))
        })?;

    // set lo interface to up
    let lo_idx = handle
        .link()
        .get()
        .match_name("lo".to_string())
        .execute()
        .try_next()
        .await
        .unwrap()
        .ok_or_else(|| MirrorError::new("failed to get lo index".to_string()))?
        .header
        .index;

    handle
        .link()
        .set(LinkUnspec::new_with_index(lo_idx).up().build())
        .execute()
        .await
        .map_err(|e| {
            MirrorError::new(format!(
                "set lo interface with idx {} to up failed: {}",
                lo_idx, e
            ))
        })?;

    Ok(())
}
