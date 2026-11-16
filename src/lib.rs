use std::collections::HashMap;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};
use std::time::Duration;

use clap::ValueEnum;
use tracing::{info, warn};

use crate::detect::BOTTLEROCKET_ROOTFS_PATH;

pub mod detect;
pub mod lvm;
mod remove_taint;
pub mod swap;

#[derive(ValueEnum, Clone, Copy, Debug)]
pub enum CloudProvider {
    Aws,
    Gcp,
    Azure,
    Generic,
}

#[derive(Clone, Default)]
pub struct Commander {
    // Environment variables to set on child processes.
    // This is mostly useful in testing to point at mocks.
    pub(crate) envs: HashMap<String, String>,
}

impl Commander {
    fn check_output(&self, args: &[&str]) -> Output {
        let failure_msg = format!("Failed to run '{args:?}'");
        let output = self.unchecked_output(args);
        let rc = output.status.code();
        if rc.unwrap() != 0 {
            panic!(
                "{failure_msg}:
Exit code: {rc:?}
Stdout:
{}
Stderr:
{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
        }
        output
    }

    fn unchecked_output(&self, args: &[&str]) -> Output {
        // We still check if we can even spawn the process,
        // we just don't check the return code.
        let failure_msg = format!("Failed to spawn '{args:?}'");
        #[cfg(test)]
        let _guard = test::SPAWN_LOCK.read().unwrap();
        Command::new(args[0])
            .args(&args[1..])
            .envs(&self.envs)
            .output()
            .expect(&failure_msg)
    }
}

/// Bottlerocket runs bootstrap containers before the main containerd, and their
/// shim creates /run/containerd as 0700. containerd keeps that mode instead of
/// its usual 0711, which stops pods in user namespaces from reaching their rootfs.
pub fn fix_bottlerocket_containerd_state_dir_mode() {
    set_containerd_state_dir_mode(&Path::new(BOTTLEROCKET_ROOTFS_PATH).join("run/containerd"));
}

fn set_containerd_state_dir_mode(path: &Path) {
    const REQUIRED_MODE: u32 = 0o711;
    let current_mode = match std::fs::metadata(path) {
        Ok(metadata) => metadata.permissions().mode() & 0o7777,
        Err(e) if e.kind() == ErrorKind::NotFound => return,
        Err(e) => {
            warn!("Failed to stat {}: {e}", path.display());
            return;
        }
    };
    // Only add permissions, in case a later version grants more.
    let new_mode = current_mode | REQUIRED_MODE;
    if new_mode == current_mode {
        return;
    }
    info!(
        "Changing mode of {} from {current_mode:o} to {new_mode:o}",
        path.display()
    );
    if let Err(e) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(new_mode)) {
        warn!("Failed to change mode of {}: {e}", path.display());
    }
}

pub async fn load_kube_config() -> kube::Config {
    let mut config = kube::Config::incluster().unwrap();

    config.connect_timeout = Some(Duration::from_secs(30));
    config.read_timeout = Some(Duration::from_secs(30));
    config.write_timeout = Some(Duration::from_secs(30));

    config
}

#[cfg(test)]
mod test {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::PathBuf;
    use std::sync::RwLock;

    use tempfile::TempDir;

    use crate::Commander;

    #[test]
    fn set_containerd_state_dir_mode() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = TempDir::with_prefix("ephemeral-storage-setup-test").unwrap();
        let path = temp_dir.path().join("containerd");
        // A missing directory is not an error.
        super::set_containerd_state_dir_mode(&path);
        std::fs::create_dir(&path).unwrap();
        // (initial mode, expected mode): permissions are added, never removed.
        for (initial_mode, expected_mode) in [(0o700, 0o711), (0o711, 0o711), (0o755, 0o755)] {
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(initial_mode)).unwrap();
            super::set_containerd_state_dir_mode(&path);
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
            assert_eq!(mode, expected_mode);
        }
    }

    /// Prevents spawning while a mock is open for writing. Otherwise a concurrently
    /// forked child can inherit the writable fd, causing ETXTBSY when the mock is exec'd.
    pub(crate) static SPAWN_LOCK: RwLock<()> = RwLock::new(());

    pub(crate) struct TestEnv {
        pub(crate) temp_dir: TempDir,
        pub(crate) commander: Commander,
    }

    impl TestEnv {
        pub(crate) fn new() -> Self {
            let temp_dir = TempDir::with_prefix("ephemeral-storage-setup-test").unwrap();
            let old_path = std::env::var("PATH").unwrap_or_default();
            let new_path = format!("{}:{}", temp_dir.path().to_string_lossy(), old_path);
            let mut commander = Commander::default();
            commander.envs.insert("PATH".to_owned(), new_path);
            commander
                .envs
                .insert("NODE_NAME".to_owned(), "test-node".to_owned());
            TestEnv {
                temp_dir,
                commander,
            }
        }

        pub(crate) fn mock(&self, command: &str, exit_code: u8, output: &str) {
            let _guard = SPAWN_LOCK.write().unwrap();
            let mut file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .create(true)
                .mode(0o755)
                .open(self.temp_dir.path().join(command))
                .unwrap();
            file.write_all(
                format!(
                    "#!/bin/bash
set -euo pipefail
cat <<'EOF'
{output}
EOF
exit {exit_code}
"
                )
                .as_bytes(),
            )
            .unwrap();
        }

        /// Reads test data file at path (relative to the root of the repo).
        pub(crate) fn read_testdata(&self, path: &str) -> String {
            std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap()
        }
    }
}
