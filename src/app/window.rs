use crate::app::config::AppConfig;
use crate::backend::device;
use crate::backend::image;
use crate::backend::types::*;
use crate::ui::config_panel::ConfigPanel;
use crate::ui::device_list::DeviceListView;
use crate::ui::dialogs;
use crate::ui::log_panel::LogPanel;
use gtk4::gio;
use gtk4::prelude::*;
use libadwaita::prelude::AdwApplicationWindowExt;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

#[allow(dead_code)]
pub struct MainWindow {
    pub window: libadwaita::ApplicationWindow,
    config: Rc<RefCell<AppConfig>>,
    device_list: Rc<DeviceListView>,
    config_panel: Rc<ConfigPanel>,
    log_panel: Rc<LogPanel>,
    status_label: gtk4::Label,
    tools_dir: PathBuf,
    pub start_button: gtk4::Button,
    pub stop_button: gtk4::Button,
    image_path_label: gtk4::Label,
    device_stack: gtk4::Stack,
    partition_names: Rc<RefCell<Vec<String>>>,
    partition_listbox: gtk4::ListBox,
    burning_pct: gtk4::Label,
    status_progress: gtk4::ProgressBar,
}

impl MainWindow {
    pub fn new(app: &libadwaita::Application) -> Self {
        let config = Rc::new(RefCell::new(AppConfig::load()));
        let tools_dir = find_tools_dir();
        let app_handle = app.clone();

        let device_list = Rc::new(DeviceListView::new());
        let config_panel = Rc::new(ConfigPanel::new());
        let log_panel = Rc::new(LogPanel::new());

        // --- Widgets for bottom bar ---
        let image_path_label = gtk4::Label::builder()
            .label("No image loaded")
            .halign(gtk4::Align::Start)
            .hexpand(true)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .build();

        let start_button = gtk4::Button::builder()
            .label("Start")
            .css_classes(["suggested-action"])
            .build();

        let stop_button = gtk4::Button::builder()
            .label("Stop")
            .sensitive(false)
            .build();

        let status_label = gtk4::Label::builder()
            .label("Ready")
            .halign(gtk4::Align::Start)
            .build();

        // --- Header bar ---
        let header = libadwaita::HeaderBar::new();

        let file_menu = gio::Menu::new();
        file_menu.append(Some("Import Image"), Some("win.import_image"));

        let recent_menu = gio::Menu::new();
        file_menu.append_submenu(Some("Recent Images"), &recent_menu);
        file_menu.append(Some("Refresh Devices"), Some("win.refresh_devices"));

        let rebuild_recent: Rc<dyn Fn()> = {
            let menu = recent_menu.clone();
            let cfg = config.clone();
            Rc::new(move || {
                menu.remove_all();
                let c = cfg.borrow();
                if c.recent_images.is_empty() {
                    menu.append_item(&gio::MenuItem::new(Some("(none)"), None));
                } else {
                    for (i, path) in c.recent_images.iter().enumerate() {
                        let a = format!("win.recent_{i}");
                        menu.append_item(&gio::MenuItem::new(Some(path), Some(&a)));
                    }
                }
            })
        };
        rebuild_recent();

        let file_button = gtk4::MenuButton::builder()
            .label("File")
            .menu_model(&file_menu)
            .build();
        file_button.set_direction(gtk4::ArrowType::None);
        file_button.add_css_class("header-menu-button");
        header.pack_start(&file_button);

        // --- Advanced Settings menu ---
        let advanced_menu = gio::Menu::new();
        advanced_menu.append(Some("Password File..."), Some("win.password_file"));
        advanced_menu.append(Some("eFuse File..."), Some("win.efuse_file"));
        advanced_menu.append(Some("Save Log..."), Some("win.save_log"));
        advanced_menu.append(Some("Clear Log"), Some("win.clear_log"));
        advanced_menu.append(Some("Partitions..."), Some("win.partitions"));
        advanced_menu.append(Some("About"), Some("win.about"));

        let advanced_button = gtk4::MenuButton::builder()
            .label("Advanced Settings")
            .menu_model(&advanced_menu)
            .build();
        advanced_button.set_direction(gtk4::ArrowType::None);
        advanced_button.add_css_class("header-menu-button");
        header.pack_start(&advanced_button);

        let partition_popover = gtk4::Popover::builder().build();
        partition_popover.set_parent(&advanced_button);
        let partition_listbox = gtk4::ListBox::builder()
            .css_classes(["partitions-list"])
            .build();
        let partition_popover_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .margin_bottom(6)
            .margin_top(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        partition_popover_box.append(
            &gtk4::Label::builder()
                .label("Select partitions to burn")
                .halign(gtk4::Align::Start)
                .css_classes(["heading"])
                .build(),
        );
        partition_popover_box.append(&partition_listbox);
        partition_popover.set_child(Some(&partition_popover_box));

        // --- Window creation ---
        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title("Amlogic USB Burning Tool")
            .default_width(config.borrow().window_width.max(800))
            .default_height(config.borrow().window_height.max(600))
            .icon_name("aml-flash-tool")
            .build();

        // --- Device stack ---
        let empty_page = libadwaita::StatusPage::builder()
            .title("No Device Connected")
            .description("Connect an Amlogic device in USB burning mode")
            .icon_name("drive-harddisk-symbolic")
            .build();
        let device_stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .build();
        device_stack.add_titled(&empty_page, Some("empty"), "No Device");
        device_stack.add_titled(&device_list.column_view, Some("devices"), "Devices");
        device_stack.set_visible_child_name("empty");

        // --- Partition names ---
        let partition_names: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

        // --- Rebuild partitions function ---
        let rebuild_partitions: Rc<dyn Fn(Vec<String>)> = {
            let listbox = partition_listbox.clone();
            let names = partition_names.clone();
            Rc::new(move |parts| {
                while let Some(child) = listbox.first_child() {
                    listbox.remove(&child);
                }
                *names.borrow_mut() = parts.clone();
                for name in &parts {
                    let check = gtk4::CheckButton::builder()
                        .label(name.as_str())
                        .active(true)
                        .build();
                    listbox.append(&check);
                }
            })
        };

        // --- Bottom bar widgets ---
        let status_progress = gtk4::ProgressBar::builder()
            .width_request(120)
            .valign(gtk4::Align::Center)
            .build();

        let burning_pct = gtk4::Label::builder().css_classes(["burning-pct"]).build();
        burning_pct.set_visible(false);

        // --- Main layout ---
        let main_content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();

        let middle_box = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .position(650)
            .build();
        middle_box.set_start_child(Some(&device_stack));

        let config_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .build();
        config_scroll.set_child(Some(&config_panel.page));
        middle_box.set_end_child(Some(&config_scroll));

        main_content.append(&middle_box);

        let separator = gtk4::Separator::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .build();
        main_content.append(&separator);

        let log_scroll = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .build();
        log_scroll.set_child(Some(&log_panel.text_view));
        main_content.append(&log_scroll);

        // --- Bottom bar (imagebar + statusbar) ---
        let bottom_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .css_classes(["imagebar"])
            .spacing(8)
            .build();
        bottom_box.append(&image_path_label);
        bottom_box.append(&start_button);
        bottom_box.append(&stop_button);

        let bottom_status = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .css_classes(["statusbar"])
            .spacing(6)
            .build();
        bottom_status.append(&status_label);
        bottom_status.append(&status_progress);
        bottom_status.append(&burning_pct);

        let bottom_area = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        bottom_area.append(&bottom_box);
        bottom_area.append(&bottom_status);

        main_content.append(&bottom_area);

        let toolbar_view = libadwaita::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&main_content));
        window.set_content(Some(&toolbar_view));

