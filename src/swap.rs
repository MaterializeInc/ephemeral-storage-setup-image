use std::collections::BTreeMap;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{Map, Value};
use tracing::{debug, info};

use crate::detect::DiskDetectorTrait;
use crate::remove_taint::remove_taint;
use crate::{CloudProvider, Commander};

const AZURE_KUBELET_DROPIN_DIR: &str = "/host/etc/systemd/system/kubelet.service.d";
const KUBELET_HEALTHY_TIMEOUT: Duration = Duration::from_secs(300);

pub struct SwapController<D: DiskDetectorTrait> {
    pub cloud_provider: CloudProvider,
    pub commander: Commander,
    pub disk_detector: D,
    pub node_name: Option<String>,
    pub taint_key: String,
    pub bottlerocket_enable_swap: bool,
    pub hack_restart_kubelet_enable_swap: bool,
    pub kubelet_healthz_addr: SocketAddr,
    pub remove_taint: bool,
    pub apply_sysctls: bool,
    pub vm_swappiness: usize,
    pub vm_min_free_kbytes: usize,
    pub vm_watermark_scale_factor: usize,
}
impl<D: DiskDetectorTrait> SwapController<D> {
    pub async fn setup(&self) {
        info!("Starting NVMe disk configuration with swap...");
        let devices = self.disk_detector.detect_devices();
        for device in &devices {
            if !self.is_existing_swap(device) {
                info!("Configuring swap on {device}");
                self.mkswap(device);
                self.swapon(device);
            }
        }

        if self.apply_sysctls {
            info!("Setting sysctls to improve swap performance and safety");
            self.sysctl("vm.swappiness", self.vm_swappiness);
            self.sysctl("vm.min_free_kbytes", self.vm_min_free_kbytes);
            self.sysctl("vm.watermark_scale_factor", self.vm_watermark_scale_factor);
        }

        if self.bottlerocket_enable_swap {
            info!("Enabling swap with the Bottlerocket apiclient");
            self.commander.check_output(&[
                "apiclient",
                "set",
                "settings.kubernetes.memory-swap-behavior=LimitedSwap",
            ]);
        }

        if self.hack_restart_kubelet_enable_swap {
            info!("Hackily enabling swap by modifying the Kubelet config and restarting it.");
            match self.cloud_provider {
                CloudProvider::Gcp => {
                    update_kubelet_config("/host/home/kubernetes/kubelet-config.yaml");
                }
                CloudProvider::Azure => {
                    let environment = self.commander.check_output(&[
                        "chroot",
                        "/host",
                        "systemctl",
                        "show",
                        "kubelet.service",
                        "-p",
                        "Environment",
                    ]);
                    match kubelet_config_file(&String::from_utf8(environment.stdout).unwrap()) {
                        // Node Auto Provisioning nodes already configure the kubelet
                        // with a config file, so update it rather than replacing it.
                        Some(path) => update_kubelet_config(&format!("/host{path}")),
                        // VMSS node pools don't use a kubelet config file, and
                        // there isn't a command line flag to enable LimitedSwap.
                        // They reference an env var for the kubelet config file
                        // args, but it isn't set initially.
                        None => {
                            update_kubelet_config("/host/var/lib/kubelet/config.yaml");
                            fs::write(
                                format!("{AZURE_KUBELET_DROPIN_DIR}/99-enable-swap.conf"),
                                r#"[Service]
Environment="KUBELET_CONFIG_FILE_FLAGS=--config /var/lib/kubelet/config.yaml""#,
                            )
                            .unwrap();
                        }
                    }
                }
                _ => panic!(
                    "Hack enabling swap by restarting the kubelet is not supported for cloud provider: {:?}",
                    self.cloud_provider
                ),
            }

            self.commander
                .check_output(&["chroot", "/host", "systemctl", "daemon-reload"]);

            self.commander.check_output(&[
                "chroot",
                "/host",
                "systemctl",
                "restart",
                "kubelet.service",
            ]);

            // systemctl returns once the kubelet has started, not once it's
            // healthy. Don't let pods onto the node if it never recovers.
            wait_for_kubelet_healthy(self.kubelet_healthz_addr, KUBELET_HEALTHY_TIMEOUT);
        }

        info!("Swap setup completed successfully");
        if self.remove_taint {
            remove_taint(
                self.node_name.as_ref().expect("clap enforced"),
                &self.taint_key,
            )
            .await;
        }
    }

    fn mkswap(&self, device: &str) {
        self.commander.check_output(&["mkswap", device]);
    }

    fn swapon(&self, device: &str) {
        // Explicitly set all devices to the same priority, so Linux will
        // allocate pages to disks round-robin, allowing for faster I/O
        // on machines with multiple disks.
        self.commander
            .check_output(&["swapon", "-p", "2048", device]);
    }

