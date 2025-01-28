use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ModInfo {
    pub name: String,
    pub side: String,
    pub url: String,
    pub game_versions: Vec<String>,
    pub loaders: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
struct ModPackInfo {
    pub mods: Vec<ModInfo>,
    pub zips: Vec<String>,
}

fn main() {
    dioxus::launch(App);
}

#[component]
fn App() -> Element {
    // Build cool things ✌️
    let info = use_resource(|| async move {
        let modpack_json = asset!("/assets/modpack.json");
        let url = format!("http://localhost:8080{}", modpack_json.resolve().to_str().unwrap());
        let response = reqwest::get(url).await.unwrap().error_for_status().unwrap();
        response.json::<ModPackInfo>().await.unwrap()
    });

    rsx! {
        // Global app resources
        document::Link { rel: "icon", href: asset!("/assets/favicon.ico") }
        document::Link { rel: "apple-touch-icon", sizes: "180x180", href: asset!("/assets/apple-touch-icon.png") }
        document::Link { rel: "icon", r#type: "image/png", sizes: "32x32", href: asset!("/assets/favicon-32x32.png") }
        document::Link { rel: "icon", r#type: "image/png", sizes: "16x16", href: asset!("/assets/favicon-16x16.png") }
        document::Link { rel: "manifest", href: asset!("/assets/site.webmanifest") }

        div {
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
                div { "Files hosted "
                    ul {
                        for zip in info.cloned().unwrap_or_default().zips {
                            li {
                                a {
                                    href: "{zip}",
                                    "{zip}"
                                }
                            }
                        }
                    }
                }
                img {
                    max_height: "512px",
                    max_width: "100%",
                    height: "auto",
                    src: asset!("/assets/prism-import.png"),
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
                    src: asset!("/assets/prism-settings.png"),
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
                        for item in info.cloned().unwrap_or_default().mods {
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
    }
}