        // ---- Shared image loading ----
        let load_image_path: Rc<dyn Fn(PathBuf)> = {
            let log = log_panel.clone();
            let panel = config_panel.clone();
            let label = image_path_label.clone();
            let cfg = config.clone();
            let tools = tools_dir.clone();
            let irebuild = rebuild_recent.clone();
            let irebuild_parts = rebuild_partitions.clone();
            let sl = status_label.clone();
            let pb = status_progress.clone();
            let pct = burning_pct.clone();
            let sb = start_button.clone();

            Rc::new(move |path: PathBuf| {
                let path_str = path.to_string_lossy().to_string();
                label.set_text(&path_str);
                panel.set_image_path(&path_str);
                log.append_log(&format!("Loading image: {path_str}"));
                sl.set_text("Loading image...");
                pb.set_fraction(0.0);
                pct.set_text("0%");
                pb.set_visible(true);
                pct.set_visible(true);
                sb.set_sensitive(false);

                cfg.borrow_mut().add_recent_image(&path_str);
                cfg.borrow().save();
                irebuild();

                let log_c = log.clone();
                let panel_c = panel.clone();
                let tools_c = tools.clone();
                let irebuild_parts_c = irebuild_parts.clone();
                let sl_prog = sl.clone();
                let sl_done = sl.clone();
                let pb_prog = pb.clone();
                let pb_done = pb.clone();
                let pct_prog = pct.clone();
                let pct_done = pct.clone();
                let sb_c = sb.clone();
                image::unpack_image_async(
                    path,
                    tools_c,
                    move |result| {
                        match result {
                            Ok(info) => {
                                log_c.append_log(&format!(
                                    "Loaded {} partitions from image",
                                    info.partitions.len()
                                ));
                                sl_done.set_text(&format!(
                                    "Image loaded: {} partitions",
                                    info.partitions.len()
                                ));
                                irebuild_parts_c(
                                    info.partitions.iter().map(|p| p.name.clone()).collect(),
                                );
                                panel_c.set_soc(&info.soc);
                            }
                            Err(e) => {
                                log_c.append_log(&format!("ERROR: {e}"));
                                sl_done.set_text("Image load failed");
                            }
                        }
                        sb_c.set_sensitive(true);
                        pb_done.set_fraction(1.0);
                        pct_done.set_text("100%");
                        pb_done.set_visible(false);
                        pct_done.set_visible(false);
                    },
                    move |item, name| {
                        let frac = (item as f64 * 0.05).min(0.95);
                        pb_prog.set_fraction(frac);
                        pct_prog.set_text(&format!("{:.0}%", frac * 100.0));
                        sl_prog.set_text(&format!("Loading image... [{item}] {name}"));
                    },
                );
            })
        };

