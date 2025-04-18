use std::sync::Arc;
use std::time::Duration;

use authelia_controller::VERSION;
use authelia_controller::context::Context;
use authelia_controller::resources::AccessControlRule;
use color_eyre::eyre::Context as _;
use dotenvy::dotenv;
use futures_util::{StreamExt as _, TryStreamExt as _};
use kube::runtime::reflector::{self};
use kube::runtime::{WatchStreamExt, watcher};
use kube::{Api, Client};
use tracing::{error, info};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Registry};

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    dotenv().ok();

    let env_filter = EnvFilter::try_from_default_env().or_else(|_| EnvFilter::try_new("info"))?;
    if std::env::var("CARGO").is_ok() {
        let logger = tracing_subscriber::fmt::layer().compact();
        Registry::default().with(logger).with(env_filter).init();
    } else {
        let logger = tracing_subscriber::fmt::layer().json();
        Registry::default().with(logger).with(env_filter).init();
    }

    let namespace = std::env::var("AUTHELIA_NAMESPACE").unwrap_or("authelia".into());
    let deployment = std::env::var("AUTHELIA_DEPLOYMENT").unwrap_or("authelia".into());
    let secret = std::env::var("AUTHELIA_SECRET").unwrap_or("authelia-acl".into());
    let interval = std::env::var("INTERVAL")
        .map(|interval| {
            interval
                .parse()
                .wrap_err_with(|| format!("INTERVAL={interval}"))
        })
        .unwrap_or(Ok(15))?;

    info!(version = VERSION, "Starting");

    let client = Client::try_default().await?;
    let access_control_rules = Api::<AccessControlRule>::all(client.clone());

    let (reader, writer) = reflector::store();

    let wc = watcher::Config::default().any_semantic();
    let mut stream = watcher(access_control_rules, wc)
        .default_backoff()
        .reflect(writer)
        .applied_objects()
        .boxed();

    let context = Arc::new(Context::new(
        client,
        "authelia.huizinga.dev",
        namespace,
        deployment,
        secret,
    ));

    tokio::spawn(async move {
        reader.wait_until_ready().await.unwrap();
        loop {
            if let Err(err) = AccessControlRule::update_acl(reader.state(), context.clone()).await {
                error!("Failed to update: {err}");
            }
            tokio::time::sleep(Duration::from_secs(interval)).await;
        }
    });

    while stream.try_next().await?.is_some() {}

    Ok(())
}
