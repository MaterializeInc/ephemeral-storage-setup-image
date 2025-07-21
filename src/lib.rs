use std::collections::HashMap;
use std::process::{Command, Output};

use clap::ValueEnum;

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
        Command::new(args[0])
            .args(&args[1..])
            .envs(&self.envs)
            .output()
            .expect(&failure_msg)
    }
}

#[cfg(test)]
mod test {
    use std::fs::OpenOptions;
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    use std::path::PathBuf;

    use tempfile::TempDir;

    use crate::Commander;

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
