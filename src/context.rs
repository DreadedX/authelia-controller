use kube::runtime::events::{Recorder, Reporter};

#[derive(Clone)]
pub struct Context {
    pub client: kube::Client,
    pub controller_name: String,
    pub namespace: String,
    pub deployment_name: String,
    pub secret_name: String,
    pub recorder: Recorder,
}

impl Context {
    pub fn new(
        client: kube::Client,
        controller_name: &str,
        namespace: impl Into<String>,
        deployment_name: impl Into<String>,
        secret_name: impl Into<String>,
    ) -> Self {
        let reporter: Reporter = controller_name.into();
        let recorder = Recorder::new(client.clone(), reporter);

        Self {
            client,
            controller_name: controller_name.into(),
            namespace: namespace.into(),
            deployment_name: deployment_name.into(),
            secret_name: secret_name.into(),
            recorder,
        }
    }
}
