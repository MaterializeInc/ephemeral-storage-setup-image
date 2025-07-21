use crate::Commander;

pub(crate) fn remove_taint(commander: Commander, node_name: &str, taint_key: &str) {
    println!("Removing taint {taint_key} from node {node_name}");
    let output = commander.unchecked_output(&[
        "kubectl",
        "taint",
        "nodes",
        node_name,
        &format!("{taint_key}-"),
    ]);
    if output.status.code() != Some(0) {
        let output = commander.check_output(&[
            "kubectl",
            "get",
            "node",
            node_name,
            "-o",
            "jsonpath={.spec.taints[*].key}",
        ]);
        if String::from_utf8_lossy(&output.stdout).contains(taint_key) {
            panic!("Error: Failed to remove taint and it still exists on the node.");
        }
    }
}
