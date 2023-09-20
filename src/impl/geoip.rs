use crate::r#impl::config::{Endpoint, Endpoints};
use async_compat::Compat;
use geoutils::{Distance, Location};
use itertools::Itertools;
use log::debug;
use maxminddb::geoip2;
use rocket::futures::executor;
use std::net::{IpAddr, Ipv4Addr};

pub fn find_best_location<'a, S: AsRef<[u8]>>(
    endpoints: &'a Endpoints,
    geoipdb: &maxminddb::Reader<S>,
    ip_addr: IpAddr,
) -> &'a Endpoint {
    match geoipdb.lookup::<geoip2::City>(ip_addr) {
        Ok(city) => match city.location {
            Some(geoip2::city::Location {
                latitude: Some(lat),
                longitude: Some(lon),
                ..
            }) => endpoints
                .get_all()
                .iter()
                .sorted_by_key(|ep| {
                    ep.location
                        .distance_to(&Location::new(lat, lon))
                        .unwrap_or_else(|_| Distance::from_meters(9999999))
                        .meters()
                        .abs() as i64
                })
                .next()
                .unwrap(),
            _ => {
                debug!(
                    "Failed to find location for IP address: {:?} has no location information.",
                    city
                );
                &endpoints.get_all()[0]
            }
        },
        Err(err) => {
            debug!(
                "Failed to find location for IP address: {}: {}",
                ip_addr, err
            );
            &endpoints.get_all()[0]
        }
    }
}

pub fn self_server_ip() -> IpAddr {
    let ip_addr = executor::block_on(Compat::new(public_ip::addr()));
    if let Some(ip_addr) = ip_addr {
        ip_addr
    } else {
        IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))
    }
}