        // --- Import action ---
        let import_action = gio::SimpleAction::new("import_image", None);
        let import_win = window.clone();
        let import_log = log_panel.clone();
        let import_loader = load_image_path.clone();

        import_action.connect_activate(move |_, _| {
            import_log.append_log("Opening file chooser...");
            let loader = import_loader.clone();
            dialogs::show_image_chooser(&import_win, move |opt| {
                if let Some(path) = opt {
                    loader(path);
                }
            });
        });
        window.add_action(&import_action);
        app_handle.set_accels_for_action("win.import_image", &["<Ctrl>O"]);

        // --- Refresh Devices action ---
        let refresh_dl = device_list.clone();
        let refresh_lp = log_panel.clone();
        let refresh_tl = tools_dir.clone();
        let refresh_sl = status_label.clone();
        let refresh_st = device_stack.clone();

        let refresh_action = gio::SimpleAction::new("refresh_devices", None);
        refresh_action.connect_activate(move |_, _| {
            refresh_lp.append_log("Refreshing devices...");
            let dl = refresh_dl.clone();
            let lp = refresh_lp.clone();
            let tl = refresh_tl.clone();
            let sl = refresh_sl.clone();
            let st = refresh_st.clone();
            device::scan_devices_background(tl, move |devices| {
                dl.update_devices(&devices);
                let count = dl.device_count();
                if count > 0 {
                    st.set_visible_child_name("devices");
                } else {
                    st.set_visible_child_name("empty");
                }
                sl.set_text(&format!("{count} device(s) detected"));
                lp.append_log(&format!("Refresh complete — {count} device(s)"));
            });
        });
        window.add_action(&refresh_action);
        app_handle.set_accels_for_action("win.refresh_devices", &["F5", "<Ctrl>r"]);

        // --- Drag and drop image loading ---
        let drop_target = gtk4::DropTarget::builder()
            .actions(gtk4::gdk::DragAction::COPY)
            .formats(&gtk4::gdk::ContentFormats::new(&["text/uri-list"]))
            .build();

        {
            let loader = load_image_path.clone();
            drop_target.connect_drop(move |_target, value, _x, _y| {
                if let Ok(text) = value.get::<String>() {
                    for line in text.lines() {
                        if let Some(path) = uri_to_path(line.trim()) {
                            if path.exists() {
                                loader(path);
                                return true;
                            }
                        }
                    }
                }
                false
            });
        }

        window.add_controller(drop_target);

        // --- Password File action ---
        let pw_win = window.clone();
        let pw_panel = config_panel.clone();
        let pw_log = log_panel.clone();
        let password_action = gio::SimpleAction::new("password_file", None);
        password_action.connect_activate(move |_, _| {
            let panel = pw_panel.clone();
            let plog = pw_log.clone();
            dialogs::show_file_dialog(&pw_win, "Select Password File", "Unlock", move |opt| {
                if let Some(path) = opt {
                    let s = path.to_string_lossy().to_string();
                    panel.set_password_path(&s);
                    plog.append_log(&format!("Password file: {s}"));
                }
            });
        });
        window.add_action(&password_action);

        // --- eFuse File action ---
        let ef_win = window.clone();
        let ef_panel = config_panel.clone();
        let ef_log = log_panel.clone();
        let efuse_action = gio::SimpleAction::new("efuse_file", None);
        efuse_action.connect_activate(move |_, _| {
            let panel = ef_panel.clone();
            let elog = ef_log.clone();
            dialogs::show_file_dialog(&ef_win, "Select eFuse File", "Open", move |opt| {
                if let Some(path) = opt {
                    let s = path.to_string_lossy().to_string();
                    panel.set_efuse_path(&s);
                    elog.append_log(&format!("eFuse file: {s}"));
                }
            });
        });
        window.add_action(&efuse_action);

        // --- Save Log action ---
        let sv_win = window.clone();
        let sv_log = log_panel.clone();
        let save_log_action = gio::SimpleAction::new("save_log", None);
        save_log_action.connect_activate(move |_, _| {
            let log = sv_log.clone();
            let filename = format!("burn-log-{}.txt", chrono::Local::now().format("%Y%m%d_%H%M%S"));
            dialogs::show_save_dialog(
                &sv_win,
                &filename,
                move |opt| {
                    if let Some(path) = opt {
                        let text = {
                            let buffer = log.text_view.buffer();
                            buffer.text(&buffer.start_iter(), &buffer.end_iter(), false)
                        };
                        match std::fs::write(&path, &text) {
                            Ok(_) => {
                                log.set_log_file(path);
                                log.append_log("Continuous logging started — all new entries will be written to file");
                            }
                            Err(e) => log.append_log(&format!("ERROR saving log: {e}")),
                        }
                    }
                },
            );
        });
        window.add_action(&save_log_action);

