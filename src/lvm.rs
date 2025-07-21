use serde::Deserialize;

use crate::Commander;
use crate::detect::DiskDetectorTrait;
use crate::remove_taint::remove_taint;

#[derive(Deserialize)]
struct LvmReportWrapper {
    report: Vec<LvmReport>,
}

#[derive(Deserialize)]
struct LvmReport {
    vg: Option<Vec<VgReport>>,
    pv: Option<Vec<PvReport>>,
}

#[derive(Deserialize)]
struct VgReport {
    vg_name: String,
}

#[derive(Deserialize)]
struct PvReport {
    pv_name: String,
}

pub struct LvmController<D: DiskDetectorTrait> {
    pub commander: Commander,
    pub disk_detector: D,
    pub node_name: String,
    pub taint_key: String,
    pub vg_name: String,
}

impl<D: DiskDetectorTrait> LvmController<D> {
    pub fn setup(&self) {
        println!("Starting NVMe disk configuration with LVM...");
        if self.volume_group_exists() {
            println!("Volume group {} already exists.", self.vg_name);
        } else {
            let devices = self.disk_detector.detect_devices();
            for device in &devices {
                if !self.physical_volume_exists(device) {
                    self.pvcreate(device);
                }
            }
            self.vgcreate(&devices);
        }
        println!("LVM setup completed successfully");
        remove_taint(self.commander.clone(), &self.node_name, &self.taint_key);
    }

    fn volume_group_exists(&self) -> bool {
        let vgs_report = self
            .commander
            .check_output(&["vgs", "--reportformat", "json"]);
        let vgs_report: LvmReportWrapper = serde_json::from_slice(&vgs_report.stdout)
            .expect("Failed to deserialize output of 'vgs --reportformat json'");
        vgs_report.report[0]
            .vg
            .as_ref()
            .unwrap()
            .iter()
            .any(|vg| vg.vg_name == self.vg_name)
    }

    fn physical_volume_exists(&self, device: &str) -> bool {
        let pvs_report = self
            .commander
            .check_output(&["pvs", "--reportformat", "json"]);
        let pvs_report: LvmReportWrapper = serde_json::from_slice(&pvs_report.stdout)
            .expect("Failed to deserialize output of 'pvs --reportformat json'");
        pvs_report.report[0]
            .pv
            .as_ref()
            .unwrap()
            .iter()
            .any(|pv| pv.pv_name == device)
    }

    fn pvcreate(&self, device: &str) {
        println!("Creating physical volume on {device}");
        self.commander.check_output(&["pvcreate", "-f", device]);
    }

    fn vgcreate(&self, devices: &[String]) {
        println!("Creating volume group {}", &self.vg_name);
        let mut args = Vec::with_capacity(devices.len() + 2);
        args.push("vgcreate");
        args.push(&self.vg_name);
        args.extend(devices.iter().map(|d| d.as_str()));
        self.commander.check_output(&args);
    }
}
