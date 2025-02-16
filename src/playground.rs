//! run rust code on the rust-lang playground

use crate::{api, commands::Args, Error};
use reqwest::header;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tracing::info;

const MAX_OUTPUT_LINES: usize = 45;

#[derive(Debug, Serialize)]
struct PlaygroundCode {
    channel: Channel,
    edition: Edition,
    code: String,
    #[serde(rename = "crateType")]
    crate_type: CrateType,
    mode: Mode,
    tests: bool,
}

impl PlaygroundCode {
    fn new(code: String) -> Self {
        PlaygroundCode {
            channel: Channel::Nightly,
            edition: Edition::E2018,
            code,
            crate_type: CrateType::Binary,
            mode: Mode::Debug,
            tests: false,
        }
    }

    fn url_from_gist(&self, gist: &str) -> String {
        let version = match self.channel {
            Channel::Nightly => "nightly",
            Channel::Beta => "beta",
            Channel::Stable => "stable",
        };

        let edition = match self.edition {
            Edition::E2015 => "2015",
            Edition::E2018 => "2018",
            Edition::E2021 => "2021",
            Edition::E2024 => "2024",
        };

        let mode = match self.mode {
            Mode::Debug => "debug",
            Mode::Release => "release",
        };

        format!(
            "https://play.rust-lang.org/?version={}&mode={}&edition={}&gist={}",
            version, mode, edition, gist
        )
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

impl FromStr for Channel {
    type Err = Box<dyn std::error::Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "stable" => Ok(Channel::Stable),
            "beta" => Ok(Channel::Beta),
            "nightly" => Ok(Channel::Nightly),
            _ => Err(format!("invalid release channel `{}`", s).into()),
        }
    }
}

#[derive(Debug, Serialize)]
enum Edition {
    #[serde(rename = "2015")]
    E2015,
    #[serde(rename = "2018")]
    E2018,
    #[serde(rename = "2021")]
    E2021,
    #[serde(rename = "2024")]
    E2024,
}

impl FromStr for Edition {
    type Err = Box<dyn std::error::Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "2015" => Ok(Edition::E2015),
            "2018" => Ok(Edition::E2018),
            "2021" => Ok(Edition::E2021),
            "2024" => Ok(Edition::E2024),
            _ => Err(format!("invalid edition `{}`", s).into()),
        }
    }
}

#[derive(Debug, Serialize)]
enum CrateType {
    #[serde(rename = "bin")]
    Binary,
    #[serde(rename = "lib")]
    Library,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "snake_case")]
enum Mode {
    Debug,
    Release,
}

impl FromStr for Mode {
    type Err = Box<dyn std::error::Error + Send + Sync>;