        // --- Clear Log action ---
        let clear_log = log_panel.clone();
        let clear_log_action = gio::SimpleAction::new("clear_log", None);
        clear_log_action.connect_activate(move |_, _| {
            clear_log.clear_log_file();
            clear_log.clear();
        });
        window.add_action(&clear_log_action);

        // --- Keyboard shortcuts: Start / Stop actions ---
        let start_action = gio::SimpleAction::new("start_burn", None);
        let sa_btn = start_button.clone();
        start_action.connect_activate(move |_, _| {
            sa_btn.activate();
        });
        window.add_action(&start_action);

        let stop_action = gio::SimpleAction::new("stop_burn", None);
        let so_btn = stop_button.clone();
        stop_action.connect_activate(move |_, _| {
            so_btn.activate();
        });
        window.add_action(&stop_action);

        app_handle.set_accels_for_action("win.start_burn", &["<Ctrl>Return", "<Ctrl>s"]);
        app_handle
            .set_accels_for_action("win.stop_burn", &["<Shift><Ctrl>Return", "<Shift><Ctrl>s"]);

        // --- Partitions action ---
        let partition_action = gio::SimpleAction::new("partitions", None);
        let pa_popover = partition_popover.clone();
        let pa_names = partition_names.clone();
        let pa_win = window.clone();
        partition_action.connect_activate(move |_, _| {
            if pa_names.borrow().is_empty() {
                dialogs::show_message_dialog(
                    &pa_win,
                    "No Image Loaded",
                    "Please load a firmware image first before selecting partitions.",
                );
            } else {
                pa_popover.popup();
            }
        });
        window.add_action(&partition_action);

        // --- About action ---
        let about_action = gio::SimpleAction::new("about", None);
        let about_win = window.clone();
        about_action.connect_activate(move |_, _| {
            let build_ts = include_str!(concat!(env!("OUT_DIR"), "/build_timestamp"))
                .trim()
                .parse::<u64>()
                .unwrap_or(0);
            let build_time = chrono::DateTime::from_timestamp(build_ts as i64, 0)
                .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                .unwrap_or_else(|| "unknown".into());
            let version = format!("{} (built {})", env!("CARGO_PKG_VERSION"), build_time);
            let about = gtk4::AboutDialog::builder()
                .transient_for(&about_win)
                .program_name("Amlogic USB Burning Tool")
                .version(&version)
                .comments("Firmware flashing tool for Amlogic devices")
                .website("https://github.com/antoxa78/Amlogic-Tool-for-Linux")
                .website_label("GitHub")
                .license_type(gtk4::License::MitX11)
                .build();
            if let Ok(pixbuf) = gtk4::gdk_pixbuf::Pixbuf::from_resource(
                "/com/amlflashtool/gui/icons/256x256/apps/aml-flash-tool.png",
            ) {
                if let Some(scaled) =
                    pixbuf.scale_simple(128, 128, gtk4::gdk_pixbuf::InterpType::Bilinear)
                {
                    let texture = gtk4::gdk::Texture::for_pixbuf(&scaled);
                    about.set_logo(Some(&texture));
                }
            }
            about.present();
        });
        window.add_action(&about_action);

