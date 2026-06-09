#![allow(clippy::collapsible_if)]
use anyhow::Result;
use lss_config::{AppConfig, LssPaths};
use lss_ipc::{Request, RequestEnvelope, Response, ResponseEnvelope, read_frame, write_frame};
use lss_types::QueryId;
use std::fs;
use tokio::net::UnixStream;

#[tokio::main]
async fn main() -> Result<()> {
    lss_config::init_tracing();

    let paths = LssPaths::discover()?;
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("init-config") => {
            AppConfig::write_default(&paths)?;
            println!("wrote default config to {}", paths.config_file);
        }
        Some("config") => match args.get(2).map(String::as_str) {
            Some("show") => {
                if paths.config_file.exists() {
                    print!("{}", fs::read_to_string(&paths.config_file)?);
                } else {
                    print!("{}", AppConfig::default_template());
                }
            }
            Some("validate") => {
                let config = AppConfig::load_or_default(&paths)?;
                AppConfig::validate(&config)?;
                println!("config is valid");
            }
            _ => print_usage(&paths),
        },
        Some("ping") => {
            let response = send_request(&paths, Request::Ping).await?;
            print_response(response);
        }
        Some("status") => {
            let response = send_request(&paths, Request::GetStatus).await?;
            print_response(response);
        }
        Some("stats") => {
            let response = send_request(&paths, Request::GetStats).await?;
            print_response(response);
        }
        Some("roots") => match args.get(2).map(String::as_str) {
            Some("add") => {
                if let Some(new_root) = args.get(3) {
                    add_root(&paths, new_root).await?;
                    let response = send_request(&paths, Request::ReloadConfig).await?;
                    println!("Added {} and sent reload request:", new_root);
                    print_response(response);
                } else {
                    println!("usage: lssctl roots add <path>");
                }
            }
            Some("remove") => {
                if let Some(target_root) = args.get(3) {
                    remove_root(&paths, target_root).await?;
                    let response = send_request(&paths, Request::ReloadConfig).await?;
                    println!("Removed {} and sent reload request:", target_root);
                    print_response(response);
                } else {
                    println!("usage: lssctl roots remove <path>");
                }
            }
            _ => {
                let response = send_request(&paths, Request::ListRoots).await?;
                print_response(response);
            }
        },
        Some("reload-config") => {
            let response = send_request(&paths, Request::ReloadConfig).await?;
            print_response(response);
        }
        Some("refresh") => {
            let response = send_request(&paths, Request::RefreshChanged).await?;
            print_response(response);
        }
        Some("failures") => {
            let response = send_request(&paths, Request::GetFailures { limit: 50 }).await?;
            print_response(response);
        }
        Some("doctor") => {
            let response = send_request(&paths, Request::Doctor).await?;
            print_response(response);
        }
        Some("search") => {
            let query_str = args.get(2..).unwrap_or(&[]).join(" ");
            let query = lss_query::parse_query(query_str);
            let config = AppConfig::load_or_default(&paths)?;
            let socket_path = config.daemon.socket_path(&paths);
            let mut stream = UnixStream::connect(socket_path.as_std_path()).await?;
            write_frame(
                &mut stream,
                &RequestEnvelope::new(Request::Search {
                    query_id: QueryId::new(),
                    query,
                }),
            )
            .await?;
            // Read streaming frames until final
            loop {
                let envelope: ResponseEnvelope = read_frame(&mut stream).await?;
                match envelope.response {
                    Response::SearchResultChunk { results, is_final, .. } => {
                        for r in &results {
                            println!("{:.3}  {}  {}",
                                r.score,
                                r.path,
                                r.snippet.chars().take(120).collect::<String>()
                            );
                        }
                        if is_final {
                            break;
                        }
                    }
                    other => {
                        print_response(other);
                        break;
                    }
                }
            }
        }
        Some("grep") => {
            let mut iter = args.iter().skip(2).peekable();
            let mut case_sensitive = true;
            let mut limit = 100usize;
            let mut filetype: Option<String> = None;

            while let Some(arg) = iter.next_if(|a| a.starts_with('-')) {
                match arg.as_str() {
                    "-i" => case_sensitive = false,
                    "-n" => {} // line numbers are always included
                    l if l.starts_with("--limit=") => {
                        if let Ok(n) = l.trim_start_matches("--limit=").parse() {
                            limit = n;
                        }
                    }
                    l if l.starts_with("--filetype=") => {
                        filetype = Some(l.trim_start_matches("--filetype=").to_string());
                    }
                    _ => {}
                }
            }

            let pattern: String = iter.cloned().collect::<Vec<_>>().join(" ");
            if pattern.is_empty() {
                println!("usage: lssctl grep [OPTIONS] <pattern>");
                return Ok(());
            }

            let config = AppConfig::load_or_default(&paths)?;
            let socket_path = config.daemon.socket_path(&paths);
            let mut stream = UnixStream::connect(socket_path.as_std_path()).await?;
            write_frame(
                &mut stream,
                &RequestEnvelope::new(Request::GrepQuery {
                    query_id: QueryId::new(),
                    pattern,
                    case_sensitive,
                    limit,
                    filetype_filters: filetype.map(|f| vec![f]).unwrap_or_default(),
                }),
            )
            .await?;

            loop {
                let envelope: ResponseEnvelope = read_frame(&mut stream).await?;
                match envelope.response {
                    Response::GrepChunk { matches, is_final, .. } => {
                        for m in &matches {
                            println!("{}:{}:{}", m.path, m.line_number, m.line);
                        }
                        if is_final {
                            break;
                        }
                    }
                    other => {
                        print_response(other);
                        break;
                    }
                }
            }
        }
        _ => print_usage(&paths),
    }

    Ok(())
}

