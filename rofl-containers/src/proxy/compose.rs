use std::{borrow::Cow, collections::BTreeMap, str::FromStr, time::Duration};

use anyhow::{anyhow, Context, Result};
use yaml_rust2::{yaml, Yaml, YamlLoader};

use rofl_proxy::http::Timeouts;

/// A parsed compose file.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ParsedCompose {
    /// Extracted port mappings.
    pub port_mappings: Vec<PortMapping>,
}

impl ParsedCompose {
    /// Parse compose file data.
    pub fn parse(data: &str) -> Result<Self> {
        let mut result = ParsedCompose {
            port_mappings: Vec::new(),
        };

        let compose = YamlLoader::load_from_str(data).context("failed to parse compose file")?;
        let compose = compose.first().ok_or(anyhow!("empty compose file"))?;
        let compose = Self::resolve_merge_keys(compose);
        let services = compose["services"]
            .as_hash()
            .ok_or(anyhow!("bad services definition"))?;
        for (service_name, service) in services {
            let service_name = match service_name.as_str() {
                Some(service_name) => service_name,
                None => continue,
            };
            let ports = match service["ports"].as_vec() {
                Some(ports) => ports,
                None => continue,
            };
            let annotations = Self::parse_annotations(service);

            for port in ports {
                let port: Option<ParsedPort> = match port {
                    Yaml::String(port) => ParsedPort::parse_short(port),
                    Yaml::Hash(_) => ParsedPort::parse_long(port),
                    _ => continue,
                };
                let port = match port {
                    Some(port) => port,
                    None => continue,
                };

                let mode = annotations
                    .get(&format!("net.oasis.proxy.ports.{}.mode", port.host_port).as_str())
                    .and_then(|mode| mode.parse().ok())
                    .unwrap_or_default();

                let custom_domain = annotations
                    .get(
                        &format!("net.oasis.proxy.ports.{}.custom_domain", port.host_port).as_str(),
                    )
                    .map(|domain| domain.to_string());

                let proxy_protocol = annotations
                    .get(
                        &format!("net.oasis.proxy.ports.{}.proxy_protocol", port.host_port)
                            .as_str(),
                    )
                    .map(|version| version.to_string());

                let timeout = |setting: &str| {
                    annotations
                        .get(
                            format!(
                                "net.oasis.proxy.ports.{}.timeout_{}",
                                port.host_port, setting
                            )
                            .as_str(),
                        )
                        .and_then(|value| value.parse::<u64>().ok())
                        .filter(|&secs| secs > 0)
                        .map(Duration::from_secs)
                };
                let timeouts = Timeouts {
                    connect: timeout("connect"),
                    connection: timeout("connection"),
                    rw: timeout("rw"),
                };

                result.port_mappings.push(PortMapping {
                    service: service_name.to_string(),
                    custom_domain,
                    proxy_protocol,
                    timeouts,
                    port,
                    mode,
                });
            }
        }

        Ok(result)
    }

    /// Resolve YAML merge keys (`<<`) as the YAML parser does not handle them.
    fn resolve_merge_keys(yaml: &Yaml) -> Yaml {
        match yaml {
            Yaml::Array(list) => Yaml::Array(list.iter().map(Self::resolve_merge_keys).collect()),
            Yaml::Hash(map) => {
                let mut result = yaml::Hash::new();
                let mut merged = Vec::new();
                for (key, value) in map {
                    match (key.as_str(), Self::resolve_merge_keys(value)) {
                        (Some("<<"), Yaml::Hash(map)) => merged.push(map),
                        (Some("<<"), Yaml::Array(list)) => {
                            merged.extend(list.into_iter().filter_map(Yaml::into_hash))
                        }
                        (_, value) => {
                            result.insert(key.clone(), value);
                        }
                    }
                }
                // Explicit keys take precedence, followed by keys from earlier merged mappings.
                for (key, value) in merged.into_iter().flatten() {
                    result.entry(key).or_insert(value);
                }
                Yaml::Hash(result)
            }
            _ => yaml.clone(),
        }
    }