        // --- Recent image actions (win.recent_0 .. win.recent_9) ---
        for i in 0..10u32 {
            let recent_cfg = config.clone();
            let recent_panel = config_panel.clone();
            let recent_log = log_panel.clone();
            let recent_tools = tools_dir.clone();
            let recent_label = image_path_label.clone();
            let recent_status = status_label.clone();
            let recent_pb = status_progress.clone();
            let recent_pct = burning_pct.clone();
            let recent_sb = start_button.clone();
            let rrebuild = rebuild_recent.clone();
            let rrebuild_parts = rebuild_partitions.clone();
            let action = gio::SimpleAction::new(&format!("recent_{i}"), None);
            action.connect_activate(move |_, _| {
                let path = {
                    let cfg = recent_cfg.borrow();
                    cfg.recent_images.get(i as usize).cloned()
                };
                if let Some(path_str) = path {
                    recent_label.set_text(&path_str);
                    recent_panel.set_image_path(&path_str);
                    recent_log.append_log(&format!("Loading recent: {path_str}"));
                    recent_status.set_text("Loading image...");
                    recent_pb.set_fraction(0.0);
                    recent_pct.set_text("0%");
                    recent_pb.set_visible(true);
                    recent_pct.set_visible(true);
                    recent_sb.set_sensitive(false);

                    recent_cfg.borrow_mut().add_recent_image(&path_str);
                    recent_cfg.borrow().save();
                    rrebuild();
                    let rrebuild_parts = rrebuild_parts.clone();

                    let log_c = recent_log.clone();
                    let panel_c = recent_panel.clone();
                    let sl_prog = recent_status.clone();
                    let sl_done = recent_status.clone();
                    let pb_prog = recent_pb.clone();
                    let pb_done = recent_pb.clone();
                    let pct_prog = recent_pct.clone();
                    let pct_done = recent_pct.clone();
                    let sb_c = recent_sb.clone();
                    image::unpack_image_async(
                        std::path::PathBuf::from(&path_str),
                        recent_tools.clone(),
                        move |result| {
                            match result {
                                Ok(info) => {
                                    log_c.append_log(&format!(
                                        "Loaded {} partitions",
                                        info.partitions.len()
                                    ));
                                    sl_done.set_text(&format!(
                                        "Image loaded: {} partitions",
                                        info.partitions.len()
                                    ));
                                    rrebuild_parts(
                                        info.partitions.iter().map(|p| p.name.clone()).collect(),
                                    );
                                    panel_c.set_soc(&info.soc);
                                }
                                Err(e) => {
                                    log_c.append_log(&format!("ERROR: {e}"));
                                    sl_done.set_text("Image load failed");
                                }
                            }
                            sb_c.set_sensitive(true);
                            pb_done.set_fraction(1.0);
                            pct_done.set_text("100%");
                            pb_done.set_visible(false);
                            pct_done.set_visible(false);
                        },
                        move |item, name| {
                            let frac = (item as f64 * 0.05).min(0.95);
                            pb_prog.set_fraction(frac);
                            pct_prog.set_text(&format!("{:.0}%", frac * 100.0));
                            sl_prog.set_text(&format!("Loading image... [{item}] {name}"));
                        },
                    );
                }
            });
            window.add_action(&action);
        }

        // --- Restore last-used options ---
        {
            let cfg = config.borrow();
            config_panel.set_soc(&cfg.last_soc);
            config_panel
                .erase_flash_switch
                .set_active(cfg.last_erase_flash);
            config_panel
                .erase_bootloader_switch
                .set_active(cfg.last_erase_bootloader);
            config_panel
                .reset_switch
                .set_active(cfg.last_reset_after_burn);
            if !cfg.last_password_path.is_empty() {
                config_panel.set_password_path(&cfg.last_password_path);
            }
            if !cfg.last_efuse_path.is_empty() {
                config_panel.set_efuse_path(&cfg.last_efuse_path);
            }
        }

        // --- Save config on window close ---
        let close_cfg = config.clone();
        let close_panel = config_panel.clone();
        let close_win = window.clone();
        window.connect_close_request(move |_| {
            let mut cfg = close_cfg.borrow_mut();
            cfg.window_width = close_win.width();
            cfg.window_height = close_win.height();
            cfg.last_erase_flash = close_panel.erase_flash_switch.is_active();
            cfg.last_erase_bootloader = close_panel.erase_bootloader_switch.is_active();
            cfg.last_reset_after_burn = close_panel.reset_switch.is_active();
            cfg.last_soc = close_panel.soc_raw();
            cfg.last_password_path = close_panel.password_path();
            cfg.last_efuse_path = close_panel.efuse_path();
            cfg.save();
            glib::Propagation::Proceed
        });

        let mw = Self {
            window,
            config,
            device_list: device_list.clone(),
            config_panel,
            log_panel: log_panel.clone(),
            status_label: status_label.clone(),
            tools_dir,
            start_button,
            stop_button,
            image_path_label,
            device_stack,
            partition_names,
            partition_listbox,
            burning_pct: burning_pct.clone(),
            status_progress: status_progress.clone(),
        };

        mw.setup_device_polling();
        mw.connect_signals();