async fn send_request(paths: &LssPaths, request: Request) -> Result<Response> {
    let config = AppConfig::load_or_default(paths)?;
    let socket_path = config.daemon.socket_path(paths);
    let mut stream = UnixStream::connect(socket_path.as_std_path()).await?;
    write_frame(&mut stream, &RequestEnvelope::new(request)).await?;
    let response: ResponseEnvelope = read_frame(&mut stream).await?;
    Ok(response.response)
}

fn print_usage(paths: &LssPaths) {
    println!(
        "usage: lssctl <init-config|config show|config validate|ping|status|stats|roots|reload-config|refresh|failures|doctor|search ...>\nconfig={}\nsocket={}",
        paths.config_file,
        paths.socket_file()
    );
}

fn print_response(response: Response) {
    println!("{response:#?}");
}

async fn add_root(paths: &LssPaths, new_path: &str) -> Result<()> {
    if !paths.config_file.exists() {
        AppConfig::write_default(paths)?;
    }

    let content = fs::read_to_string(&paths.config_file)?;
    let mut doc: toml_edit::DocumentMut = content.parse()?;

    if !doc.contains_key("roots") {
        doc["roots"] = toml_edit::Item::ArrayOfTables(toml_edit::ArrayOfTables::new());
    }

    if let Some(roots) = doc["roots"].as_array_of_tables_mut() {
        // check if it already exists
        for table in roots.iter() {
            if let Some(path) = table.get("path") {
                if path.as_str() == Some(new_path) {
                    return Ok(());
                }
            }
        }

        let mut new_table = toml_edit::Table::new();
        new_table.insert("path", toml_edit::value(new_path));
        roots.push(new_table);
    }

    fs::write(&paths.config_file, doc.to_string())?;
    Ok(())
}

async fn remove_root(paths: &LssPaths, target_path: &str) -> Result<()> {
    if !paths.config_file.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&paths.config_file)?;
    let mut doc: toml_edit::DocumentMut = content.parse()?;

    if let Some(roots) = doc
        .get_mut("roots")
        .and_then(|r| r.as_array_of_tables_mut())
    {
        let mut index_to_remove = None;
        for (i, table) in roots.iter().enumerate() {
            if let Some(path) = table.get("path") {
                if path.as_str() == Some(target_path) {
                    index_to_remove = Some(i);
                    break;
                }
            }
        }

        if let Some(i) = index_to_remove {
            roots.remove(i);
        }
    }

    fs::write(&paths.config_file, doc.to_string())?;
    Ok(())
}
