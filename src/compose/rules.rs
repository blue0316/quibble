
use log::debug;

use crate::security::{Rule, Alert, RuleID, Severity, AlertLocation};


pub struct ComposeVersion;

impl Rule for ComposeVersion {
   fn check(alerts: &mut Vec<crate::security::Alert>, compose_file: &super::ComposeFile) {
        debug!("Compose Version rule enabled...");
        // https://docs.docker.com/compose/compose-file/compose-versioning/
        match compose_file.compose.version.as_str() {
            "1" => {
                alerts.push(Alert {
                    id: RuleID::None,
                    details: String::from("Compose v1"),
                    severity: Severity::Medium,
                    path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                })
            },
            "2" | "2.0" | "2.1" | "2.2" | "2.3" | "2.4" => {
                alerts.push(Alert {
                    id: RuleID::None,
                    details: String::from("Compose v2 used"),
                    severity: Severity::Low,
                    path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                })
            },
            "3" | "3.0" | "3.1" | "3.2" | "3.3" | "3.4" | "3.5" => {
                alerts.push(Alert {
                    id: crate::security::RuleID::None,
                    details: String::from("Using old Compose v3 spec, consider upgrading"),
                    severity: Severity::Low,
                    path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                })
            },
            _ => {
                debug!("Unknown or secure version of Docker Compose")
            }
        }
    } 
}

/// Container Images
pub struct ContainerImages;

const CONTAINER_REGISTRIES: &[&str; 2] = &[ 
    "docker.io",
    "ghcr.io"
];

impl Rule for ContainerImages {
    fn check(alerts: &mut Vec<Alert>, compose_file: &super::ComposeFile) {
        for service in compose_file.compose.services.values() {
            // TODO: Manually building project

            // Pulling remote image
            if let Some(image) = &service.image {
                // Format strings 
                if image.contains("${") {
                    alerts.push(Alert {
                        details: format!("Container Image using Environment Variable: {}", image),
                        path: AlertLocation { path: compose_file.path.clone(), ..Default::default()},
                        ..Default::default()
                    })
                } 
                else {
                    let container = service.parse_image().unwrap();

                    alerts.push(Alert {
                        details: format!("Container Image: {}", container),
                        path: AlertLocation { path: compose_file.path.clone(), ..Default::default()},
                        ..Default::default()
                    });

                    // Rule: Pinned to latest rolling container image
                    // - The main reason behind this is if you are using watchtower or other
                    // service to update containers it might cause issues
                    if container.tag.as_str() == "latest" {
                        alerts.push(Alert {
                            details: String::from("Container using latest / rolling release tag"),
                            severity: Severity::Low,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()},
                            ..Default::default()
                        });
                    }

                    // Rule: Unknown registry
                    // - Which registries could be in here?
                    // - How does a user update / add approved registries?
                    if ! CONTAINER_REGISTRIES.contains(&container.instance.as_str()) {
                        alerts.push(Alert {
                            details: format!("Container from unknown registry: {}", &container.instance),
                            severity: Severity::Low,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()},
                            ..Default::default()
                        });
                    }
                }
            }
        }
    }
}


pub struct DockerSocket;

impl Rule for DockerSocket {
    fn check(alerts: &mut Vec<Alert>, compose_file: &super::ComposeFile) {
        debug!("Docker Socker Rule enabled...");

        for service in compose_file.compose.services.values() {
            if let Some(volumes) = &service.volumes {
                let result = volumes.iter()
                    .find(|&s| s.starts_with("/var/run/docker.sock"));

                if result.is_some() {
                    alerts.push(Alert {
                        id: RuleID::Owasp("D04".to_string()),
                        details: String::from("Docker Socket being passed into container"),
                        severity: Severity::High,
                        path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                    })
                }
            }

        }
    }
    
}


pub struct SecurityOpts;

