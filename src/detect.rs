use serde::Deserialize;

use crate::{CloudProvider, Commander};

const BOTTLEROCKET_ROOTFS_PATH: &str = "/.bottlerocket/rootfs";

#[derive(Deserialize)]
struct Lsblk {
    blockdevices: Vec<LsblkBlockDevice>,
}
#[derive(Deserialize, Debug, PartialEq)]
struct LsblkBlockDevice {
    children: Option<Vec<LsblkBlockDevice>>,
    // Arbitrary string identifying the device model.
    // Not all cloud providers set this to a reasonable value.
    // GCP :(
    model: Option<String>,
    mountpoint: Option<String>,
    // Device path (ie: /dev/nvme0n1)
    // Note that in bottlerocket, this still only starts with /dev,
    // even though we're in a container that has it in /.bottlerocket/rootfs/dev
    path: String,
    // Connection of device (nvme, sata, etc...)
    tran: Option<String>,
    // Type of device (disk, part, etc...)
    #[serde(rename = "type")]
    type_: String,
}

trait LsblkIteratorExt {
    fn filter_model(self, model_filter: &str) -> impl Iterator<Item = LsblkBlockDevice>;
    fn paths(self) -> impl Iterator<Item = String>;
}

impl<I: Iterator<Item = LsblkBlockDevice>> LsblkIteratorExt for I {
    fn filter_model(self, model_filter: &str) -> impl Iterator<Item = LsblkBlockDevice> {
        self.filter(move |device| {
            device
                .model
                .as_ref()
                .map(|model| {
                    println!("Checking model: {model}");
                    println!("against filter: {model_filter}");
                    model.contains(model_filter)
                })
                .unwrap_or(false)
        })
    }

    fn paths(self) -> impl Iterator<Item = String> {
        self.map(|device| device.path)
    }
}

pub trait DiskDetectorTrait {
    fn detect_devices(&self) -> Vec<String>;
}

impl DiskDetectorTrait for DiskDetector {
    fn detect_devices(&self) -> Vec<String> {
        println!(
            "Detecting disks for cloud provider: {:?}",
            self.cloud_provider
        );
        let devices = match self.cloud_provider {
            CloudProvider::Aws => self.detect_aws_devices(),
            CloudProvider::Gcp => self.detect_gcp_devices(),
            CloudProvider::Azure => self.detect_azure_devices(),
            CloudProvider::Generic => self.detect_generic_devices(),
        };
        if devices.is_empty() {
            panic!("No suitable NVMe devices found");
        }
        println!("Found devices: {:?}", &devices);
        devices
    }
}

pub struct DiskDetector {
    cloud_provider: CloudProvider,
    commander: Commander,
}

impl DiskDetector {
    pub fn new(commander: Commander, cloud_provider: CloudProvider) -> Self {
        DiskDetector {
            cloud_provider,
            commander,
        }
    }
    fn lsblk(&self) -> impl Iterator<Item = LsblkBlockDevice> {
        let output = self
            .commander
            .check_output(&["lsblk", "--json", "--output-all"]);
        serde_json::from_slice::<Lsblk>(&output.stdout)
            .expect("Failed to deserialize output of 'lsblk --json --output-all'")
            .blockdevices
            .into_iter()
            .filter(|device| {
                device.mountpoint.is_none()
                    && device
                        .children
                        .as_ref()
                        .map(|children| children.is_empty())
                        .unwrap_or(true)
                    && device.tran.as_deref() == Some("nvme")
                    && device.type_ == "disk"
            })
    }

    fn find(&self, dir: &str, name: &str) -> Vec<String> {
        let mut devices: Vec<String> = String::from_utf8_lossy(
            &self
                .commander
                .check_output(&["find", dir, "-name", name])
                .stdout,
        )
        .trim()
        .lines()
        // We only want full disks, not partitions on them.
        .filter(|line| !line.contains("-part"))
        .map(|line| {
            // Get the device path without links,
            // so we can later remove duplicates
            // and compare with lsblk output.
            #[cfg(not(test))]
            return std::fs::canonicalize(line)
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();

            #[cfg(test)]
            {
                println!("{line}");
                let ordinal = line.chars().last().unwrap();
                format!("/dev/nvme{ordinal}n1")
            }
        })
        .collect();
        devices.sort();
        devices.dedup();
        devices
    }

    fn detect_aws_devices(&self) -> Vec<String> {
        if std::fs::exists(BOTTLEROCKET_ROOTFS_PATH).unwrap() {
            self.detect_aws_bottlerocket_devices()
        } else {
            self.detect_aws_standard_devices()
        }
    }