    fn is_existing_swap(&self, device: &str) -> bool {
        // /proc/swaps has contents like:
        // Filename				Type		Size		Used		Priority
        // /nvme0n1                                partition	393215996	0		-2
        std::fs::read_to_string("/proc/swaps")
            .expect("failed to read /proc/swaps")
            .trim()
            .lines()
            .skip(1)
            .map(|line| line.split_whitespace().next().unwrap())
            // /proc/swaps is inconsistent in how it reports things,
            // sometimes leaving off the /dev at the beginning of the path.
            .any(|line| device.ends_with(line))
    }

    fn sysctl(&self, key: &str, value: usize) {
        self.commander
            .check_output(&["sysctl", &format!("{key}={value}")]);
    }
}

fn update_kubelet_config(path: &str) {
    // Read existing configuration, if any.
    let mut kubelet_config: BTreeMap<String, Value> = match fs::read(path) {
        Ok(data) => serde_saphyr::from_slice(&data).unwrap(),
        Err(e) if e.kind() == ErrorKind::NotFound => BTreeMap::new(),
        Err(e) => panic!("failed to read kubelet config {path}: {e:?}"),
    };

    // Ensure we have the type information, in case we're making a new file.
    kubelet_config
        .entry("kind".to_owned())
        .or_insert(Value::String("KubeletConfiguration".to_owned()));
    kubelet_config
        .entry("apiVersion".to_owned())
        .or_insert(Value::String("kubelet.config.k8s.io/v1beta1".to_owned()));

    // Enable swap.
    kubelet_config.insert("failSwapOn".to_owned(), Value::Bool(false));
    let mut memory_swap = Map::new();
    memory_swap.insert(
        "swapBehavior".to_owned(),
        Value::String("LimitedSwap".to_owned()),
    );
    kubelet_config.insert("memorySwap".to_owned(), Value::Object(memory_swap));

    // Write the updates, keeping a JSON file JSON.
    let data = if path.ends_with(".json") {
        serde_json::to_string_pretty(&kubelet_config).unwrap()
    } else {
        serde_saphyr::to_string(&kubelet_config).unwrap()
    };
    fs::write(path, data).unwrap();
}

/// Waits until the kubelet's healthz endpoint at `addr` returns `ok`.
/// Panics if it doesn't within `timeout`.
fn wait_for_kubelet_healthy(addr: SocketAddr, timeout: Duration) {
    info!("Waiting for the kubelet to be healthy");
    let start = Instant::now();
    loop {
        match kubelet_healthz(addr) {
            Ok(true) => {
                info!("Kubelet is healthy");
                return;
            }
            Ok(false) => debug!("Kubelet isn't healthy yet"),
            Err(e) => debug!("Kubelet healthz isn't reachable yet: {e}"),
        }
        assert!(
            start.elapsed() < timeout,
            "kubelet wasn't healthy at {addr} after {timeout:?}"
        );
        sleep(Duration::from_secs(1));
    }
}

/// Returns whether `GET /healthz` at `addr` returns 200 with the body `ok`.
fn kubelet_healthz(addr: SocketAddr) -> std::io::Result<bool> {
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    // HTTP/1.0, so the server closes the connection after responding.
    stream.write_all(b"GET /healthz HTTP/1.0\r\nHost: localhost\r\n\r\n")?;
    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        return Ok(false);
    };
    Ok(head.split_whitespace().nth(1) == Some("200") && body == "ok")
}

/// Returns the kubelet config file passed with
/// `KUBELET_CONFIG_FILE_FLAGS=--config <path>`, given the output of
/// `systemctl show -p Environment`. systemd has already merged the drop-ins,
/// and quotes assignments containing spaces, e.g. `Environment="A=b c" D=e`.
fn kubelet_config_file(environment: &str) -> Option<String> {
    let words = environment
        .trim()
        .strip_prefix("Environment=")
        .and_then(shlex::split)
        .unwrap_or_else(|| panic!("unexpected systemctl show output: {environment:?}"));
    let value = words
        .iter()
        .filter_map(|word| word.strip_prefix("KUBELET_CONFIG_FILE_FLAGS="))
        .next_back()?;
    config_flag_path(value)
}

