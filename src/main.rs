use anyhow::Context;
use dioxus::prelude::*;
use regex::Regex;
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::env::var;
use std::fs;
use toml::Table;

#[derive(Clone, Debug)]
struct ModpackInfo {
    pub name: String,
    pub side: String,
    pub url: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

fn sort_game_versions(game_versions: &mut [String]) {
    game_versions.sort_by(|a, b| {
        let minor_a = a.split('.').nth(1).unwrap().parse::<u32>().unwrap();
        let minor_b = b.split('.').nth(1).unwrap().parse::<u32>().unwrap();
        let minor = minor_b.cmp(&minor_a);
        if minor == Ordering::Equal {
            let patch_a = a.split('.').nth(2).unwrap_or("0").parse::<u32>().unwrap();
            let patch_b = b.split('.').nth(2).unwrap_or("0").parse::<u32>().unwrap();
            return patch_b.cmp(&patch_a);
        }
        minor
    });
}

async fn get_modrinth_mods(mod_ids: Vec<String>) -> Result<Vec<ModpackInfo>, anyhow::Error> {
    let mut mods = Vec::new();
    if !mod_ids.is_empty() {
        let url = format!(
            "https://api.modrinth.com/v2/projects?ids=[\"{}\"]",
            mod_ids.join("\",\"")
        );
        let response = reqwest::get(url).await?.error_for_status()?;
        let data = response.json::<Vec<Value>>().await?;
        for item in data {
            let name = item.get("title").unwrap().as_str().unwrap().to_string();
            let slug = item.get("slug").unwrap().as_str().unwrap().to_string();
            let url = format!("https://modrinth.com/mod/{slug}");
            let side = match (
                item.get("client_side").unwrap().as_str(),
                item.get("server_side").unwrap().as_str(),
            ) {
                (Some("required"), Some("unsupported")) => "client".to_string(),
                (Some("optional"), Some("unsupported")) => "client".to_string(),
                (Some("unsupported"), Some("required")) => "server".to_string(),
                (Some("unsupported"), Some("optional")) => "server".to_string(),
                _ => "both".to_string(),
            };
            let mut loaders = item
                .get("loaders")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_string())
                .collect::<Vec<String>>();
            loaders.sort();

            let re = Regex::new(r"^1\.[0-9]+(\.[0-9]+)?$")?;
            let mut game_versions = item
                .get("game_versions")
                .unwrap()
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().to_string())
                .filter(|version| re.is_match(version))
                .collect::<Vec<String>>();
            sort_game_versions(&mut game_versions);
            mods.push(ModpackInfo {
                name,
                side,
                url,
                loaders,
                game_versions,
            })
        }
    }
    Ok(mods)
}

async fn get_curseforge_mods(mod_ids: Vec<i64>) -> Result<Vec<ModpackInfo>, anyhow::Error> {
    let mut mods = Vec::new();
    let forge_api_key = var("FORGE_API_KEY")?;
    if !mod_ids.is_empty() {
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.curseforge.com/v1/mods")
            .header("x-api-key", forge_api_key.as_str())
            .json(&json!({
                "modIds": mod_ids,
            }))
            .send()
            .await?
            .error_for_status()?;
        let response = response.json::<Value>().await?;
        let data = response
            .get("data")
            .context("can't find data")?
            .as_array()
            .context("couldn't get as array")?;
        for item in data {
            let name = item
                .get("name")
                .context("Can't find name")?
                .as_str()
                .context("Can't parse name as str")?
                .to_string();
            let url = item
                .get("links")
                .context("Can't find links")?
                .get("websiteUrl")
                .context("Can't find url")?
                .as_str()
                .context("Can't parse url as str")?
                .to_string();
            let latest_files_indexes = item
                .get("latestFilesIndexes")
                .context("Can't find latestFilesIndexes")?
                .as_array()
                .context("Can't get as array")?;
            let mut game_versions = HashSet::new();
            let mut mod_loaders = HashSet::new();
            for index in latest_files_indexes {
                let game_version = index
                    .get("gameVersion")
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string();
                game_versions.insert(game_version);
                if let Some(mod_loader) = index.get("modLoader") {
                    let mod_loader = mod_loader.as_i64();
                    let mod_loader = match mod_loader {
                        Some(0) => String::from("any"),
                        Some(1) => String::from("forge"),
                        Some(2) => String::from("cauldron"),
                        Some(3) => String::from("liteloader"),
                        Some(4) => String::from("fabric"),
                        Some(5) => String::from("quilt"),
                        Some(6) => String::from("neoforge"),
                        _ => String::from("unknown"),
                    };
                    mod_loaders.insert(mod_loader);
                }
            }
            let mut game_versions = game_versions.iter().cloned().collect::<Vec<String>>();
            sort_game_versions(&mut game_versions);
            let mut loaders = mod_loaders.iter().cloned().collect::<Vec<String>>();
            loaders.sort();
            let side = String::from("unknown");
            mods.push(ModpackInfo {
                name,
                side,
                url,
                loaders,
                game_versions,
            })
        }
    }
    Ok(mods)
}

