use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::cmp::Ordering;
use std::collections::HashSet;
use std::env::var;
use std::fs;
use toml::Table;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
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
        let re = Regex::new(r"^1\.[0-9]+(\.[0-9]+)?$")?;
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
    let forge_api_key = var("FORGE_API_KEY").unwrap_or_default();
    if !mod_ids.is_empty() && !forge_api_key.is_empty() {
        let client = reqwest::Client::new();
        let response = client
            .post("https://api.curseforge.com/v1/mods")
            .header("x-api-key", forge_api_key.as_str())
            .json(&json!({
                "modIds": mod_ids,
            }))
            .send()
            .await
            ?
            .error_for_status()
            ?;
        let response = response.json::<Value>().await?;
        let data = response.get("data").unwrap().as_array().unwrap();
        for item in data {
            let name = item.get("name").unwrap().as_str().unwrap().to_string();
            let url = item
                .get("links")
                .unwrap()
                .get("websiteUrl")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            let latest_files_indexes = item.get("latestFilesIndexes").unwrap().as_array().unwrap();
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
    } else {
        eprintln!("Skipping curseforge mods")
    }
    Ok(mods)
}

async fn read_modpack() -> Result<Vec<ModpackInfo>, anyhow::Error> {
    let mod_files = fs::read_to_string("modpack/index.toml")
        .unwrap()
        .parse::<Table>()
        .unwrap()
        .get("files")
        .unwrap()
        .as_array()
        .unwrap()
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
        let mod_file = fs::read_to_string(format!("modpack/{file}"))
            .unwrap()
            .parse::<Table>()
            .unwrap();

        if let Some(update_section) = mod_file.get("update") {
            match (
                update_section.get("curseforge"),
                update_section.get("modrinth"),
            ) {
                (Some(curseforge_section), None) => {
                    let mod_id = curseforge_section
                        .get("project-id")
                        .unwrap()
                        .as_integer()
                        .unwrap();
                    cf_mods.push(mod_id);
                }
                (None, Some(modrinth_section)) => {
                    let mod_id = modrinth_section
                        .get("mod-id")
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string();
                    mr_mods.push(mod_id);
                }
                (_, _) => {
                    eprintln!("Unexpected update section without curseforge or modrinth")
                }
            }
        } else {
            // Hosted elsewhere
            let name = mod_file.get("name").unwrap().as_str().unwrap().to_string();
            let side = mod_file.get("side").unwrap().as_str().unwrap().to_string();
            let url = mod_file
                .get("download")
                .unwrap()
                .get("url")
                .unwrap()
                .as_str()
                .unwrap()
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

pub fn get_prism_zips() -> Vec<String> {
    fs::read_dir("modpack")
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            if name.starts_with("prism") && name.ends_with(".zip") {
                Some(name)
            } else {
                None
            }
        })
        .collect::<Vec<String>>()
}

#[tokio::main]
async fn main() {
    println!("cargo::rerun-if-changed=modpack");

    match read_modpack().await {
        Ok(mods) => {
            let zips = get_prism_zips();
            let json = json!({
        "mods": mods,
        "zips": zips,
    });
            let json_string = serde_json::to_string(&json).unwrap();
            fs::write("assets/modpack_generated.json", json_string).unwrap();
        }
        Err(e) => {
            eprintln!("error fetching mods {e}");
        }
    }
}
