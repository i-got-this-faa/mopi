use anyhow::Result;
use gtk4::glib::MainContext;
use gtk4::prelude::*;
use gtk4::{
    Align, Application, ApplicationWindow, Box, Entry, Label, ListBox, Orientation, ScrolledWindow,
    SelectionMode,
};
use gtk4_layer_shell::{Layer, LayerShell};
use lss_config::{AppConfig, LssPaths};
use lss_ipc::{Request, RequestEnvelope, Response, ResponseEnvelope, read_frame, write_frame};
use lss_types::{MatchReason, QueryId, SearchResult};
use tokio::net::UnixStream;
use tokio::sync::watch;

#[derive(Debug, Clone)]
enum GuiEvent {
    ResultsUpdated(Vec<SearchResult>),
    Error(String),
}

fn main() -> Result<()> {
    lss_config::init_tracing();

    let paths = LssPaths::discover()?;
    let config = AppConfig::load_or_default(&paths)?;
    let socket_path = config.daemon.socket_path(&paths);

    let app = Application::builder()
        .application_id("com.github.lss.lssi")
        .build();

    let (gui_tx, gui_rx) = async_channel::unbounded();
    let (search_tx, mut search_rx) = watch::channel(String::new());

    // Background Tokio thread
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("Failed to build tokio runtime");

        rt.block_on(async move {
            let mut stream = match UnixStream::connect(socket_path.as_std_path()).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = gui_tx.try_send(GuiEvent::Error(format!(
                        "Failed to connect to lssd: {}",
                        e
                    )));
                    return;
                }
            };

            let mut last_query_id: Option<QueryId> = None;

            loop {
                tokio::select! {
                    changed = search_rx.changed() => {
                        if changed.is_err() {
                            break;
                        }
                    }
                    else => break,
                }

                let mut next_query = search_rx.borrow().clone();
                while search_rx.has_changed().unwrap_or(false) {
                    if search_rx.changed().await.is_err() {
                        return;
                    }
                    next_query = search_rx.borrow().clone();
                }

                // Cancel previous search if still active
                if let Some(prev_id) = last_query_id.take() {
                    let cancel_req = Request::CancelSearch { query_id: prev_id };
                    let _ = write_frame(&mut stream, &RequestEnvelope::new(cancel_req)).await;
                }

                let current_query_id = QueryId::new();
                last_query_id = Some(current_query_id);

                let query = lss_query::parse_query(next_query.clone());
                let req = Request::Search {
                    query_id: current_query_id,
                    query,
                };
                if let Err(e) = write_frame(&mut stream, &RequestEnvelope::new(req)).await {
                    let _ = gui_tx.try_send(GuiEvent::Error(format!("IPC write error: {}", e)));
                    break;
                }

                loop {
                    match read_frame::<ResponseEnvelope>(&mut stream).await {
                        Ok(env) => match env.response {
                            Response::SearchResultChunk { query_id, results, is_final } => {
                                if query_id == current_query_id {
                                    let _ = gui_tx.try_send(GuiEvent::ResultsUpdated(results));
                                    if is_final {
                                        break;
                                    }
                                }
                            }
                            Response::SearchResults { query_id, results } => {
                                if query_id == current_query_id {
                                    let _ = gui_tx.try_send(GuiEvent::ResultsUpdated(results));
                                    break;
                                }
                            }
                            Response::Error { message } => {
                                let _ = gui_tx.try_send(GuiEvent::Error(message));
                                break;
                            }
                            _ => {}
                        },
                        Err(e) => {
                            let _ =
                                gui_tx.try_send(GuiEvent::Error(format!("IPC read error: {}", e)));
                            return;
                        }
                    }
                }
            }
        });
    });

    app.connect_activate(move |app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Kiwi")
            .default_width(600)
            .default_height(400)
            .build();

        window.init_layer_shell();
        window.set_layer(Layer::Overlay);
        window.set_keyboard_mode(gtk4_layer_shell::KeyboardMode::Exclusive);

        // Center the window on screen
        window.set_margin(gtk4_layer_shell::Edge::Top, 100);
        // window.auto_exclusive_zone_enable();

        let vbox = Box::new(Orientation::Vertical, 0);
        window.set_child(Some(&vbox));

        let entry = Entry::builder()
            .placeholder_text("Search...")
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(8)
            .margin_end(8)
            .build();
        vbox.append(&entry);

        let scrolled = ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vexpand(true)
            .build();
        vbox.append(&scrolled);

        let list_box = ListBox::builder()
            .selection_mode(SelectionMode::Single)
            .build();
        scrolled.set_child(Some(&list_box));

        let window_clone_for_activation = window.clone();
        list_box.connect_row_activated(move |_, row| {
            if let Some(child) = row.child() {
                let path = child.widget_name();
                if let Err(e) = open::that(path.as_str()) {
                    eprintln!("Failed to open file: {}", e);
                } else {
                    window_clone_for_activation.close();
                }
            }
        });

        let search_tx_clone = search_tx.clone();
        entry.connect_changed(move |entry| {
            let text = entry.text().to_string();
            let _ = search_tx_clone.send(text);
        });

        // Close on escape
        let event_controller = gtk4::EventControllerKey::new();
        let window_clone = window.clone();
        event_controller.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape {
                window_clone.close();
                gtk4::glib::Propagation::Stop
            } else {
                gtk4::glib::Propagation::Proceed
            }
        });
        window.add_controller(event_controller);

        let list_box_clone = list_box.clone();

        let ctx = MainContext::default();
        let gui_rx = gui_rx.clone();
        ctx.spawn_local(async move {
            while let Ok(event) = gui_rx.recv().await {
                match event {
                    GuiEvent::ResultsUpdated(results) => {
                        while let Some(child) = list_box_clone.first_child() {
                            list_box_clone.remove(&child);
                        }
                        for result in results {
                            let row = create_result_row(&result);
                            list_box_clone.append(&row);
                        }
                    }
                    GuiEvent::Error(msg) => {
                        println!("Error: {}", msg);
                    }
                }
            }
        });

        // Trigger initial empty search
        let _ = search_tx.send(String::new());

        window.present();
    });

    app.run();

    Ok(())
}