        mw
    }

    fn setup_device_polling(&self) {
        let device_list = self.device_list.clone();
        let log_panel = self.log_panel.clone();
        let tools_dir = self.tools_dir.clone();
        let status_label = self.status_label.clone();
        let stack = self.device_stack.clone();
        let last_scan: Rc<RefCell<Option<std::time::Instant>>> = Rc::new(RefCell::new(None));

        glib::timeout_add_local(std::time::Duration::from_secs(2), move || {
            let now = std::time::Instant::now();
            if let Some(t) = *last_scan.borrow() {
                if now.duration_since(t) < std::time::Duration::from_secs(10) {
                    return glib::ControlFlow::Continue;
                }
            }
            *last_scan.borrow_mut() = Some(now);

            let dl = device_list.clone();
            let lp = log_panel.clone();
            let tl = tools_dir.clone();
            let sl = status_label.clone();
            let st = stack.clone();

            let prev_count = dl.device_count();
            device::scan_devices_background(tl, move |devices| {
                dl.update_devices(&devices);
                let count = dl.device_count();

                if count > 0 {
                    st.set_visible_child_name("devices");
                } else {
                    st.set_visible_child_name("empty");
                }

                if count > prev_count {
                    lp.append_log(&format!("Device connected — {count} device(s) detected"));
                    sl.set_text(&format!("{count} device(s) detected"));
                } else if count == 0 && prev_count > 0 {
                    sl.set_text("No device detected");
                }
            });

            glib::ControlFlow::Continue
        });
    }

    fn connect_signals(&self) {
        let win = self.window.clone();
        let log = self.log_panel.clone();
        let device_list = self.device_list.clone();
        let start_btn = self.start_button.clone();
        let stop_btn = self.stop_button.clone();
        let tools_dir = self.tools_dir.clone();
        let status_label = self.status_label.clone();
        let config_panel = self.config_panel.clone();
        let partition_names = self.partition_names.clone();
        let partition_listbox = self.partition_listbox.clone();
        let burning_pct = self.burning_pct.clone();
        let status_progress = self.status_progress.clone();

        self.start_button.connect_clicked(move |_| {
            // Build burn config from UI
            let mut cfg = BurnConfig::default();
            let img_path = config_panel.image_path();
            if img_path.is_empty() {
                log.append_log("ERROR: No image loaded");
                return;
            }
            cfg.image_path = Some(PathBuf::from(&img_path));
            cfg.wipe = config_panel.erase_flash_switch.is_active();
            let erase_bootloader = config_panel.erase_bootloader_switch.is_active();
            cfg.erase_bootloader = erase_bootloader;
            cfg.reset = config_panel.reset_switch.is_active();
            cfg.soc = config_panel.soc();
            let pw = config_panel.password_path();
            if !pw.is_empty() {
                cfg.password_path = Some(PathBuf::from(&pw));
            }
            let ef = config_panel.efuse_path();
            if !ef.is_empty() {
                cfg.efuse_path = Some(PathBuf::from(&ef));
            }
            let selected: Vec<String> = {
                let mut result = Vec::new();
                let mut child = partition_listbox.first_child();
                while let Some(widget) = child {
                    // ListBox auto-wraps appended widgets in ListBoxRow,
                    // so we must unwrap the row before reaching the CheckButton.
                    if let Some(row) = widget.downcast_ref::<gtk4::ListBoxRow>() {
                        if let Some(inner) = row.child() {
                            if let Some(check) = inner.downcast_ref::<gtk4::CheckButton>() {
                                if check.is_active() {
                                    result.push(check.label().unwrap_or_default().to_string());
                                }
                            }
                        }
                    }
                    child = widget.next_sibling();
                }
                result
            };
            let total_parts = partition_names.borrow().len();
            if selected.is_empty() {
                cfg.parts = "none".into();
            } else if selected.len() == total_parts {
                cfg.parts = "all".into();
            } else {
                cfg.parts = selected.join(",");
            }

            let adnl_path = tools_dir.join("adnl").join("adnl");
            start_btn.set_sensitive(false);
            let (adnl_tx, adnl_rx) = std::sync::mpsc::channel::<bool>();
            if adnl_path.exists() {
                std::thread::spawn(move || {
                    let has_adnl = std::process::Command::new("timeout")
                        .arg("5")
                        .arg(&adnl_path)
                        .arg("devices")
                        .output()
                        .ok()
                        .map(|out| {
                            String::from_utf8_lossy(&out.stdout).lines().any(|l| {
                                let trimmed = l.trim();
                                !trimmed.is_empty() && trimmed != "No DNL devices found"
                            })
                        })
                        .unwrap_or(false);
                    let _ = adnl_tx.send(has_adnl);
                });
            } else {
                let _ = adnl_tx.send(false);
            }

            let win_c = win.clone();
            let log_c = log.clone();
            let device_list_c = device_list.clone();
            let start_btn_c = start_btn.clone();
            let stop_btn_c = stop_btn.clone();
            let status_label_c = status_label.clone();
            let burning_pct_c = burning_pct.clone();
            let status_progress_c = status_progress.clone();
            let tools_dir_c = tools_dir.clone();

            glib::idle_add_local(move || -> glib::ControlFlow {
                let has_adnl = match adnl_rx.try_recv() {
                    Ok(v) => v,
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        return glib::ControlFlow::Continue;
                    }
                    Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                        start_btn_c.set_sensitive(true);
                        return glib::ControlFlow::Break;
                    }
                };

                if has_adnl {
                    log_c.append_log("ADNL device detected, using adnl_flash_tool.sh");
                    cfg.adnl = true;
                }

                if !cfg.adnl {
                    const SUPPORTED_SOCS: &[&str] = &["m8", "gxl", "axg", "txlx", "g12a"];
                    if !SUPPORTED_SOCS.contains(&cfg.soc.as_str()) {
                        log_c.append_log(&format!(
                            "ERROR: SoC '{}' is not supported by the classic burn engine (supported: m8, gxl, axg, txlx, g12a). Connect the device in ADNL mode for this SoC.",
                            cfg.soc
                        ));
                        status_label_c.set_text("Burn aborted: unsupported SoC");
                        start_btn_c.set_sensitive(true);
                        return glib::ControlFlow::Break;
                    }
                }

                let log_i = log_c.clone();
                let device_list_i = device_list_c.clone();
                let start_btn_i = start_btn_c.clone();
                let stop_btn_i = stop_btn_c.clone();
                let status_label_i = status_label_c.clone();
                let burning_pct_i = burning_pct_c.clone();
                let status_progress_i = status_progress_c.clone();
                let tools_dir_i = tools_dir_c.clone();
                let cfg_i = cfg.clone();

                let start_logic: Rc<RefCell<Option<Box<dyn FnMut()>>>> =
                    Rc::new(RefCell::new(None));
                *start_logic.borrow_mut() = Some(Box::new(move || {
                    let devices_to_burn: Vec<Rc<RefCell<DeviceState>>> = {
                        let devices = device_list_i.devices.borrow();
                        devices
                            .iter()
                            .filter(|d| {
                                matches!(d.borrow().status, BurnStatus::Ready)
                            })
                            .cloned()
                            .collect()
                    };

                    if devices_to_burn.is_empty() {
                        log_i.append_log("No ready devices");
                        start_btn_i.set_sensitive(true);
                        return;
                    }

                    let script_dir = tools_dir_i
                        .parent()
                        .map(|p| p.to_path_buf())
                        .unwrap_or_else(|| tools_dir_i.clone());

                    let count = devices_to_burn.len();
                    log_i.append_log(&format!("Starting burn on {count} device(s)"));
                    status_label_i.set_text(&format!("Burning {count} device(s)..."));
                    burning_pct_i.set_text("0%");
                    burning_pct_i.set_visible(true);
                    status_progress_i.set_fraction(0.0);
                    status_progress_i.set_visible(true);
                    start_btn_i.set_sensitive(false);
                    stop_btn_i.set_sensitive(true);

                    for device in devices_to_burn {
                        let (cancel_tx, cancel_rx) = std::sync::mpsc::channel::<()>();
                        {
                            let mut dev = device.borrow_mut();
                            dev.status = BurnStatus::Burning(0.0);
                            dev.cancel_tx = Some(cancel_tx);
                        }
                        device_list_i.refresh_display();

                        let cfg = cfg_i.clone();
                        let script_dir = script_dir.clone();
                        let log_c = log_i.clone();
                        let dl = device_list_i.clone();
                        let dev_clone = device.clone();
                        let start_c = start_btn_i.clone();
                        let stop_c = stop_btn_i.clone();
                        let status_c = status_label_i.clone();
                        let bp_c = burning_pct_i.clone();
                        let sp_c = status_progress_i.clone();

                        glib::MainContext::default().spawn_local(async move {
                            log_c.append_log(&format!(
                                "[{}] Starting burn...",
                                dev_clone.borrow().hub_id
                            ));

                            let result =
                                crate::backend::burner::BurnProcess::start(&cfg, &script_dir);
                            let mut burner = match result {
                                Ok(b) => b,
                                Err(e) => {
                                    log_c.append_log(&format!(
                                        "[{}] Failed to start: {e}",
                                        dev_clone.borrow().hub_id
                                    ));
                                    dev_clone.borrow_mut().status = BurnStatus::Error(e);
                                    dl.refresh_display();
                                    finish_burn_check(&dl, &bp_c, &sp_c, &start_c, &stop_c, &status_c);
                                    return;
                                }
                            };

                            let mut cancelled = false;
                            let mut success = false;
                            let mut exit_code = None;

                            loop {
                                if cancel_rx.try_recv().is_ok() {
                                    cancelled = true;
                                    log_c.append_log(&format!(
                                        "[{}] Cancelled by user",
                                        dev_clone.borrow().hub_id
                                    ));
                                    burner.kill();
                                    break;
                                }

                                match burner.read_line() {
                                    Some(crate::backend::burner::BurnEvent::Line(line, is_err)) => {
                                        if !line.trim().is_empty() {
                                            let tag = if is_err { " [STDERR]" } else { "" };
                                            log_c.append_log(&format!(
                                                "[{}]{tag} {line}",
                                                dev_clone.borrow().hub_id
                                            ));
                                        }
                                        if let Some(pct) =
                                            crate::backend::progress::parse_progress(&line)
                                        {
                                            dev_clone.borrow_mut().status =
                                                BurnStatus::Burning(pct / 100.0);
                                            dl.refresh_display();
                                            let devices = dl.devices.borrow();
                                            let burning: Vec<f64> = devices.iter().filter_map(|d| {
                                                if let BurnStatus::Burning(p) = d.borrow().status { Some(p) } else { None }
                                            }).collect();
                                            let avg = if burning.is_empty() { 0.0 } else { burning.iter().sum::<f64>() / burning.len() as f64 };
                                            bp_c.set_text(&format!("{:.0}%", avg * 100.0));
                                            sp_c.set_fraction(avg);
                                        }
                                        if line.contains("[OK]") {
                                            success = true;
                                        }
                                        if line.contains("[KO]") {
                                            success = false;
                                        }
                                    }
                                    Some(crate::backend::burner::BurnEvent::Exited(code)) => {
                                        exit_code = Some(code);
                                        // Drain any remaining lines
                                    }
                                    Some(crate::backend::burner::BurnEvent::Error(e)) => {
                                        log_c.append_log(&format!(
                                            "[{}] Error: {e}",
                                            dev_clone.borrow().hub_id
                                        ));
                                        break;
                                    }
                                    None => {
                                        if exit_code.is_some() {
                                            break;
                                        }
                                        glib::timeout_future(
                                            std::time::Duration::from_millis(50),
                                        )
                                        .await;
                                    }
                                }
                            }

                            if cancelled {
                                dev_clone.borrow_mut().status =
                                    BurnStatus::Error("Cancelled".into());
                            } else {
                                let code = exit_code.unwrap_or(-1);
                                if code == 0 && success {
                                    dev_clone.borrow_mut().status = BurnStatus::Success;
                                    log_c.append_log(&format!(
                                        "[{}] Completed!",
                                        dev_clone.borrow().hub_id
                                    ));
                                } else if code == 0 && !success {
                                    dev_clone.borrow_mut().status =
                                        BurnStatus::Error("No [OK] marker".into());
                                    log_c.append_log(&format!(
                                        "[{}] Process succeeded but no [OK] marker found",
                                        dev_clone.borrow().hub_id
                                    ));
                                } else {
                                    dev_clone.borrow_mut().status =
                                        BurnStatus::Error(format!("Exit code: {code}"));
                                    log_c.append_log(&format!(
                                        "[{}] Finished (exit: {code})",
                                        dev_clone.borrow().hub_id
                                    ));
                                }
                            }

                            dev_clone.borrow_mut().cancel_tx = None;
                            dl.refresh_display();

                            finish_burn_check(&dl, &bp_c, &sp_c, &start_c, &stop_c, &status_c);
                        });
                    }
                }));

                if erase_bootloader {
                    let start_c = start_logic.clone();
                    let cancel_btn = start_btn_c.clone();
                    dialogs::show_confirm_dialog(
                        &win_c,
                        "Erase Bootloader",
                        "Are you sure? Erasing the bootloader may brick the device if interrupted.",
                        move |confirmed| {
                            if confirmed {
                                let mut guard = start_c.borrow_mut();
                                if let Some(ref mut f) = *guard {
                                    f();
                                }
                            } else {
                                cancel_btn.set_sensitive(true);
                            }
                        },
                    );
                } else {
                    let mut guard = start_logic.borrow_mut();
                    if let Some(ref mut f) = *guard {
                        f();
                    }
                }

                glib::ControlFlow::Break
            });
        });

        let log = self.log_panel.clone();
        let device_list = self.device_list.clone();
        let start_btn = self.start_button.clone();
        let stop_btn = self.stop_button.clone();
        let status_label = self.status_label.clone();

        self.stop_button.connect_clicked(move |_| {
            let devices = device_list.devices.borrow();
            let mut any_cancelled = false;
            for d in devices.iter() {
                let mut dev = d.borrow_mut();
                if let Some(tx) = dev.cancel_tx.take() {
                    let _ = tx.send(());
                    any_cancelled = true;
                    log.append_log(&format!("[{}] Sending stop...", dev.hub_id));
                }
            }
            if !any_cancelled {
                log.append_log("No running processes to stop");
            } else {
                status_label.set_text("Stopping all devices...");
                start_btn.set_sensitive(false);
                stop_btn.set_sensitive(false);
            }
        });
    }
}