    fn detect_aws_bottlerocket_devices(&self) -> Vec<String> {
        self.detect_aws_standard_devices()
            .into_iter()
            .map(|path| format!("{BOTTLEROCKET_ROOTFS_PATH}{path}"))
            .collect()
    }
    fn detect_aws_standard_devices(&self) -> Vec<String> {
        self.lsblk()
            .filter_model("Amazon EC2 NVMe Instance Storage")
            .paths()
            .collect()
    }

    fn detect_gcp_devices(&self) -> Vec<String> {
        // `lsblk` doesn't contain a descriptive model for
        // GCP devices, so out of paranoia, we use `find` to
        // filter to local SSDs. We don't only use `find`
        // because the devices might have partitions or other
        // children we need to filter out.
        // All local disks will take the form of google-local-*.
        // We'll make the assumption that the machine has homogeneous
        // disk setup, and that the disks the user configured or are
        // provided by the machine are NVME or equivilently fast.
        let find_paths = [self.find("/dev/disk/by-id", "google-local-*")].concat();

        self.lsblk()
            .paths()
            .filter(|path| find_paths.contains(path))
            .collect()
    }

    fn detect_azure_devices(&self) -> Vec<String> {
        self.lsblk()
            .filter_model("Microsoft NVMe Direct Disk")
            .paths()
            .collect()
    }

    fn detect_generic_devices(&self) -> Vec<String> {
        self.lsblk().paths().collect()
    }
}

#[cfg(test)]
mod test {
    use crate::CloudProvider;
    use crate::detect::{DiskDetector, LsblkBlockDevice};
    use crate::test::TestEnv;

    #[test]
    fn test_lsblk_filters() {
        let test_env = TestEnv::new();
        let disk_detector = DiskDetector::new(test_env.commander.clone(), CloudProvider::Aws);

        let lsblk_output = test_env.read_testdata("testdata/lsblk_contrived.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected: Vec<LsblkBlockDevice> = vec![
            LsblkBlockDevice {
                children: Some(vec![]),
                model: Some("Amazon EC2 NVMe Instance Storage".to_owned()),
                mountpoint: None,
                path: "/dev/nvme0n1".to_owned(),
                tran: Some("nvme".to_owned()),
                type_: "disk".to_owned(),
            },
            LsblkBlockDevice {
                children: Some(vec![]),
                model: Some("Amazon EC2 NVMe Instance Storage".to_owned()),
                mountpoint: None,
                path: "/dev/nvme1n1".to_owned(),
                tran: Some("nvme".to_owned()),
                type_: "disk".to_owned(),
            },
            LsblkBlockDevice {
                children: Some(vec![]),
                model: Some("some other model".to_owned()),
                mountpoint: None,
                path: "/dev/nvme2n1".to_owned(),
                tran: Some("nvme".to_owned()),
                type_: "disk".to_owned(),
            },
            LsblkBlockDevice {
                children: None,
                model: Some("Amazon EC2 NVMe Instance Storage".to_owned()),
                mountpoint: None,
                path: "/dev/nvme7n1".to_owned(),
                tran: Some("nvme".to_owned()),
                type_: "disk".to_owned(),
            },
            LsblkBlockDevice {
                children: None,
                model: Some("Microsoft NVMe Direct Disk v49990322".to_owned()),
                mountpoint: None,
                path: "/dev/nvme8n1".to_owned(),
                tran: Some("nvme".to_owned()),
                type_: "disk".to_owned(),
            },
            LsblkBlockDevice {
                children: None,
                model: Some("nvme_card".to_owned()),
                mountpoint: None,
                path: "/dev/nvme9n1".to_owned(),
                tran: Some("nvme".to_owned()),
                type_: "disk".to_owned(),
            },
        ];
        let actual: Vec<LsblkBlockDevice> = disk_detector.lsblk().collect();
        assert_eq!(expected, actual);

        let lsblk_output = test_env.read_testdata("testdata/aws/lsblk.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected: Vec<LsblkBlockDevice> = vec![LsblkBlockDevice {
            children: None,
            model: Some("Amazon EC2 NVMe Instance Storage        ".to_owned()),
            mountpoint: None,
            path: "/dev/nvme1n1".to_owned(),
            tran: Some("nvme".to_owned()),
            type_: "disk".to_owned(),
        }];
        let actual: Vec<LsblkBlockDevice> = disk_detector.lsblk().collect();
        assert_eq!(expected, actual);