    fn parse_annotations(service: &Yaml) -> BTreeMap<&str, Cow<'_, str>> {
        match &service["annotations"] {
            Yaml::Array(list) => list
                .iter()
                .filter_map(|v| v.as_str()?.split_once('='))
                .map(|(k, v)| (k, Cow::Borrowed(v)))
                .collect(),
            Yaml::Hash(map) => map
                .into_iter()
                .filter_map(|(k, v)| {
                    // Compose accepts numeric annotation values (e.g. timeouts) too.
                    let v = match v {
                        Yaml::String(v) => Cow::Borrowed(v.as_str()),
                        Yaml::Integer(v) => Cow::Owned(v.to_string()),
                        _ => return None,
                    };
                    Some((k.as_str()?, v))
                })
                .collect(),
            _ => BTreeMap::new(),
        }
    }
}

/// Mode for the proxy behavior on a given port.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub enum PortMappingMode {
    Passthrough,
    #[default]
    TerminateTls,
    Ignore,
}

impl FromStr for PortMappingMode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "passthrough" => Ok(Self::Passthrough),
            "terminate-tls" => Ok(Self::TerminateTls),
            "ignore" => Ok(Self::Ignore),
            _ => Err(anyhow!("unsupported mode")),
        }
    }
}

/// A service port mapping.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct PortMapping {
    /// Service name.
    pub service: String,
    /// Optional custom domain.
    pub custom_domain: Option<String>,
    /// Optional PROXY protocol version that the service expects on this port.
    pub proxy_protocol: Option<String>,
    /// Proxy timeout overrides for this port.
    pub timeouts: Timeouts,
    /// Port descriptor.
    pub port: ParsedPort,
    /// Mode for the proxy behavior on this port.
    pub mode: PortMappingMode,
}

/// A parsed port.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub struct ParsedPort {
    /// Protocol name.
    pub protocol: String,
    /// Host interface address.
    pub host_address: String,
    /// Host port.
    pub host_port: u16,
    /// Container port.
    pub container_port: u16,
}

impl ParsedPort {
    /// Parse short port mapping definition.
    fn parse_short(mapping: &str) -> Option<Self> {
        // Parse optional protocol (defaults to "tcp").
        let atoms: Vec<&str> = mapping.split('/').collect();
        let protocol = match atoms.len() {
            1 => "tcp",
            2 => atoms[1],
            _ => return None, // Invalid port format.
        };

        // NOTE: Given that parsing IPv6 addresses in the short notation is a bit awkward we simply
        //       do not support it and the user needs to use the long port mapping.

        let remainder = atoms[0];
        let atoms: Vec<&str> = remainder.split(':').collect();
        let (host_address, host_port, container_port) = match atoms.len() {
            1 => {
                // Only container port is defined. This binds to a random port on the host and so is
                // currently not supported.
                return None;
            }
            2 => {
                // Explicit host port bound to all interfaces on the host.
                let host_address = "127.0.0.1".to_string();
                let host_port = atoms[0].parse().ok()?;
                let container_port = atoms[1].parse().ok()?;
                (host_address, host_port, container_port)
            }
            3 => {
                // Explicit host address and port.
                let host_address = atoms[0].to_string();
                let host_port = atoms[1].parse().ok()?;
                let container_port = atoms[2].parse().ok()?;
                (host_address, host_port, container_port)
            }
            _ => return None, // Invalid or unsupported port format.
        };

        // Ensure ports are valid.
        if host_port == 0 || container_port == 0 {
            return None;
        }

        Some(ParsedPort {
            protocol: protocol.to_string(),
            host_address,
            host_port,
            container_port,
        })
    }