async fn read_modpack() -> Result<Vec<ModpackInfo>, anyhow::Error> {
    let mod_files = fs::read_to_string("index.toml")?
        .parse::<Table>()?
        .get("files")
        .context("couldn't find files array")?
        .as_array()
        .context("couldn't parse into array")?
        .iter()
        .filter_map(|file| {
            if let (Some(name), Some(metafile)) = (
                file.get("file").and_then(|v| v.as_str()),
                file.get("metafile").and_then(|v| v.as_bool()),
            ) {
                if metafile {
                    return Some(name.to_string());
                }
            }
            None
        })
        .collect::<Vec<String>>();

    let mut mods = Vec::new();
    let mut mr_mods = Vec::new();
    let mut cf_mods = Vec::new();
    for file in mod_files {
        let mod_file = fs::read_to_string(file)?.parse::<Table>()?;

        if let Some(update_section) = mod_file.get("update") {
            match (
                update_section.get("curseforge"),
                update_section.get("modrinth"),
            ) {
                (Some(curseforge_section), None) => {
                    let mod_id = curseforge_section
                        .get("project-id")
                        .context("couldn't find project id")?
                        .as_integer()
                        .context("Can't parse project id as int")?;
                    cf_mods.push(mod_id);
                }
                (None, Some(modrinth_section)) => {
                    let mod_id = modrinth_section
                        .get("mod-id")
                        .context("couldn't find mod id")?
                        .as_str()
                        .context("Can't parse mod id as str")?
                        .to_string();
                    mr_mods.push(mod_id);
                }
                (_, _) => {
                    eprintln!("Unexpected update section without curseforge or modrinth")
                }
            }
        } else {
            // Hosted elsewhere
            let name = mod_file
                .get("name")
                .context("can't find name")?
                .as_str()
                .context("can't parse name to str")?
                .to_string();
            let side = mod_file
                .get("side")
                .context("can't find side")?
                .as_str()
                .context("can't parse side to str")?
                .to_string();
            let url = mod_file
                .get("download")
                .context("can't find download")?
                .get("url")
                .context("can't find url")?
                .as_str()
                .context("cant parse url as str")?
                .to_string();

            mods.push(ModpackInfo {
                name,
                side,
                url,
                game_versions: Vec::new(),
                loaders: Vec::new(),
            })
        }
    }

    let mut modrinth_mods = get_modrinth_mods(mr_mods).await?;
    mods.append(&mut modrinth_mods);
    let mut curseforge_mods = get_curseforge_mods(cf_mods).await?;
    mods.append(&mut curseforge_mods);

    mods.sort_by(|a, b| a.name.cmp(&b.name).then(a.side.cmp(&b.side)));
    Ok(mods)
}

