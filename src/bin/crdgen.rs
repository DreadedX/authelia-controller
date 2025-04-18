use kube::CustomResourceExt;

fn main() {
    let resources =
        [
            serde_yaml::to_string(&authelia_controller::resources::AccessControlRule::crd())
                .unwrap(),
        ]
        .join("---\n");
    print!("{resources}")
}