    /// Parse long port mapping definition.
    fn parse_long(mapping: &Yaml) -> Option<Self> {
        let protocol = mapping["protocol"].as_str().unwrap_or("tcp").to_string();
        let host_address = mapping["host_ip"]
            .as_str()
            .unwrap_or("127.0.0.1")
            .to_string();
        let host_port = match mapping["published"] {
            Yaml::String(ref str) => str.parse().ok()?,
            Yaml::Integer(int) => int.try_into().ok()?,
            _ => return None,
        };
        let container_port = mapping["target"].as_i64()?.try_into().ok()?;

        // Ensure ports are valid.
        if host_port == 0 || container_port == 0 {
            return None;
        }

        Some(ParsedPort {
            protocol: protocol.to_string(),
            host_address,
            host_port,
            container_port,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_port_mapping_mode() {
        let tcs = vec![
            ("invalid", None),
            ("passthrough", Some(PortMappingMode::Passthrough)),
            ("terminate-tls", Some(PortMappingMode::TerminateTls)),
            ("ignore", Some(PortMappingMode::Ignore)),
        ];
        for tc in tcs {
            let mode = tc.0.parse().ok();
            assert_eq!(tc.1, mode);
        }
    }

    #[test]
    fn test_parse_port_short() {
        let tcs = vec![
            ("foo bar goo", None),
            ("::1:1234:1234", None),
            ("123456789", None),
            ("123456789:123456789", None),
            ("1234", None),
            ("0:1234", None),
            ("1234:0", None),
            (
                "1234:1234",
                Some(ParsedPort {
                    protocol: "tcp".to_string(),
                    host_address: "127.0.0.1".to_string(),
                    host_port: 1234,
                    container_port: 1234,
                }),
            ),
            (
                "1234:5678",
                Some(ParsedPort {
                    protocol: "tcp".to_string(),
                    host_address: "127.0.0.1".to_string(),
                    host_port: 1234,
                    container_port: 5678,
                }),
            ),
            (
                "1234:5678/udp",
                Some(ParsedPort {
                    protocol: "udp".to_string(),
                    host_address: "127.0.0.1".to_string(),
                    host_port: 1234,
                    container_port: 5678,
                }),
            ),
            (
                "127.0.0.2:1234:5678/udp",
                Some(ParsedPort {
                    protocol: "udp".to_string(),
                    host_address: "127.0.0.2".to_string(),
                    host_port: 1234,
                    container_port: 5678,
                }),
            ),
        ];

        for tc in tcs {
            let port = ParsedPort::parse_short(tc.0);
            assert_eq!(port, tc.1);
        }
    }

    #[test]
    fn test_parse_compose_file_1() {
        let data = r#"
services:
    frontend:
        image: docker.io/hashicorp/http-echo:latest@sha256:fcb75f691c8b0414d670ae570240cbf95502cc18a9ba57e982ecac589760a186
        platform: linux/amd64
        environment:
            ECHO_TEXT: "hello rofl world"
        ports:
            - "5678:5678"
            - target: 1234
              published: "8888"
              host_ip: "127.0.0.2"
            - target: 6666
              published: 6666
"#;
        let parsed = ParsedCompose::parse(data).unwrap();
        assert_eq!(parsed.port_mappings.len(), 3);

        let mapping = &parsed.port_mappings[0];
        assert_eq!(&mapping.service, "frontend");
        assert_eq!(&mapping.port.protocol, "tcp");
        assert_eq!(&mapping.port.host_address, "127.0.0.1");
        assert_eq!(mapping.port.host_port, 5678);
        assert_eq!(mapping.port.container_port, 5678);
        assert_eq!(mapping.mode, PortMappingMode::TerminateTls);

        let mapping = &parsed.port_mappings[1];
        assert_eq!(&mapping.service, "frontend");
        assert_eq!(&mapping.port.protocol, "tcp");
        assert_eq!(&mapping.port.host_address, "127.0.0.2");
        assert_eq!(mapping.port.host_port, 8888);
        assert_eq!(mapping.port.container_port, 1234);
        assert_eq!(mapping.mode, PortMappingMode::TerminateTls);

        let mapping = &parsed.port_mappings[2];
        assert_eq!(&mapping.service, "frontend");
        assert_eq!(&mapping.port.protocol, "tcp");
        assert_eq!(&mapping.port.host_address, "127.0.0.1");
        assert_eq!(mapping.port.host_port, 6666);
        assert_eq!(mapping.port.container_port, 6666);
        assert_eq!(mapping.mode, PortMappingMode::TerminateTls);
    }

    #[test]
    fn test_parse_compose_file_2() {
        let data = r#"
services:
    frontend:
        image: docker.io/hashicorp/http-echo:latest@sha256:fcb75f691c8b0414d670ae570240cbf95502cc18a9ba57e982ecac589760a186
        platform: linux/amd64
        environment:
            ECHO_TEXT: "hello rofl world"
        annotations:
            net.oasis.proxy.ports.5678.mode: passthrough
            net.oasis.proxy.ports.5678.proxy_protocol: v2
            net.oasis.proxy.ports.8888.mode: ignore
            net.oasis.proxy.ports.8888.custom_domain: example.com
        ports:
            - "5678:5678"
            - target: 1234
              published: "8888"
              host_ip: "127.0.0.2"
"#;
        let parsed = ParsedCompose::parse(data).unwrap();
        assert_eq!(parsed.port_mappings.len(), 2);

        let mapping = &parsed.port_mappings[0];
        assert_eq!(&mapping.service, "frontend");
        assert_eq!(&mapping.port.protocol, "tcp");
        assert_eq!(&mapping.port.host_address, "127.0.0.1");
        assert_eq!(mapping.port.host_port, 5678);
        assert_eq!(mapping.port.container_port, 5678);
        assert_eq!(mapping.mode, PortMappingMode::Passthrough);
        assert_eq!(mapping.custom_domain, None);
        assert_eq!(mapping.proxy_protocol, Some("v2".to_string()));

        let mapping = &parsed.port_mappings[1];
        assert_eq!(&mapping.service, "frontend");
        assert_eq!(&mapping.port.protocol, "tcp");
        assert_eq!(&mapping.port.host_address, "127.0.0.2");
        assert_eq!(mapping.port.host_port, 8888);
        assert_eq!(mapping.port.container_port, 1234);
        assert_eq!(mapping.mode, PortMappingMode::Ignore);
        assert_eq!(mapping.custom_domain, Some("example.com".to_string()));
        assert_eq!(mapping.proxy_protocol, None);
    }

    #[test]
    fn test_parse_compose_file_timeouts() {
        let data = r#"
services:
    frontend:
        image: docker.io/hashicorp/http-echo:latest
        annotations:
            net.oasis.proxy.ports.5678.timeout_rw: "300"
            net.oasis.proxy.ports.5678.timeout_connection: 3600
            net.oasis.proxy.ports.5678.timeout_connect: 5
            net.oasis.proxy.ports.8888.timeout_rw: 0
            net.oasis.proxy.ports.8888.timeout_connection: invalid
        ports:
            - "5678:5678"
            - "8888:8888"
            - "9999:9999"
"#;
        let parsed = ParsedCompose::parse(data).unwrap();
        assert_eq!(parsed.port_mappings.len(), 3);

        // Both string and integer values should be accepted.
        let mapping = &parsed.port_mappings[0];
        assert_eq!(mapping.port.host_port, 5678);
        assert_eq!(
            mapping.timeouts,
            Timeouts {
                connect: Some(Duration::from_secs(5)),
                connection: Some(Duration::from_secs(3600)),
                rw: Some(Duration::from_secs(300)),
            }
        );

        // Invalid values should be ignored.
        let mapping = &parsed.port_mappings[1];
        assert_eq!(mapping.port.host_port, 8888);
        assert_eq!(mapping.timeouts, Timeouts::default());

        // Ports without annotations should use the defaults.
        let mapping = &parsed.port_mappings[2];
        assert_eq!(mapping.port.host_port, 9999);
        assert_eq!(mapping.timeouts, Timeouts::default());
    }

    #[test]
    fn test_parse_compose_file_merge_keys() {
        let data = r#"
x-annotations: &annotations
    net.oasis.proxy.ports.5678.mode: passthrough
    net.oasis.proxy.ports.5678.proxy_protocol: v2
x-service: &service
    image: docker.io/hashicorp/http-echo:latest
    ports:
        - "5678:5678"
services:
    frontend:
        <<: *service
        annotations:
            <<: *annotations
            net.oasis.proxy.ports.5678.mode: terminate-tls
"#;
        let parsed = ParsedCompose::parse(data).unwrap();
        assert_eq!(parsed.port_mappings.len(), 1);

        let mapping = &parsed.port_mappings[0];
        assert_eq!(mapping.port.host_port, 5678);
        assert_eq!(mapping.mode, PortMappingMode::TerminateTls);
        assert_eq!(mapping.proxy_protocol, Some("v2".to_string()));
    }
}
