use crate::Commander;
use crate::detect::DiskDetectorTrait;
use crate::remove_taint::remove_taint;

pub struct SwapController<D: DiskDetectorTrait> {
    pub commander: Commander,
    pub disk_detector: D,
    pub node_name: String,
    pub taint_key: String,
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
        println!("Setting sysctls to improve swap performance and safety");
        self.sysctl("vm.swappiness", 100);
        self.sysctl("vm.min_free_kbytes", 1048576);
        self.sysctl("vm.watermark_scale_factor", 100);
        println!("Swap setup completed successfully");
        remove_taint(&self.node_name, &self.taint_key).await;
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
}