    fn from_str(s: &str) -> Result<Self, Error> {
        match s {
            "debug" => Ok(Mode::Debug),
            "release" => Ok(Mode::Release),
            _ => Err(format!("invalid compilation mode `{}`", s).into()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct PlayResult {
    success: bool,
    stdout: String,
    stderr: String,
}

async fn run_code(args: Arc<Args>, code: String) -> Result<String, Error> {
    let mut errors = String::new();

    let warnings = args.params.get("warn").map(|s| &s[..]).unwrap_or("false");
    let channel = args
        .params
        .get("channel")
        .map(|s| &s[..])
        .unwrap_or("nightly");
    let mode = args.params.get("mode").map(|s| &s[..]).unwrap_or("debug");
    let edition = args.params.get("edition").map(|s| &s[..]).unwrap_or("2024");

    let mut request = PlaygroundCode::new(code.clone());

    match Channel::from_str(&channel) {
        Ok(c) => request.channel = c,
        Err(e) => errors += &format!("{}\n", e),
    }

    match Mode::from_str(&mode) {
        Ok(m) => request.mode = m,
        Err(e) => errors += &format!("{}\n", e),
    }

    match Edition::from_str(&edition) {
        Ok(e) => request.edition = e,
        Err(e) => errors += &format!("{}\n", e),
    }

    if !code.contains("fn main") {
        request.crate_type = CrateType::Library;
    }

    let message = "*Running code on playground...*";
    api::send_reply(args.clone(), message).await?;

    let resp = args
        .http
        .post("https://play.rust-lang.org/execute")
        .json(&request)
        .send()
        .await?;

    let result: PlayResult = resp.json().await?;

    let result = if warnings == "true" {
        format!("{}\n{}", result.stderr, result.stdout)
    } else if result.success {
        result.stdout
    } else {
        result.stderr
    };

    let lines = result.lines().count();

    Ok(
        if result.len() + errors.len() > 1993 || lines > MAX_OUTPUT_LINES {
            format!(
                "{}Output too large. Playground link: {}",
                errors,
                get_playground_link(args, code, request).await?
            )
        } else if result.len() == 0 {
            format!("{}compilation succeeded.", errors)
        } else {
            format!("{}```\n{}```", errors, result)
        },
    )
}

async fn get_playground_link(
    args: Arc<Args>,
    code: String,
    request: PlaygroundCode,
) -> Result<String, Error> {
    let mut payload = HashMap::new();
    payload.insert("code", code);

    let resp = args
        .http
        .post("https://play.rust-lang.org/meta/gist/")
        .header(header::REFERER, "https://discord.gg/rust-lang")
        .json(&payload)
        .send()
        .await?;

    let resp: HashMap<String, String> = resp.json().await?;
    info!("gist response: {:?}", resp);

    resp.get("id")
        .map(|id| request.url_from_gist(id))
        .ok_or_else(|| "no gist found".into())
}

pub async fn run(args: Arc<Args>) -> Result<(), Error> {
    let code = args
        .params
        .get("code")
        .map(String::from)
        .ok_or("Unable to retrieve param: query")?;

    let result = run_code(args.clone(), code).await?;
    api::send_reply(args.clone(), &result).await?;
    Ok(())
}

pub async fn help(args: Arc<Args>, name: &str) -> Result<(), Error> {
    let message = format!(
        "Compile and run rust code. All code is executed on https://play.rust-lang.org.
```?{} mode={{}} channel={{}} edition={{}} warn={{}} ``\u{200B}`code``\u{200B}` ```
Optional arguments:
    \tmode: debug, release (default: debug)
    \tchannel: stable, beta, nightly (default: nightly)
    \tedition: 2015, 2018, 2021, 2024 (default: 2024)
    \twarn: boolean flag to enable compilation warnings
    ",
        name
    );

    api::send_reply(args.clone(), &message).await?;
    Ok(())
}

pub async fn err(args: Arc<Args>) -> Result<(), Error> {
    let message = "Missing code block. Please use the following markdown:
\\`\\`\\`rust
    code here
\\`\\`\\`
    ";

    api::send_reply(args.clone(), message).await?;
    Ok(())
}

pub async fn eval(args: Arc<Args>) -> Result<(), Error> {
    let code = args
        .params
        .get("code")
        .map(String::from)
        .ok_or("Unable to retrieve param: query")?;

    if code.contains("fn main") {
        api::send_reply(
            args.clone(),
            "code passed to ?eval should not contain `fn main`",
        )
        .await?;
    } else {
        let code = format!("fn main(){{ println!(\"{{:?}}\",{{ {} \n}}); }}", code);

        let result = run_code(args.clone(), code).await?;
        api::send_reply(args.clone(), &result).await?;
    }

    Ok(())
}

pub async fn eval_err(args: Arc<Args>) -> Result<(), Error> {
    let message = "Missing code block. Please use the following markdown:
    \\`code here\\`
    or
    \\`\\`\\`rust
        code here
    \\`\\`\\`
    ";

    api::send_reply(args.clone(), message).await?;
    Ok(())
}