        let lsblk_output = test_env.read_testdata("testdata/azure/lsblk.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected: Vec<LsblkBlockDevice> = vec![LsblkBlockDevice {
            children: None,
            model: Some("Microsoft NVMe Direct Disk v2           ".to_owned()),
            mountpoint: None,
            path: "/dev/nvme0n1".to_owned(),
            tran: Some("nvme".to_owned()),
            type_: "disk".to_owned(),
        }];
        let actual: Vec<LsblkBlockDevice> = disk_detector.lsblk().collect();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_detect_aws_bottlerocket_devices() {
        let test_env = TestEnv::new();
        let disk_detector = DiskDetector::new(test_env.commander.clone(), CloudProvider::Aws);

        let lsblk_output = test_env.read_testdata("testdata/aws/lsblk.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec!["/.bottlerocket/rootfs/dev/nvme1n1".to_owned()];
        let actual = disk_detector.detect_aws_bottlerocket_devices();
        assert_eq!(expected, actual);

        let lsblk_output = test_env.read_testdata("testdata/lsblk_contrived.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec![
            "/.bottlerocket/rootfs/dev/nvme0n1".to_owned(),
            "/.bottlerocket/rootfs/dev/nvme1n1".to_owned(),
            "/.bottlerocket/rootfs/dev/nvme7n1".to_owned(),
        ];
        let actual = disk_detector.detect_aws_bottlerocket_devices();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_detect_aws_standard_devices() {
        let test_env = TestEnv::new();
        let disk_detector = DiskDetector::new(test_env.commander.clone(), CloudProvider::Aws);

        let lsblk_output = test_env.read_testdata("testdata/aws/lsblk.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec!["/dev/nvme1n1".to_owned()];
        let actual = disk_detector.detect_aws_standard_devices();
        assert_eq!(expected, actual);

        let lsblk_output = test_env.read_testdata("testdata/lsblk_contrived.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec![
            "/dev/nvme0n1".to_owned(),
            "/dev/nvme1n1".to_owned(),
            "/dev/nvme7n1".to_owned(),
        ];
        let actual = disk_detector.detect_aws_standard_devices();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_detect_azure_devices() {
        let test_env = TestEnv::new();
        let disk_detector = DiskDetector::new(test_env.commander.clone(), CloudProvider::Azure);

        let lsblk_output = test_env.read_testdata("testdata/azure/lsblk.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec!["/dev/nvme0n1".to_owned()];
        let actual = disk_detector.detect_azure_devices();
        assert_eq!(expected, actual);

        let lsblk_output = test_env.read_testdata("testdata/lsblk_contrived.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec!["/dev/nvme8n1".to_owned()];
        let actual = disk_detector.detect_azure_devices();
        assert_eq!(expected, actual);
    }

    #[test]
    fn test_detect_gcp_devices() {
        let test_env = TestEnv::new();
        let disk_detector = DiskDetector::new(test_env.commander.clone(), CloudProvider::Gcp);

        let lsblk_output = test_env.read_testdata("testdata/gcp/lsblk.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        // For testing, we evaluate the links to
        // /dev/nvme{last_char_from_find_line}n1
        test_env.mock(
            "find",
            0,
            r#"/dev/disk/by-id/google-local-nvme-ssd-1
/dev/disk/by-id/google-local-ssd-block0
/dev/disk/by-id/google-persistent-disk-0
/dev/disk/by-id/google-persistent-disk-0-part3
/dev/disk/by-id/google-persistent-disk-0-part4
/dev/disk/by-id/nvme-nvme_card_nvme_card5
"#,
        );
        let expected = vec!["/dev/nvme0n1".to_owned()];
        let actual = disk_detector.detect_gcp_devices();
        assert_eq!(expected, actual);

        test_env.mock(
            "find",
            0,
            // We sort these before comparing with lsblk
            r#"/dev/disk/by-id/google-local-ssd-block9
/dev/disk/by-id/google-local-ssd-doesnt-match-lsblk4
/dev/disk/by-id/google-local-ssd-ok-lsblk2
"#,
        );
        let lsblk_output = test_env.read_testdata("testdata/lsblk_contrived.json");
        test_env.mock("lsblk", 0, &lsblk_output);
        let expected = vec!["/dev/nvme2n1".to_owned(), "/dev/nvme9n1".to_owned()];
        let actual = disk_detector.detect_gcp_devices();
        assert_eq!(expected, actual);
    }
}
