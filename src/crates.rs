use crate::{api, commands::Args, Error};
use reqwest::header;
use serde::Deserialize;
use std::sync::Arc;
use tracing::info;

const USER_AGENT: &str = "rust-lang/discord-mods-bot";

#[derive(Debug, Deserialize)]
struct Crates {
    crates: Vec<Crate>,
}
#[derive(Debug, Deserialize)]
struct Crate {
    id: String,
    name: String,
    newest_version: String,
    max_stable_version: Option<String>,
    #[serde(rename = "updated_at")]
    updated: String,
    downloads: u64,
    #[serde(default)]
    description: String,
    documentation: Option<String>,
}

async fn get_crate(args: Arc<Args>) -> Result<Option<Crate>, Error> {
    let query = args
        .params
        .get("query")
        .ok_or("Unable to retrieve param: query")?;

    info!("searching for crate `{}`", query);

    let crate_list = args
        .http
        .get("https://crates.io/api/v1/crates")
        .header(header::USER_AGENT, USER_AGENT)
        .query(&[("q", query)])
        .send()
        .await?
        .json::<Crates>()
        .await?;

    Ok(crate_list.crates.into_iter().next())
}

pub async fn search(args: Arc<Args>) -> Result<(), Error> {
    if let Some(krate) = get_crate(args.clone()).await? {
        args.msg
            .channel_id
            .send_message(&args.cx, |m| {
                m.embed(|e| {
                    e.title(&krate.name)
                        .url(format!("https://crates.io/crates/{}", krate.id))
                        .description(&krate.description)
                        .field(
                            "version",
                            krate
                                .max_stable_version
                                .as_ref()
                                .unwrap_or(&krate.newest_version),
                            true,
                        )
                        .field("downloads", &krate.downloads, true)
                        .timestamp(krate.updated.as_str())
                });

                m
            })
            .await?;
    } else {
        let message = "No crates found.";
        api::send_reply(args.clone(), message).await?;
    }

    Ok(())
}

fn rustc_crate(crate_name: &str) -> Option<&str> {
    match crate_name {
        "std" => Some("https://doc.rust-lang.org/stable/std/"),
        "core" => Some("https://doc.rust-lang.org/stable/core/"),
        "alloc" => Some("https://doc.rust-lang.org/stable/alloc/"),
        "proc_macro" => Some("https://doc.rust-lang.org/stable/proc_macro/"),
        "beta" => Some("https://doc.rust-lang.org/beta/std/"),
        "nightly" => Some("https://doc.rust-lang.org/nightly/std/"),
        "rustc" => Some("https://doc.rust-lang.org/nightly/nightly-rustc/"),
        _ => None,
    }
}

pub async fn doc_search(args: Arc<Args>) -> Result<(), Error> {
    let query = args
        .params
        .get("query")
        .ok_or("Unable to retrieve param: query")?;

    let mut query_iter = query.splitn(2, "::");
    let crate_name = query_iter.next().unwrap();

    let doc_url = if let Some(rustc_crate) = rustc_crate(crate_name) {
        Some(rustc_crate.to_string())
    } else if let Some(krate) = get_crate(args.clone()).await? {
        let name = krate.name;
        krate
            .documentation
            .or_else(|| Some(format!("https://docs.rs/{}", name)))
    } else {
        None
    };

    if let Some(mut url) = doc_url {
        if let Some(item_path) = query_iter.next() {
            url += &format!("?search={}", item_path);
        }

        api::send_reply(args.clone(), &url).await?;
    } else {
        let message = "No crates found.";
        api::send_reply(args.clone(), message).await?;
    }

    Ok(())
}

/// Print the help message
pub async fn help(args: Arc<Args>) -> Result<(), Error> {
    let help_string = "search for a crate on crates.io
```
?crate query...
```";
    api::send_reply(args.clone(), &help_string).await?;
    Ok(())
}

/// Print the help message
pub async fn doc_help(args: Arc<Args>) -> Result<(), Error> {
    let help_string = "retrieve documentation for a given crate
```
?docs crate_name...
```";
    api::send_reply(args.clone(), &help_string).await?;
    Ok(())
}
