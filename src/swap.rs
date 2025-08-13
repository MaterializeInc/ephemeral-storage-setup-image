use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;

use serde_yaml::{Mapping, Value};

use crate::detect::DiskDetectorTrait;
use crate::remove_taint::remove_taint;
use crate::{CloudProvider, Commander};

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
        println!("Starting NVMe disk configuration with swap...");
        let devices = self.disk_detector.detect_devices();
        for device in &devices {
            if !self.is_existing_swap(device) {
                println!("Configuring swap on {device}");
                self.mkswap(device);
                self.swapon(device);
            }
        }

        if self.apply_sysctls {
            println!("Setting sysctls to improve swap performance and safety");
            self.sysctl("vm.swappiness", self.vm_swappiness);
            self.sysctl("vm.min_free_kbytes", self.vm_min_free_kbytes);
            self.sysctl("vm.watermark_scale_factor", self.vm_watermark_scale_factor);
        }

        if self.bottlerocket_enable_swap {
            println!("Enabling swap with the Bottlerocket apiclient");
            self.commander.check_output(&[
                "apiclient",
                "set",
                "settings.kubernetes.memory-swap-behavior=LimitedSwap",
            ]);
        }

        if self.hack_restart_kubelet_enable_swap {
            println!("Hackily enabling swap by modifying the Kubelet config and restarting it.");
            match self.cloud_provider {
                CloudProvider::Gcp => {
                    self.update_kubelet_config("/host/home/kubernetes/kubelet-config.yaml");
                }
                CloudProvider::Azure => {
                    // Azure doesn't use a kubelet config file by default,
                    // and there isn't a command line flag to enable LimitedSwap.
                    self.update_kubelet_config("/host/var/lib/kubelet/config.yaml");
                    // Azure does reference an env var for the kubelet config file args,
                    // but it isn't set initially.
                    fs::write(
                        "/host/etc/systemd/system/kubelet.service.d/99-enable-swap.conf",
                        r#"[Service]
Environment="KUBELET_CONFIG_FILE_FLAGS=--config /var/lib/kubelet/config.yaml""#,
                    )
                    .unwrap();
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

        println!("Swap setup completed successfully");
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
        self.commander.check_output(&["swapon", device]);
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

    fn update_kubelet_config(&self, path: &str) {
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

        // Write the updates.
        fs::write(path, serde_yaml::to_string(&kubelet_config).unwrap()).unwrap();
    }
}