fn finish_burn_check(
    dl: &Rc<DeviceListView>,
    bp_c: &gtk4::Label,
    sp_c: &gtk4::ProgressBar,
    start_c: &gtk4::Button,
    stop_c: &gtk4::Button,
    status_c: &gtk4::Label,
) {
    let any_burning = dl
        .devices
        .borrow()
        .iter()
        .any(|d| matches!(d.borrow().status, BurnStatus::Burning(_)));
    if !any_burning {
        bp_c.set_visible(false);
        sp_c.set_visible(false);
        start_c.set_sensitive(true);
        stop_c.set_sensitive(false);
        let total = dl.device_count();
        let ok_count = dl
            .devices
            .borrow()
            .iter()
            .filter(|d| matches!(d.borrow().status, BurnStatus::Success))
            .count();
        let err_count = dl
            .devices
            .borrow()
            .iter()
            .filter(|d| matches!(d.borrow().status, BurnStatus::Error(_)))
            .count();
        status_c.set_text(&format!(
            "Done — {ok_count} OK, {err_count} Fail / {total} total"
        ));
    }
}

fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost/").unwrap_or(rest);
    Some(PathBuf::from(percent_decode(rest)))
}

fn percent_decode(s: &str) -> String {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn find_tools_dir() -> PathBuf {
    let system_tools = PathBuf::from("/usr/lib/amlogicburn/tools");
    if system_tools.exists() {
        return system_tools;
    }

    let exe = std::env::current_exe().ok();
    if let Some(exe_path) = exe {
        let dir = exe_path.parent().unwrap();
        let tools_candidate = dir.join("tools");
        if tools_candidate.exists() {
            return tools_candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools")
}
