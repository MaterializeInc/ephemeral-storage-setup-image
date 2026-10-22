use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use serde_yaml::{Mapping, Value};
use tracing::info;

use crate::detect::DiskDetectorTrait;
use crate::remove_taint::remove_taint;
use crate::{CloudProvider, Commander};

const AZURE_KUBELET_DROPIN_DIR: &str = "/host/etc/systemd/system/kubelet.service.d";

pub struct SwapController<D: DiskDetectorTrait> {
    pub cloud_provider: CloudProvider,
    pub commander: Commander,
    pub disk_detector: D,
    pub node_name: Option<String>,
    pub taint_key: String,
    pub bottlerocket_enable_swap: bool,
    pub hack_restart_kubelet_enable_swap: bool,
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
                    match existing_kubelet_config_file(AZURE_KUBELET_DROPIN_DIR) {
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
        Ok(data) => serde_yaml::from_slice(&data).unwrap(),
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
    let mut memory_swap = Mapping::new();
    memory_swap.insert(
        Value::String("swapBehavior".to_owned()),
        Value::String("LimitedSwap".to_owned()),
    );
    kubelet_config.insert("memorySwap".to_owned(), Value::Mapping(memory_swap));

    // Write the updates, keeping a JSON file JSON.
    let data = if path.ends_with(".json") {
        serde_json::to_string_pretty(&kubelet_config).unwrap()
    } else {
        serde_yaml::to_string(&kubelet_config).unwrap()
    };
    fs::write(path, data).unwrap();
}

/// Returns the kubelet config file a systemd drop-in in `dir` passes with
/// `KUBELET_CONFIG_FILE_FLAGS=--config <path>`. Drop-ins apply in lexical
/// order, so the last one wins.
fn existing_kubelet_config_file(dir: &str) -> Option<String> {
    let mut entries: Vec<_> = match fs::read_dir(dir) {
        Ok(entries) => entries.map(|e| e.unwrap().path()).collect(),
        Err(e) if e.kind() == ErrorKind::NotFound => return None,
        Err(e) => panic!("failed to read {dir}: {e:?}"),
    };
    entries.sort();
    entries
        .iter()
        .filter_map(|p| config_flag_path(p))
        .next_back()
}

/// Finds `--config <path>` or `--config=<path>` among the flags. systemd splits
/// the variable on whitespace when expanding it, so we do too, and the kubelet
/// takes the last `--config`, so we do too. Panics on a non-empty value without
/// a `--config` path: falling back to our own drop-in would replace those flags.
fn config_flag_path(dropin: &Path) -> Option<String> {
    let content = fs::read_to_string(dropin).ok()?;
    let (_, rest) = content.split_once("KUBELET_CONFIG_FILE_FLAGS=")?;
    let value = rest.split(['"', '\n']).next().unwrap().trim();
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
        panic!(
            "no --config path in KUBELET_CONFIG_FILE_FLAGS in {}: {value:?}",
            dropin.display()
        );
    };
    Some(path.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_dropins(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (name, content) in files {
            fs::write(dir.path().join(name), content).unwrap();
        }
        dir
    }

    #[test]
    fn test_existing_kubelet_config_file_nap() {
        let dir = write_dropins(&[
            (
                "10-cgroupv2.conf",
                "[Service]\nEnvironment=\"KUBELET_CGROUP_FLAGS=--cgroup-driver=systemd\"\n",
            ),
            (
                "10-componentconfig.conf",
                "[Service]\nEnvironment=\"KUBELET_CONFIG_FILE_FLAGS=--config /etc/default/kubeletconfig.json\"\n",
            ),
        ]);
        assert_eq!(
            existing_kubelet_config_file(dir.path().to_str().unwrap()),
            Some("/etc/default/kubeletconfig.json".to_owned()),
        );
    }

    #[test]
    fn test_existing_kubelet_config_file_vmss() {
        let dir = write_dropins(&[(
            "10-containerd-base-flag.conf",
            "[Service]\nEnvironment=\"KUBELET_CONTAINERD_FLAGS=--runtime-request-timeout=15m\"\n",
        )]);
        assert_eq!(
            existing_kubelet_config_file(dir.path().to_str().unwrap()),
            None
        );
        assert_eq!(existing_kubelet_config_file("/nonexistent"), None);

        let dir = write_dropins(&[(
            "10-componentconfig.conf",
            "[Service]\nEnvironment=\"KUBELET_CONFIG_FILE_FLAGS=\"\n",
        )]);
        assert_eq!(
            existing_kubelet_config_file(dir.path().to_str().unwrap()),
            None
        );
    }

    #[test]
    fn test_existing_kubelet_config_file_flag_forms() {
        for flags in [
            "--config=/etc/default/kubeletconfig.json",
            "--v=2 --config /etc/default/kubeletconfig.json",
            "--v=2  --config=/etc/default/kubeletconfig.json --foo bar",
            "--config /etc/kubelet.yaml --config /etc/default/kubeletconfig.json",
        ] {
            let dir = write_dropins(&[(
                "10-componentconfig.conf",
                &format!("[Service]\nEnvironment=\"KUBELET_CONFIG_FILE_FLAGS={flags}\"\n"),
            )]);
            assert_eq!(
                existing_kubelet_config_file(dir.path().to_str().unwrap()),
                Some("/etc/default/kubeletconfig.json".to_owned()),
                "{flags}",
            );
        }
    }

    #[test]
    #[should_panic(expected = "no --config path in KUBELET_CONFIG_FILE_FLAGS")]
    fn test_existing_kubelet_config_file_without_config() {
        let dir = write_dropins(&[(
            "10-componentconfig.conf",
            "[Service]\nEnvironment=\"KUBELET_CONFIG_FILE_FLAGS=--v=2\"\n",
        )]);
        existing_kubelet_config_file(dir.path().to_str().unwrap());
    }

    #[test]
    #[should_panic(expected = "no --config path in KUBELET_CONFIG_FILE_FLAGS")]
    fn test_existing_kubelet_config_file_config_without_path() {
        let dir = write_dropins(&[(
            "10-componentconfig.conf",
            "[Service]\nEnvironment=\"KUBELET_CONFIG_FILE_FLAGS=--v=2 --config\"\n",
        )]);
        existing_kubelet_config_file(dir.path().to_str().unwrap());
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