impl Rule for SecurityOpts {
    fn check(alerts: &mut Vec<Alert>, compose_file: &super::ComposeFile) {
        for service in compose_file.compose.services.values() {
            if let Some(secopts) = &service.security_opt {
                for secopt in secopts {
                    if secopt.starts_with("no-new-privileges") && secopt.ends_with("false") {
                        alerts.push(Alert {
                            id: RuleID::Owasp("D04".to_string()),
                            details: String::from("Security Opts `no-new-privileges` set to `false`"),
                            severity: Severity::High,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }
                }
            }
            else {
                alerts.push(Alert {
                    id: RuleID::Owasp("D04".to_string()),
                    details: String::from("Security Opts `no-new-privileges` not set"),
                    severity: Severity::High,
                    path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                })
            }
        }

    }
}

pub struct KernalParameters;

impl Rule for KernalParameters {
    fn check(alerts: &mut Vec<Alert>, compose_file: &super::ComposeFile) {
        for service in compose_file.compose.services.values() {
            if let Some(syscalls) = &service.sysctls {
                alerts.push(Alert {
                    details: String::from("Enabling extra syscalls"),
                    ..Default::default()
                });

                for syscall in syscalls {
                    if syscall.starts_with("net.ipv4.conf.all") {
                        alerts.push(Alert {
                            id: RuleID::None,
                            details: format!("IPv4 Kernal Parameters modified: {}", syscall),
                            severity: Severity::Information,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }
                }
            }

            if let Some(capabilities) = &service.cap_add {
                alerts.push(Alert {
                    details: String::from("Using extra Kernal Parameters"),
                    ..Default::default()
                });

                for cap in capabilities {
                    // https://man7.org/linux/man-pages/man7/capabilities.7.html
                    // https://cloud.redhat.com/blog/increasing-security-of-istio-deployments-by-removing-the-need-for-privileged-containers
                    if cap.contains("NET_ADMIN") {
                        alerts.push(Alert {
                            id: RuleID::None,
                            details: String::from("Container with high networking privileages"),
                            severity: Severity::Medium,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }

                    if cap.contains("ALL") {
                        alerts.push(Alert {
                            details: String::from("All capabilities are enabled"),
                            severity: Severity::High,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()},
                            ..Default::default()
                        })
                    }
                }
            }

            if let Some(capabilities) = &service.cap_add {
                alerts.push(Alert {
                    details: String::from("Using extra Kernal Parameters"),
                    ..Default::default()
                });

                for cap in capabilities {
                    // https://man7.org/linux/man-pages/man7/capabilities.7.html
                    // https://cloud.redhat.com/blog/increasing-security-of-istio-deployments-by-removing-the-need-for-privileged-containers
                    if cap.contains("NET_ADMIN") {
                        alerts.push(Alert {
                            id: RuleID::None,
                            details: String::from("Container with high networking privileages"),
                            severity: Severity::Medium,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }
                    if cap.contains("SYS_ADMIN") {
                        alerts.push(Alert {
                            id: RuleID::None,
                            details: String::from("Container with high system privileages"),
                            severity: Severity::Medium,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }
                }
            }
        }
    }
}


pub struct EnvironmentVariables;

impl Rule for EnvironmentVariables {
    fn check(alerts: &mut Vec<Alert>, compose_file: &super::ComposeFile) {
        for service in compose_file.compose.services.values() {
            if let Some(envvars) = &service.environment {
                for envvar in envvars {
                    if envvar.contains("DEBUG") {
                        alerts.push(Alert {
                            id: RuleID::Cwe(String::from("1244")),
                            details: String::from("Debugging enabled in the container"),
                            severity: Severity::Medium,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }
                    // TODO: better way of detecting this
                    if envvar.contains("PASSWORD") || envvar.contains("KEY") {
                        alerts.push(Alert {
                            id: RuleID::Cwe(String::from("215")),
                            details: format!("Possible Hardcoded password: {}", envvar),
                            severity: Severity::High,
                            path: AlertLocation { path: compose_file.path.clone(), ..Default::default()}
                        })
                    }
                }
            }
        }
    }
}



pub fn checks(compose_file: &super::ComposeFile) -> Vec<Alert> {
    let mut alerts: Vec<Alert> = Vec::new();

    ComposeVersion::check(&mut alerts, compose_file);
    ContainerImages::check(&mut alerts, compose_file);
    DockerSocket::check(&mut alerts, compose_file);
    SecurityOpts::check(&mut alerts, compose_file);
    KernalParameters::check(&mut alerts, compose_file);
    
    alerts
}