pub fn get_prism_zips() -> Result<Vec<String>, anyhow::Error> {
    Ok(std::fs::read_dir(".")?
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            if name.starts_with("prism") && name.ends_with(".zip") {
                Some(name)
            } else {
                None
            }
        })
        .collect::<Vec<String>>())
}

fn zip_display() -> Result<Element, anyhow::Error> {
    let prism_zips = get_prism_zips()?;
    Ok(rsx! {
        ul {
            for zip in prism_zips {
                li {
                    a {
                        href: "{zip}",
                        "{zip}"
                    }
                }
            }
        }
    })
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mods = read_modpack().await?;
    let index = format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <link rel="icon" href="/favicon.ico">
    <link rel="apple-touch-icon" sizes="180x180" href="/apple-touch-icon.png">
    <link rel="icon" type="image/png" sizes="32x32" href="/favicon-32x32.png">
    <link rel="icon" type="image/png" sizes="16x16" href="/favicon-16x16.png">
    <link rel="manifest" href="/site.webmanifest">
    <title>Minecraft Modpack</title>
    <script>0</script>
</head>
{}
</html>"#,
        dioxus_ssr::render_element(rsx! {
            body {
                width: "100vw",
                height: "100vh",
                margin: "0",
                display: "flex",
                flex_direction: "column",
                font_family: "'Noto Sans', 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif",
                div {
                    margin: "20px",
                    div { "Modpack info" }
                    div { "Import the appropriate zip file into prism and packwiz will take care of the rest" }
                    div { "Files hosted " {zip_display()?} }
                    img {
                        max_height: "512px",
                        max_width: "100%",
                        height: "auto",
                        src: "/prism-import.png",
                        alt: "import prism instance"
                    }
                    div {
                        "To do this your self, download the packwiz bootstrap jar from "
                        a {
                            href: "https://github.com/packwiz/packwiz-installer-bootstrap/releases",
                            "Github Releases"
                        }
                        " and place it within the (.)minecraft directory of a newly created prism instance."
                    }
                    div {"Go to Edit Instance -> Settings -> Custom commands, then check the Custom Commands box and paste the following command into the pre-launch command field:"}
                    div {
                        font_family: "monospace",
                        font_size: "14px",
                        background:"#666",
                        padding:"10px",
                        "\"$INST_JAVA\" -jar packwiz-installer-bootstrap.jar https://packwiz.toyvo.dev/pack.toml"
                    }
                    img {
                        max_height: "512px",
                        max_width: "100%",
                        height: "auto",
                        src: "/prism-settings.png",
                        alt: "Setup packwiz"
                    }
                    div {
                        "Mods included: "
                        div {
                            display: "grid",
                            width: "100%",
                            grid_template_columns: "1fr 1fr auto auto",
                            gap: "12px",
                            div {
                                "Mod Name"
                            }
                            div {
                                "Side"
                            }
                            div {
                                "Minecraft Versions"
                            }
                            div {
                                "Loader"
                            }
                            for item in mods {
                                a {
                                    href: "{item.url}",
                                    "{item.name}"
                                }
                                div {
                                    "{item.side}"
                                }
                                div {
                                    display: "flex",
                                    flex_flow: "row wrap",
                                    align_content: "start",
                                    gap: "4px",
                                    for version in &item.game_versions {
                                       div {
                                            background: "grey",
                                            border_radius: "100px",
                                            padding: "4px",
                                            text_wrap: "nowrap",
                                            "{version}"
                                       }
                                    }
                                }
                                div {
                                    display: "flex",
                                    flex_flow: "row wrap",
                                    align_content: "start",
                                    gap: "4px",
                                    for loader in &item.loaders {
                                       div {
                                            background: "grey",
                                            border_radius: "100px",
                                            padding: "4px",
                                            text_wrap: "nowrap",
                                            "{loader}"
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
    );
    fs::write("index.html", index)?;
    Ok(())
}