/// Finds `--config <path>` or `--config=<path>` among the flags. systemd splits
/// the variable on whitespace when expanding it, so we do too, and the kubelet
/// takes the last `--config`, so we do too. Panics on a non-empty value without
/// an absolute `--config` path: falling back to our own drop-in would replace
/// those flags.
fn config_flag_path(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        return None;
    }
    let mut config = None;
    let mut args = value.split_whitespace();
    while let Some(arg) = args.next() {
        let path = match arg.strip_prefix("--config=") {
            Some(path) => Some(path),
            None if arg == "--config" => args.next(),
            None => continue,
        };
        config = path.filter(|p| !p.is_empty());
        if config.is_none() {
            break;
        }
    }
    let Some(path) = config else {
        panic!("no --config path in KUBELET_CONFIG_FILE_FLAGS: {value:?}");
    };
    assert!(
        path.starts_with('/'),
        "relative --config path in KUBELET_CONFIG_FILE_FLAGS: {path:?}"
    );
    Some(path.to_owned())
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::*;

    /// Serves one healthz response per connection, in order.
    fn serve_healthz(responses: &'static [(&'static str, &'static str)]) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buf = [0; 1024];
                while !request.ends_with(b"\r\n\r\n") {
                    let n = stream.read(&mut buf).unwrap();
                    request.extend_from_slice(&buf[..n]);
                }
                write!(
                    stream,
                    "HTTP/1.0 {status}\r\nContent-Length: {}\r\n\r\n{body}",
                    body.len()
                )
                .unwrap();
            }
        });
        addr
    }

    #[test]
    fn test_wait_for_kubelet_healthy() {
        let addr = serve_healthz(&[
            ("500 Internal Server Error", "[-]syncloop failed"),
            ("200 OK", "ok"),
        ]);
        wait_for_kubelet_healthy(addr, Duration::from_secs(10));
    }

    #[test]
    #[should_panic(expected = "kubelet wasn't healthy")]
    fn test_wait_for_kubelet_healthy_timeout() {
        // Nothing listens on a port once its listener is dropped.
        let addr = TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        wait_for_kubelet_healthy(addr, Duration::from_secs(2));
    }

    #[test]
    fn test_kubelet_config_file_nap() {
        assert_eq!(
            kubelet_config_file(
                "Environment=KUBELET_CGROUP_FLAGS=--cgroup-driver=systemd \"KUBELET_CONFIG_FILE_FLAGS=--config /etc/default/kubeletconfig.json\" \"KUBELET_FLAGS=--v=2 --node-ip=10.0.0.1\"\n"
            ),
            Some("/etc/default/kubeletconfig.json".to_owned()),
        );
    }

    #[test]
    fn test_kubelet_config_file_vmss() {
        for environment in [
            "Environment=\n",
            "Environment=KUBELET_CONTAINERD_FLAGS=--runtime-request-timeout=15m\n",
            "Environment=KUBELET_CONFIG_FILE_FLAGS= \"KUBELET_FLAGS=--v=2 --node-ip=10.0.0.1\"\n",
        ] {
            assert_eq!(kubelet_config_file(environment), None, "{environment}");
        }
    }

    #[test]
    fn test_kubelet_config_file_flag_forms() {
        for environment in [
            "Environment=KUBELET_CONFIG_FILE_FLAGS=--config=/etc/default/kubeletconfig.json",
            "Environment=\"KUBELET_CONFIG_FILE_FLAGS=--v=2 --config /etc/default/kubeletconfig.json\"",
            "Environment=\"KUBELET_CONFIG_FILE_FLAGS=--v=2  --config=/etc/default/kubeletconfig.json --foo bar\"",
            "Environment=\"KUBELET_CONFIG_FILE_FLAGS=--config /etc/kubelet.yaml --config /etc/default/kubeletconfig.json\"",
        ] {
            assert_eq!(
                kubelet_config_file(environment),
                Some("/etc/default/kubeletconfig.json".to_owned()),
                "{environment}",
            );
        }
    }

    #[test]
    #[should_panic(expected = "no --config path in KUBELET_CONFIG_FILE_FLAGS")]
    fn test_kubelet_config_file_without_config() {
        kubelet_config_file("Environment=KUBELET_CONFIG_FILE_FLAGS=--v=2");
    }

    #[test]
    #[should_panic(expected = "no --config path in KUBELET_CONFIG_FILE_FLAGS")]
    fn test_kubelet_config_file_config_without_path() {
        kubelet_config_file("Environment=\"KUBELET_CONFIG_FILE_FLAGS=--v=2 --config\"");
    }

    #[test]
    #[should_panic(expected = "relative --config path in KUBELET_CONFIG_FILE_FLAGS")]
    fn test_kubelet_config_file_relative_config() {
        kubelet_config_file(
            "Environment=\"KUBELET_CONFIG_FILE_FLAGS=--config kubeletconfig.json\"",
        );
    }

    #[test]
    fn test_update_kubelet_config_keeps_existing_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kubeletconfig.json");
        let path = path.to_str().unwrap();
        fs::write(
            path,
            r#"{"kind":"KubeletConfiguration","apiVersion":"kubelet.config.k8s.io/v1beta1","clusterDNS":["10.1.0.10"],"authentication":{"webhook":{"enabled":true}},"failSwapOn":true}"#,
        )
        .unwrap();

        update_kubelet_config(path);

        let config: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(config["clusterDNS"], serde_json::json!(["10.1.0.10"]));
        assert_eq!(config["authentication"]["webhook"]["enabled"], true);
        assert_eq!(config["failSwapOn"], false);
        assert_eq!(config["memorySwap"]["swapBehavior"], "LimitedSwap");
    }
}
