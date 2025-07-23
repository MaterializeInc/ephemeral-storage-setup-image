use k8s_openapi::api::core::v1::Node;
use kube::api::PostParams;
use kube::{Api, Client};

use crate::load_kube_config;

pub(crate) async fn remove_taint(node_name: &str, taint_key: &str) {
    let kube_config = load_kube_config().await;
    let client = Client::try_from(kube_config.clone()).unwrap();
    let node_api: Api<Node> = Api::all(client);

    for _ in 0..5 {
        let mut node = node_api
            .get(node_name)
            .await
            .expect(&format!("Failed to get node {node_name}"));
        let Some(taint_position) = taint_position(taint_key, &node) else {
            println!("Node {} is not tainted", node_name);
            return;
        };
        println!("Removing taint {taint_key} from node {node_name}");
        node.spec
            .as_mut()
            .unwrap()
            .taints
            .as_mut()
            .unwrap()
            .remove(taint_position);
        match node_api
            .replace(node_name, &PostParams::default(), &node)
            .await
        {
            Ok(_) => {
                println!("Taint {taint_key} removed");
                return;
            }
            Err(kube::Error::Api(e)) if e.code == 409 => {
                println!("Conflict while replacing node");
            }
            Err(e) => panic!("{}", e),
        };
    }
}

fn taint_position(taint_key: &str, node: &Node) -> Option<usize> {
    // Check if the node has the taint
    node.spec
        .as_ref()
        .and_then(|spec| spec.taints.as_ref())
        .and_then(|taints| taints.iter().position(|taint| taint.key == taint_key))
}