fn create_result_row(result: &SearchResult) -> Box {
    let row = Box::new(Orientation::Vertical, 4);
    row.set_widget_name(result.path.as_str());
    row.set_margin_start(8);
    row.set_margin_end(8);
    row.set_margin_top(4);
    row.set_margin_bottom(4);

    let title_box = Box::new(Orientation::Horizontal, 8);

    let title = Label::builder()
        .label(&result.title)
        .halign(Align::Start)
        .css_classes(vec!["title".to_string()])
        .build();
    title.set_markup(&format!("<b>{}</b>", result.title));
    title_box.append(&title);

    let mut reasons = Vec::new();
    for reason in &result.reasons {
        let tag = match reason {
            MatchReason::Name => "name",
            MatchReason::Path => "path",
            MatchReason::Content => "content",
            MatchReason::Semantic => "semantic",
            MatchReason::Metadata => "metadata",
        };
        reasons.push(tag);
    }

    let reason_label = Label::builder()
        .label(reasons.join(", "))
        .halign(Align::End)
        .hexpand(true)
        .build();
    reason_label.set_markup(&format!("<small><i>{}</i></small>", reasons.join(", ")));
    title_box.append(&reason_label);

    row.append(&title_box);

    let path = Label::builder()
        .label(result.path.as_str())
        .halign(Align::Start)
        .build();
    path.set_markup(&format!("<small>{}</small>", result.path));
    row.append(&path);

    let snippet = Label::builder()
        .label(&result.snippet)
        .halign(Align::Start)
        .wrap(true)
        .build();
    snippet.set_markup(&result.snippet);
    row.append(&snippet);

    row
}
