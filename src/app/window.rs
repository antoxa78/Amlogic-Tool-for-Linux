use crate::app::config::AppConfig;
use crate::backend::burner::{BurnEvent, BurnProcess};
use crate::backend::device;
use crate::backend::image;
use crate::backend::types::*;
use crate::ui::config_panel::ConfigPanel;
use crate::ui::device_list::DeviceListView;
use crate::ui::dialogs;
use crate::ui::log_panel::LogPanel;
use gtk4::gio;
use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

/// SoCs the classic (non-ADNL) `update`-based burn engine can handle.
const CLASSIC_SOCS: &[&str] = &["m8", "gxl", "axg", "txlx", "g12a"];
const APP_NAME: &str = "Amlogic USB Burning Tool";
/// Width of the settings panel on the right when the window opens.
const SETTINGS_WIDTH: i32 = 330;

pub struct MainWindow {
    pub window: libadwaita::ApplicationWindow,
    inner: Rc<Inner>,
}

struct Inner {
    app: libadwaita::Application,
    window: libadwaita::ApplicationWindow,
    config: RefCell<AppConfig>,
    tools_dir: PathBuf,

    device_list: Rc<DeviceListView>,
    config_panel: Rc<ConfigPanel>,
    log: Rc<LogPanel>,

    toasts: libadwaita::ToastOverlay,
    title: libadwaita::WindowTitle,
    device_stack: gtk4::Stack,
    recent_menu: gio::Menu,
    open_button: libadwaita::SplitButton,

    image_name: gtk4::Label,
    image_detail: gtk4::Label,
    status_label: gtk4::Label,
    progress: gtk4::ProgressBar,
    start_button: gtk4::Button,
    stop_button: gtk4::Button,

    loading: Cell<bool>,
    burning: Cell<bool>,
    scanning: Cell<bool>,
    force_close: Cell<bool>,
}

impl MainWindow {
    pub fn new(app: &libadwaita::Application) -> Self {
        let mut config = AppConfig::load();
        // Never open larger than the screen (e.g. 1366×768 laptops):
        // leave room for panels/taskbars.
        if let Some((mw, mh)) = primary_monitor_size() {
            config.window_width = config.window_width.min(mw - 40).max(720);
            config.window_height = config.window_height.min(mh - 90).max(520);
        }
        let tools_dir = find_tools_dir();

        let device_list = Rc::new(DeviceListView::new());
        let config_panel = ConfigPanel::new();
        let log = Rc::new(LogPanel::new());

        // ── Window ───────────────────────────────────────────────────────
        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title(APP_NAME)
            .default_width(config.window_width)
            .default_height(config.window_height)
            .width_request(720)
            .height_request(520)
            .icon_name("aml-flash-tool")
            .build();
        if config.window_maximized {
            window.maximize();
        }

        // ── Header bar ───────────────────────────────────────────────────
        let title = libadwaita::WindowTitle::new(APP_NAME, "No device connected");
        let header = libadwaita::HeaderBar::builder().title_widget(&title).build();

        let recent_menu = gio::Menu::new();
        let open_button = libadwaita::SplitButton::builder()
            .child(
                &libadwaita::ButtonContent::builder()
                    .icon_name("document-open-symbolic")
                    .label("Open Image")
                    .build(),
            )
            .menu_model(&recent_menu)
            .dropdown_tooltip("Recent images")
            .tooltip_text("Open a firmware image (Ctrl+O)")
            .action_name("win.import_image")
            .build();
        header.pack_start(&open_button);

        let refresh_button = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Refresh devices (F5)")
            .action_name("win.refresh_devices")
            .build();
        header.pack_start(&refresh_button);

        let main_menu = gio::Menu::new();
        let log_section = gio::Menu::new();
        log_section.append(Some("Save Log…"), Some("win.save_log"));
        log_section.append(Some("Clear Log"), Some("win.clear_log"));
        main_menu.append_section(None, &log_section);
        let app_section = gio::Menu::new();
        app_section.append(Some("Keyboard Shortcuts"), Some("win.shortcuts"));
        app_section.append(Some("About Amlogic USB Burning Tool"), Some("win.about"));
        main_menu.append_section(None, &app_section);
        let menu_button = gtk4::MenuButton::builder()
            .icon_name("open-menu-symbolic")
            .menu_model(&main_menu)
            .primary(true)
            .tooltip_text("Main menu")
            .build();
        header.pack_end(&menu_button);

        // ── Device area ──────────────────────────────────────────────────
        // Compact empty state: must fit in ~half of a 768px-high screen.
        let empty_page = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .valign(gtk4::Align::Center)
            .halign(gtk4::Align::Center)
            .margin_top(12)
            .margin_bottom(12)
            .margin_start(24)
            .margin_end(24)
            .css_classes(["empty-devices"])
            .build();
        empty_page.append(
            &gtk4::Image::builder()
                .icon_name("drive-removable-media-symbolic")
                .pixel_size(48)
                .css_classes(["dim-label"])
                .margin_bottom(4)
                .build(),
        );
        empty_page.append(
            &gtk4::Label::builder()
                .label("No Device Connected")
                .css_classes(["title-3"])
                .build(),
        );
        empty_page.append(
            &gtk4::Label::builder()
                .label("Put the device into USB burning mode and connect it — it is detected automatically.")
                .wrap(true)
                .justify(gtk4::Justification::Center)
                .max_width_chars(48)
                .build(),
        );
        empty_page.append(
            &gtk4::Label::builder()
                .label("Not showing up? Check the udev rule, or hold the reset button while plugging in.")
                .wrap(true)
                .justify(gtk4::Justification::Center)
                .max_width_chars(52)
                .css_classes(["caption", "dim-label"])
                .build(),
        );
        empty_page.append(
            &gtk4::Button::builder()
                .label("Scan Again")
                .halign(gtk4::Align::Center)
                .margin_top(6)
                .action_name("win.refresh_devices")
                .css_classes(["pill", "small-pill"])
                .build(),
        );

        let device_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&device_list.column_view)
            .build();
        let device_card = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(["card", "device-card"])
            .margin_top(12)
            .margin_bottom(6)
            .margin_start(12)
            .margin_end(6)
            .build();
        device_card.append(&device_scroll);

        let device_stack = gtk4::Stack::builder()
            .transition_type(gtk4::StackTransitionType::Crossfade)
            .hexpand(true)
            .build();
        device_stack.add_named(&empty_page, Some("empty"));
        device_stack.add_named(&device_card, Some("devices"));
        device_stack.set_visible_child_name("empty");

        // ── Settings side panel ──────────────────────────────────────────
        config_panel.page.set_width_request(SETTINGS_WIDTH - 20);
        config_panel.page.add_css_class("settings-panel");

        // ── Log area ─────────────────────────────────────────────────────
        let log_header = gtk4::Box::builder()
            .spacing(6)
            .css_classes(["log-header"])
            .build();
        log_header.append(
            &gtk4::Label::builder()
                .label("Log")
                .css_classes(["heading"])
                .halign(gtk4::Align::Start)
                .hexpand(true)
                .build(),
        );
        for (icon, tip, action) in [
            ("edit-copy-symbolic", "Copy log", "win.copy_log"),
            ("document-save-symbolic", "Save log to file…", "win.save_log"),
            ("edit-clear-all-symbolic", "Clear log", "win.clear_log"),
        ] {
            log_header.append(
                &gtk4::Button::builder()
                    .icon_name(icon)
                    .tooltip_text(tip)
                    .action_name(action)
                    .css_classes(["flat", "circular"])
                    .build(),
            );
        }
        let log_scroll = gtk4::ScrolledWindow::builder()
            .hexpand(true)
            .vexpand(true)
            .child(&log.text_view)
            .build();
        let log_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .css_classes(["log-pane"])
            .build();
        log_box.append(&log_header);
        log_box.append(&log_scroll);

        // Left column: devices on top, log below.
        let left_paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Vertical)
            .start_child(&device_stack)
            .end_child(&log_box)
            .resize_start_child(true)
            .resize_end_child(true)
            .shrink_start_child(false)
            .shrink_end_child(false)
            // Header + bottom bar take ~110px; split the rest roughly in half.
            .position(((config.window_height - 110).max(300) as f64 * 0.5) as i32)
            .build();

        // Settings get the full height on the right so they always fit,
        // even on 1366×768 screens.
        let main_paned = gtk4::Paned::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .start_child(&left_paned)
            .end_child(&config_panel.page)
            .resize_start_child(true)
            .resize_end_child(false)
            .shrink_start_child(false)
            .shrink_end_child(false)
            .position((config.window_width - 340).max(380))
            .build();

        // ── Bottom action bar ────────────────────────────────────────────
        let image_icon = gtk4::Image::builder()
            .icon_name("media-optical-symbolic")
            .pixel_size(24)
            .css_classes(["dim-label"])
            .build();
        let image_name = gtk4::Label::builder()
            .label("No image loaded")
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .css_classes(["heading"])
            .build();
        let image_detail = gtk4::Label::builder()
            .label("Open an .img file or drop it onto the window")
            .halign(gtk4::Align::Start)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .css_classes(["caption", "dim-label"])
            .build();
        let image_text = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .valign(gtk4::Align::Center)
            .hexpand(true)
            .build();
        image_text.append(&image_name);
        image_text.append(&image_detail);

        let status_label = gtk4::Label::builder()
            .label("Ready")
            .halign(gtk4::Align::End)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .max_width_chars(36)
            .css_classes(["caption", "status-text"])
            .build();
        let progress = gtk4::ProgressBar::builder()
            .width_request(200)
            .show_text(true)
            .visible(false)
            .valign(gtk4::Align::Center)
            .css_classes(["bar-progress"])
            .build();
        let status_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .valign(gtk4::Align::Center)
            .build();
        status_box.append(&status_label);
        status_box.append(&progress);

        let start_button = gtk4::Button::builder()
            .child(
                &libadwaita::ButtonContent::builder()
                    .icon_name("media-playback-start-symbolic")
                    .label("Start")
                    .build(),
            )
            .tooltip_text("Start burning (Ctrl+Enter)")
            .css_classes(["suggested-action", "pill", "action-button"])
            .sensitive(false)
            .build();
        let stop_button = gtk4::Button::builder()
            .child(
                &libadwaita::ButtonContent::builder()
                    .icon_name("media-playback-stop-symbolic")
                    .label("Stop")
                    .build(),
            )
            .tooltip_text("Abort burning (Shift+Ctrl+Enter)")
            .css_classes(["destructive-action", "pill", "action-button"])
            .visible(false)
            .build();

        let bottom_bar = gtk4::Box::builder()
            .spacing(12)
            .css_classes(["bottom-bar"])
            .build();
        bottom_bar.append(&image_icon);
        bottom_bar.append(&image_text);
        bottom_bar.append(&status_box);
        bottom_bar.append(&stop_button);
        bottom_bar.append(&start_button);

        // ── Assemble ─────────────────────────────────────────────────────
        let toolbar_view = libadwaita::ToolbarView::new();
        toolbar_view.add_top_bar(&header);
        toolbar_view.set_content(Some(&main_paned));

        // Size the panes from the window's *real* size (it may open maximized,
        // so the saved size is not reliable): settings panel fixed at
        // SETTINGS_WIDTH, devices/log split in half. Stops once the user
        // drags a divider.
        follow_size(&main_paned, |max| (max - SETTINGS_WIDTH).max(380));
        follow_size(&left_paned, |max| max / 2);
        toolbar_view.add_bottom_bar(&bottom_bar);
        toolbar_view.set_bottom_bar_style(libadwaita::ToolbarStyle::Raised);

        let toasts = libadwaita::ToastOverlay::new();
        toasts.set_child(Some(&toolbar_view));
        window.set_content(Some(&toasts));

        let inner = Rc::new(Inner {
            app: app.clone(),
            window: window.clone(),
            config: RefCell::new(config),
            tools_dir,
            device_list,
            config_panel,
            log,
            toasts,
            title,
            device_stack,
            recent_menu,
            open_button,
            image_name,
            image_detail,
            status_label,
            progress,
            start_button,
            stop_button,
            loading: Cell::new(false),
            burning: Cell::new(false),
            scanning: Cell::new(false),
            force_close: Cell::new(false),
        });

        inner.log_startup_diagnostics();
        inner.restore_options();
        inner.rebuild_recent();
        Inner::setup_actions(&inner);
        Inner::setup_drop(&inner);
        Inner::setup_signals(&inner);
        Inner::setup_device_polling(&inner);

        Self { window, inner }
    }

    /// Load an image passed on the command line / via "Open With".
    pub fn open_file(&self, path: PathBuf) {
        self.inner.load_image(path);
    }
}

impl Inner {
    // ── Setup ────────────────────────────────────────────────────────────

    fn log_startup_diagnostics(&self) {
        self.log.append_log(&format!(
            "Amlogic USB Burning Tool {} — tools: {}",
            env!("CARGO_PKG_VERSION"),
            self.tools_dir.display()
        ));
        let arch = if cfg!(target_arch = "x86_64") { "linux-x86" } else { "linux-arm" };
        for rel in [format!("{arch}/update"), "aml_flash_tool.sh".to_string()] {
            if !self.tools_dir.join(&rel).exists() {
                self.log.append_log(&format!(
                    "WARNING: bundled tool missing: {}",
                    self.tools_dir.join(&rel).display()
                ));
            }
        }
    }

    fn restore_options(&self) {
        let cfg = self.config.borrow();
        let p = &self.config_panel;
        p.set_soc(&cfg.last_soc);
        p.erase_flash_switch.set_active(cfg.last_erase_flash);
        p.erase_bootloader_switch.set_active(cfg.last_erase_bootloader);
        p.reset_switch.set_active(cfg.last_reset_after_burn);
        p.set_password_path(&cfg.last_password_path);
        p.set_efuse_path(&cfg.last_efuse_path);
    }

    fn store_options(&self) {
        let mut cfg = self.config.borrow_mut();
        let p = &self.config_panel;
        cfg.last_erase_flash = p.erase_flash_switch.is_active();
        cfg.last_erase_bootloader = p.erase_bootloader_switch.is_active();
        cfg.last_reset_after_burn = p.reset_switch.is_active();
        cfg.last_soc = p.soc_raw();
        cfg.last_password_path = p.password_path();
        cfg.last_efuse_path = p.efuse_path();
        cfg.save();
    }

    fn add_action(self: &Rc<Self>, name: &str, accels: &[&str], f: impl Fn(&Rc<Self>) + 'static) {
        let action = gio::SimpleAction::new(name, None);
        let weak = Rc::downgrade(self);
        action.connect_activate(move |_, _| {
            if let Some(this) = weak.upgrade() {
                f(&this);
            }
        });
        self.window.add_action(&action);
        if !accels.is_empty() {
            self.app.set_accels_for_action(&format!("win.{name}"), accels);
        }
    }

    fn setup_actions(this: &Rc<Self>) {
        this.add_action("import_image", &["<Ctrl>o"], |s| {
            let weak = Rc::downgrade(s);
            dialogs::show_image_chooser(&s.window, move |opt| {
                if let (Some(path), Some(s)) = (opt, weak.upgrade()) {
                    s.load_image(path);
                }
            });
        });

        // Recent images: one parameterised action instead of ten fixed ones.
        let open_recent =
            gio::SimpleAction::new("open_recent", Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(this);
        open_recent.connect_activate(move |_, param| {
            if let (Some(s), Some(p)) = (weak.upgrade(), param.and_then(|v| v.get::<String>())) {
                s.load_image(PathBuf::from(p));
            }
        });
        this.window.add_action(&open_recent);

        this.add_action("clear_recent", &[], |s| {
            s.config.borrow_mut().recent_images.clear();
            s.config.borrow().save();
            s.rebuild_recent();
        });

        this.add_action("refresh_devices", &["F5", "<Ctrl>r"], |s| {
            s.log.append_log("Scanning for devices…");
            s.scan_devices(true);
        });

        this.add_action("start_burn", &["<Ctrl>Return", "<Ctrl>KP_Enter"], |s| {
            if s.start_button.is_sensitive() && s.start_button.is_visible() {
                Inner::start_burn(s);
            }
        });
        this.add_action("stop_burn", &["<Shift><Ctrl>Return"], |s| s.stop_burn());

        this.add_action("password_file", &[], |s| {
            let weak = Rc::downgrade(s);
            dialogs::show_file_dialog(&s.window, "Select Password File", "Select", move |opt| {
                if let (Some(path), Some(s)) = (opt, weak.upgrade()) {
                    let p = path.to_string_lossy().to_string();
                    s.config_panel.set_password_path(&p);
                    s.log.append_log(&format!("Password file: {p}"));
                }
            });
        });

        this.add_action("efuse_file", &[], |s| {
            let weak = Rc::downgrade(s);
            dialogs::show_file_dialog(&s.window, "Select eFuse File", "Select", move |opt| {
                if let (Some(path), Some(s)) = (opt, weak.upgrade()) {
                    let p = path.to_string_lossy().to_string();
                    s.config_panel.set_efuse_path(&p);
                    s.log.append_log(&format!("eFuse file: {p}"));
                }
            });
        });

        this.add_action("save_log", &[], |s| {
            let weak = Rc::downgrade(s);
            let filename = format!(
                "burn-log-{}.txt",
                chrono::Local::now().format("%Y%m%d_%H%M%S")
            );
            dialogs::show_save_dialog(&s.window, &filename, move |opt| {
                let (Some(path), Some(s)) = (opt, weak.upgrade()) else {
                    return;
                };
                match std::fs::write(&path, s.log.text()) {
                    Ok(_) => {
                        s.log.set_log_file(path.clone());
                        s.log.append_log(&format!(
                            "Log saved to {} — new entries are appended to it",
                            path.display()
                        ));
                        s.toast("Log saved");
                    }
                    Err(e) => {
                        s.log.append_log(&format!("ERROR saving log: {e}"));
                        s.toast(&format!("Could not save log: {e}"));
                    }
                }
            });
        });

        this.add_action("copy_log", &[], |s| {
            s.window.clipboard().set_text(&s.log.text());
            s.toast("Log copied to clipboard");
        });

        this.add_action("clear_log", &["<Ctrl>l"], |s| {
            s.log.clear_log_file();
            s.log.clear();
        });

        this.add_action("shortcuts", &["<Ctrl>question"], |s| s.show_shortcuts());
        this.add_action("about", &[], |s| s.show_about());
        this.add_action("close", &["<Ctrl>w", "<Ctrl>q"], |s| s.window.close());
    }

    fn setup_drop(this: &Rc<Self>) {
        let drop = gtk4::DropTarget::new(
            gtk4::gdk::FileList::static_type(),
            gtk4::gdk::DragAction::COPY,
        );
        let weak = Rc::downgrade(this);
        drop.connect_drop(move |_, value, _, _| {
            let Some(s) = weak.upgrade() else {
                return false;
            };
            let Ok(list) = value.get::<gtk4::gdk::FileList>() else {
                return false;
            };
            if let Some(path) = list.files().iter().filter_map(|f| f.path()).find(|p| p.is_file()) {
                s.load_image(path);
                return true;
            }
            false
        });
        this.window.add_controller(drop);
    }

    fn setup_signals(this: &Rc<Self>) {
        let weak = Rc::downgrade(this);
        this.start_button.connect_clicked(move |_| {
            if let Some(s) = weak.upgrade() {
                Inner::start_burn(&s);
            }
        });
        let weak = Rc::downgrade(this);
        this.stop_button.connect_clicked(move |_| {
            if let Some(s) = weak.upgrade() {
                s.stop_burn();
            }
        });

        // Keep log colors readable when the system switches light/dark.
        let weak = Rc::downgrade(this);
        libadwaita::StyleManager::default().connect_dark_notify(move |sm| {
            if let Some(s) = weak.upgrade() {
                s.log.update_tag_colors(sm.is_dark());
            }
        });

        // Ask before quitting mid-burn; persist settings on close.
        let weak = Rc::downgrade(this);
        this.window.connect_close_request(move |win| {
            let Some(s) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if s.burning.get() && !s.force_close.get() {
                let weak = Rc::downgrade(&s);
                dialogs::show_confirm_dialog(
                    win,
                    "Quit While Burning?",
                    "Flashing is still in progress. Quitting aborts it and can leave the device unbootable.",
                    "Abort and Quit",
                    move |ok| {
                        let Some(s) = weak.upgrade() else { return };
                        if ok {
                            s.force_close.set(true);
                            s.stop_burn();
                            let win = s.window.clone();
                            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                                win.close();
                            });
                        }
                    },
                );
                return glib::Propagation::Stop;
            }
            {
                let mut cfg = s.config.borrow_mut();
                cfg.window_maximized = win.is_maximized();
                if !win.is_maximized() {
                    cfg.window_width = win.width();
                    cfg.window_height = win.height();
                }
            }
            s.store_options();
            glib::Propagation::Proceed
        });
    }

    fn setup_device_polling(this: &Rc<Self>) {
        this.scan_devices(false);
        let weak = Rc::downgrade(this);
        glib::timeout_add_local(Duration::from_secs(3), move || match weak.upgrade() {
            Some(s) => {
                s.scan_devices(false);
                glib::ControlFlow::Continue
            }
            None => glib::ControlFlow::Break,
        });
    }

    // ── Helpers ──────────────────────────────────────────────────────────

    fn toast(&self, msg: &str) {
        let toast = libadwaita::Toast::new(&glib::markup_escape_text(msg));
        toast.set_timeout(4);
        self.toasts.add_toast(toast);
    }

    fn set_status(&self, msg: &str) {
        self.status_label.set_text(msg);
        self.status_label.set_tooltip_text(Some(msg));
    }

    fn update_controls(&self) {
        let has_image = !self.config_panel.image_path().is_empty();
        let busy = self.loading.get() || self.burning.get();
        self.start_button.set_sensitive(has_image && !busy);
        self.start_button.set_visible(!self.burning.get());
        self.stop_button.set_visible(self.burning.get());
        self.stop_button.set_sensitive(self.burning.get());
        self.open_button.set_sensitive(!busy);
        self.config_panel.set_locked(self.burning.get());
        self.start_button.set_tooltip_text(Some(if !has_image {
            "Open a firmware image first"
        } else {
            "Start burning (Ctrl+Enter)"
        }));
    }

    fn rebuild_recent(&self) {
        self.recent_menu.remove_all();
        let cfg = self.config.borrow();
        let items = gio::Menu::new();
        if cfg.recent_images.is_empty() {
            items.append_item(&gio::MenuItem::new(Some("No recent images"), Some("win.none")));
        }
        for path in &cfg.recent_images {
            let p = Path::new(path);
            let name = p
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.clone());
            let dir = p
                .parent()
                .and_then(|d| d.file_name())
                .map(|d| format!("  —  {}", d.to_string_lossy()))
                .unwrap_or_default();
            // Escape underscores so they are not taken as mnemonics.
            let label = format!("{name}{dir}").replace('_', "__");
            let item = gio::MenuItem::new(Some(&label), None);
            item.set_action_and_target_value(Some("win.open_recent"), Some(&path.to_variant()));
            items.append_item(&item);
        }
        self.recent_menu.append_section(Some("Recent Images"), &items);
        if !cfg.recent_images.is_empty() {
            let tail = gio::Menu::new();
            tail.append(Some("Clear Recent"), Some("win.clear_recent"));
            self.recent_menu.append_section(None, &tail);
        }
    }

    fn show_image(&self, path: Option<&Path>, detail: &str) {
        match path {
            Some(p) => {
                let name = p
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.image_name.set_text(&name);
                self.image_name.set_tooltip_text(Some(&p.to_string_lossy()));
            }
            None => {
                self.image_name.set_text("No image loaded");
                self.image_name.set_tooltip_text(None);
            }
        }
        self.image_detail.set_text(detail);
    }

    // ── Image loading ────────────────────────────────────────────────────

    fn load_image(self: &Rc<Self>, path: PathBuf) {
        if self.burning.get() || self.loading.get() {
            self.toast("Busy — wait for the current operation to finish");
            return;
        }
        let path_str = path.to_string_lossy().to_string();
        if !path.is_file() {
            self.toast("Image file not found");
            self.log.append_log(&format!("ERROR: file not found: {path_str}"));
            self.config.borrow_mut().recent_images.retain(|p| p != &path_str);
            self.config.borrow().save();
            self.rebuild_recent();
            return;
        }

        self.loading.set(true);
        self.config_panel.set_image_path("");
        self.config_panel.set_partitions(&[]);
        self.update_controls();

        let size = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
        let dir = path
            .parent()
            .map(|d| d.to_string_lossy().to_string())
            .unwrap_or_default();
        self.show_image(Some(&path), &format!("Reading… · {}", human_size(size)));
        self.log.append_log(&format!("Loading image: {path_str}"));
        self.set_status("Reading image…");
        self.progress.set_text(Some("Extracting"));
        self.progress.set_fraction(0.0);
        self.progress.set_visible(true);

        self.config.borrow_mut().add_recent_image(&path_str);
        self.config.borrow().save();
        self.rebuild_recent();

        let weak_done = Rc::downgrade(self);
        let weak_prog = Rc::downgrade(self);
        let path_done = path.clone();
        image::unpack_image_async(
            path,
            self.tools_dir.clone(),
            move |result| {
                let Some(s) = weak_done.upgrade() else { return };
                s.loading.set(false);
                s.progress.set_visible(false);
                match result {
                    Ok(info) => {
                        let n = info.partitions.len();
                        let names: Vec<String> =
                            info.partitions.iter().map(|p| p.name.clone()).collect();
                        s.config_panel.set_image_path(&path_done.to_string_lossy());
                        s.config_panel.set_partitions(&names);
                        s.config_panel.set_soc(&info.soc);
                        s.show_image(
                            Some(&path_done),
                            &format!(
                                "{} · {n} partitions · SoC {} · {dir}",
                                human_size(size),
                                info.soc
                            ),
                        );
                        s.log.append_log(&format!(
                            "Image loaded: {n} partitions, SoC {}",
                            info.soc
                        ));
                        s.set_status("Image ready");
                    }
                    Err(e) => {
                        s.show_image(None, "Open an .img file or drop it onto the window");
                        s.log.append_log(&format!("ERROR: could not read image: {e}"));
                        s.set_status("Image load failed");
                        s.toast("Could not read image — see log for details");
                    }
                }
                s.update_controls();
            },
            move |_count, name| {
                if let Some(s) = weak_prog.upgrade() {
                    s.progress.pulse();
                    s.progress.set_text(Some(&name));
                }
            },
        );
    }

    // ── Devices ──────────────────────────────────────────────────────────

    fn scan_devices(self: &Rc<Self>, manual: bool) {
        // Never poke the USB bus with `update scan` while flashing.
        if self.scanning.get() || (self.burning.get() && !manual) {
            return;
        }
        if self.burning.get() {
            self.toast("Device scan is paused while burning");
            return;
        }
        self.scanning.set(true);
        let weak = Rc::downgrade(self);
        let prev = self.device_list.device_count();
        device::scan_devices_background(self.tools_dir.clone(), move |devices| {
            let Some(s) = weak.upgrade() else { return };
            s.scanning.set(false);
            s.device_list.update_devices(&devices);
            let count = s.device_list.device_count();
            s.device_stack
                .set_visible_child_name(if count > 0 { "devices" } else { "empty" });
            s.title.set_subtitle(&match count {
                0 => "No device connected".to_string(),
                1 => "1 device connected".to_string(),
                n => format!("{n} devices connected"),
            });
            if count != prev {
                if count > prev {
                    s.log.append_log(&format!("Device connected — {count} device(s)"));
                } else {
                    s.log.append_log(&format!("Device disconnected — {count} device(s)"));
                }
                if !s.burning.get() && !s.loading.get() {
                    s.set_status(&format!("{count} device(s) detected"));
                }
            }
            if manual {
                s.log.append_log(&format!("Scan complete — {count} device(s)"));
            }
        });
    }

    // ── Burning ──────────────────────────────────────────────────────────

    fn start_burn(this: &Rc<Self>) {
        if this.burning.get() || this.loading.get() {
            return;
        }
        let img_path = this.config_panel.image_path();
        if img_path.is_empty() {
            this.toast("Open a firmware image first");
            return;
        }

        // Finished devices that are still attached can be flashed again.
        this.device_list.reset_finished();
        let ready = this
            .device_list
            .devices
            .borrow()
            .iter()
            .filter(|d| matches!(d.borrow().status, BurnStatus::Ready))
            .count();
        if ready == 0 {
            this.toast("No device connected in USB burning mode");
            return;
        }

        let mut cfg = BurnConfig {
            image_path: Some(PathBuf::from(&img_path)),
            wipe: this.config_panel.erase_flash_switch.is_active(),
            erase_bootloader: this.config_panel.erase_bootloader_switch.is_active(),
            reset: this.config_panel.reset_switch.is_active(),
            soc: this.config_panel.soc(),
            ..Default::default()
        };
        let pw = this.config_panel.password_path();
        if !pw.is_empty() {
            cfg.password_path = Some(PathBuf::from(pw));
        }
        let ef = this.config_panel.efuse_path();
        if !ef.is_empty() {
            cfg.efuse_path = Some(PathBuf::from(ef));
        }
        let (selected, total) = this.config_panel.selected_partitions();
        cfg.parts = if selected.is_empty() {
            "none".into()
        } else if selected.len() == total {
            "all".into()
        } else {
            selected.join(",")
        };
        if selected.is_empty() && !cfg.wipe && !cfg.erase_bootloader {
            this.toast("Select at least one partition to burn");
            return;
        }
        this.store_options();

        let proceed = {
            let weak = Rc::downgrade(this);
            move |cfg: BurnConfig| {
                if let Some(s) = weak.upgrade() {
                    Inner::detect_engine_and_burn(&s, cfg);
                }
            }
        };

        if cfg.erase_bootloader {
            dialogs::show_confirm_dialog(
                &this.window,
                "Erase Bootloader?",
                "Erasing the bootloader can brick the device if the burn is interrupted.",
                "Erase and Burn",
                move |ok| {
                    if ok {
                        proceed(cfg);
                    }
                },
            );
        } else {
            proceed(cfg);
        }
    }

    fn detect_engine_and_burn(this: &Rc<Self>, mut cfg: BurnConfig) {
        this.burning.set(true);
        this.update_controls();
        this.set_status("Detecting device mode…");

        let adnl_path = this.tools_dir.join("adnl").join("adnl");
        let (tx, rx) = std::sync::mpsc::channel::<bool>();
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
                            let t = l.trim();
                            !t.is_empty() && t != "No DNL devices found"
                        })
                    })
                    .unwrap_or(false);
                let _ = tx.send(has_adnl);
            });
        } else {
            let _ = tx.send(false);
        }

        let weak = Rc::downgrade(this);
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let has_adnl = match rx.try_recv() {
                Ok(v) => v,
                Err(std::sync::mpsc::TryRecvError::Empty) => return glib::ControlFlow::Continue,
                Err(_) => false,
            };
            let Some(s) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if has_adnl {
                if cfg.wipe || cfg.erase_bootloader {
                    s.log.append_log(
                        "ERROR: Erase Flash / Erase Bootloader are not supported by the \
                         ADNL engine — uncheck them and retry",
                    );
                    s.toast("ADNL engine does not support erase options");
                    s.set_status("Burn aborted: unsupported ADNL option");
                    s.burning.set(false);
                    s.update_controls();
                    return glib::ControlFlow::Break;
                }
                s.log.append_log("ADNL device detected — using adnl_flash_tool.sh");
                cfg.adnl = true;
            } else if !CLASSIC_SOCS.contains(&cfg.soc.as_str()) {
                s.log.append_log(&format!(
                    "ERROR: SoC '{}' is not supported by the classic burn engine \
                     (supported: {}). Connect the device in ADNL mode for this SoC.",
                    cfg.soc,
                    CLASSIC_SOCS.join(", ")
                ));
                s.toast(&format!("SoC '{}' needs ADNL mode", cfg.soc));
                s.set_status("Burn aborted: unsupported SoC");
                s.burning.set(false);
                s.update_controls();
                return glib::ControlFlow::Break;
            }
            Inner::launch_burns(&s, cfg.clone());
            glib::ControlFlow::Break
        });
    }

    fn launch_burns(this: &Rc<Self>, cfg: BurnConfig) {
        let devices: Vec<Rc<RefCell<DeviceState>>> = this
            .device_list
            .devices
            .borrow()
            .iter()
            .filter(|d| matches!(d.borrow().status, BurnStatus::Ready))
            .cloned()
            .collect();
        if devices.is_empty() {
            this.toast("Device disappeared before burning started");
            this.burning.set(false);
            this.update_controls();
            return;
        }

        // adnl_flash_tool.sh selects the first device reported by `adnl
        // devices`; running it once per device would flash the same unit
        // repeatedly.  Until the script accepts a serial, allow one at a time.
        if cfg.adnl && devices.len() > 1 {
            this.log.append_log(&format!(
                "ERROR: {} devices are ready, but the ADNL engine supports one device per burn",
                devices.len()
            ));
            this.toast("ADNL: burn one device at a time");
            this.burning.set(false);
            this.update_controls();
            return;
        }

        let script_dir = this
            .tools_dir
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| this.tools_dir.clone());

        let count = devices.len();
        this.log.append_log(&format!(
            "Starting burn on {count} device(s) — parts={}, wipe={}, reset={}, soc={}",
            cfg.parts, cfg.wipe, cfg.reset, cfg.soc
        ));
        this.set_status(&format!("Burning {count} device(s)…"));
        this.progress.set_fraction(0.0);
        this.progress.set_text(Some("0%"));
        this.progress.set_visible(true);

        for device in devices {
            let (cancel_tx, cancel_rx) = std::sync::mpsc::channel::<()>();
            {
                let mut dev = device.borrow_mut();
                dev.status = BurnStatus::Burning(0.0);
                dev.cancel_tx = Some(cancel_tx);
            }
            this.device_list.refresh_display();

            let s = this.clone();
            let cfg = cfg.clone();
            let script_dir = script_dir.clone();
            glib::MainContext::default().spawn_local(async move {
                s.run_device_burn(device, cfg, script_dir, cancel_rx).await;
            });
        }
    }

    async fn run_device_burn(
        self: Rc<Self>,
        dev: Rc<RefCell<DeviceState>>,
        cfg: BurnConfig,
        script_dir: PathBuf,
        cancel_rx: std::sync::mpsc::Receiver<()>,
    ) {
        let id = dev.borrow().hub_id.clone();
        self.log.append_log(&format!("[{id}] Starting burn…"));

        let mut burner = match BurnProcess::start(&cfg, &script_dir) {
            Ok(b) => b,
            Err(e) => {
                self.log.append_log(&format!("[{id}] Failed to start: {e}"));
                dev.borrow_mut().status = BurnStatus::Error(e);
                dev.borrow_mut().cancel_tx = None;
                self.device_list.refresh_display();
                self.finish_burn_check();
                return;
            }
        };

        let mut cancelled = false;
        let mut success = false;
        let mut exit_code = None;

        loop {
            if cancel_rx.try_recv().is_ok() {
                cancelled = true;
                self.log.append_log(&format!("[{id}] Cancelled by user"));
                burner.kill();
                break;
            }
            match burner.read_line() {
                Some(BurnEvent::Line(line, is_err)) => {
                    if !line.trim().is_empty() {
                        let tag = if is_err { " [stderr]" } else { "" };
                        self.log.append_log(&format!("[{id}]{tag} {line}"));
                    }
                    let clean = crate::ui::log_panel::strip_ansi(&line);
                    if let Some(pct) = crate::backend::progress::parse_progress(&clean) {
                        dev.borrow_mut().status = BurnStatus::Burning(pct / 100.0);
                        self.device_list.refresh_display();
                        self.update_overall_progress();
                    }
                    if clean.contains("[OK]") {
                        success = true;
                    }
                    if clean.contains("[KO]") {
                        success = false;
                    }
                }
                Some(BurnEvent::Exited(code)) => exit_code = Some(code),
                Some(BurnEvent::Error(e)) => {
                    self.log.append_log(&format!("[{id}] Error: {e}"));
                    burner.kill();
                    break;
                }
                None => {
                    if exit_code.is_some() {
                        break;
                    }
                    glib::timeout_future(Duration::from_millis(50)).await;
                }
            }
        }

        let status = if cancelled {
            BurnStatus::Error("Cancelled".into())
        } else {
            match (exit_code.unwrap_or(-1), success) {
                (0, true) => {
                    self.log.append_log(&format!("[{id}] Completed successfully"));
                    BurnStatus::Success
                }
                (0, false) => {
                    self.log.append_log(&format!(
                        "[{id}] WARNING: process exited cleanly but no [OK] marker was seen"
                    ));
                    BurnStatus::Error("No [OK] marker".into())
                }
                (code, _) => {
                    self.log.append_log(&format!("[{id}] Failed (exit code {code})"));
                    BurnStatus::Error(format!("exit code {code}"))
                }
            }
        };
        {
            let mut d = dev.borrow_mut();
            d.status = status;
            d.cancel_tx = None;
        }
        self.device_list.refresh_display();
        self.finish_burn_check();
    }

    fn update_overall_progress(&self) {
        let devices = self.device_list.devices.borrow();
        let burning: Vec<f64> = devices
            .iter()
            .filter_map(|d| match d.borrow().status {
                BurnStatus::Burning(p) => Some(p),
                _ => None,
            })
            .collect();
        if burning.is_empty() {
            return;
        }
        let avg = burning.iter().sum::<f64>() / burning.len() as f64;
        self.progress.set_fraction(avg.clamp(0.0, 1.0));
        self.progress.set_text(Some(&format!("{:.0}%", avg * 100.0)));
    }

    fn finish_burn_check(&self) {
        if self.device_list.any_burning() {
            return;
        }
        self.burning.set(false);
        self.progress.set_visible(false);
        let devices = self.device_list.devices.borrow();
        let ok = devices
            .iter()
            .filter(|d| matches!(d.borrow().status, BurnStatus::Success))
            .count();
        let failed = devices
            .iter()
            .filter(|d| matches!(d.borrow().status, BurnStatus::Error(_)))
            .count();
        let cancelled = devices
            .iter()
            .any(|d| matches!(&d.borrow().status, BurnStatus::Error(e) if e == "Cancelled"));
        drop(devices);
        let summary = match (ok, failed) {
            _ if cancelled => "Burn stopped".to_string(),
            (o, 0) => format!("Burn finished — {o} succeeded"),
            (0, f) => format!("Burn failed on {f} device(s)"),
            (o, f) => format!("Burn finished — {o} succeeded, {f} failed"),
        };
        self.set_status(&summary);
        self.toast(&summary);
        self.update_controls();

        if !self.window.is_active() {
            let n = gio::Notification::new(APP_NAME);
            n.set_body(Some(&summary));
            self.app.send_notification(Some("burn-finished"), &n);
        }
    }

    fn stop_burn(&self) {
        let mut any = false;
        for d in self.device_list.devices.borrow().iter() {
            let mut dev = d.borrow_mut();
            if let Some(tx) = dev.cancel_tx.take() {
                let _ = tx.send(());
                any = true;
                self.log.append_log(&format!("[{}] Stopping…", dev.hub_id));
            }
        }
        if any {
            self.set_status("Stopping…");
            self.stop_button.set_sensitive(false);
        }
    }

    // ── Dialogs ──────────────────────────────────────────────────────────

    fn show_about(&self) {
        let build_ts = include_str!(concat!(env!("OUT_DIR"), "/build_timestamp"))
            .trim()
            .parse::<i64>()
            .unwrap_or(0);
        let build_time = chrono::DateTime::from_timestamp(build_ts, 0)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".into());
        let about = libadwaita::AboutDialog::builder()
            .application_name(APP_NAME)
            .application_icon("aml-flash-tool")
            .version(env!("CARGO_PKG_VERSION"))
            .developer_name("Anton")
            .comments(format!(
                "Flash firmware to Amlogic devices over USB.\nBuilt {build_time}"
            ))
            .website("https://github.com/antoxa78/Amlogic-Tool-for-Linux")
            .issue_url("https://github.com/antoxa78/Amlogic-Tool-for-Linux/issues")
            .license_type(gtk4::License::MitX11)
            .build();
        about.present(Some(&self.window));
    }

    fn show_shortcuts(&self) {
        let rows = [
            ("Open image", "<Ctrl>o"),
            ("Start burning", "<Ctrl>Return"),
            ("Stop burning", "<Shift><Ctrl>Return"),
            ("Refresh devices", "F5"),
            ("Clear log", "<Ctrl>l"),
            ("Close window", "<Ctrl>w"),
        ];
        let group = libadwaita::PreferencesGroup::new();
        for (title, accel) in rows {
            let row = libadwaita::ActionRow::builder().title(title).build();
            row.add_suffix(
                &gtk4::ShortcutLabel::builder()
                    .accelerator(accel)
                    .valign(gtk4::Align::Center)
                    .build(),
            );
            group.add(&row);
        }
        let page = libadwaita::PreferencesPage::new();
        page.add(&group);
        let view = libadwaita::ToolbarView::new();
        view.add_top_bar(&libadwaita::HeaderBar::new());
        view.set_content(Some(&page));
        let dialog = libadwaita::Dialog::builder()
            .title("Keyboard Shortcuts")
            .content_width(380)
            .content_height(420)
            .child(&view)
            .build();
        dialog.present(Some(&self.window));
    }
}

/// Keep a paned divider at `place(total_length)` while the window is being
/// sized, until the user moves the divider themselves.
fn follow_size(paned: &gtk4::Paned, place: impl Fn(i32) -> i32 + 'static) {
    let last_set: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));
    paned.connect_notify_local(Some("max-position"), move |p, _| {
        let max = p.max_position();
        if max <= 0 {
            return;
        }
        // max-position already excludes the end child's minimum size, so add
        // it back to get the full length available to both panes.
        let end_min = p
            .end_child()
            .map(|c| c.measure(p.orientation(), -1).0)
            .unwrap_or(0);
        let total = max + end_min;
        let untouched = last_set.get().is_none_or(|v| v == p.position());
        if untouched {
            let pos = place(total).clamp(p.min_position(), max);
            p.set_position(pos);
            last_set.set(Some(pos));
        }
    });
}

fn primary_monitor_size() -> Option<(i32, i32)> {
    let display = gtk4::gdk::Display::default()?;
    let monitors = display.monitors();
    let monitor = monitors.item(0).and_downcast::<gtk4::gdk::Monitor>()?;
    let geo = monitor.geometry();
    Some((geo.width(), geo.height()))
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

fn find_tools_dir() -> PathBuf {
    let system_tools = PathBuf::from("/usr/lib/amlogicburn/tools");
    if system_tools.exists() {
        return system_tools;
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
    {
        let candidate = dir.join("tools");
        if candidate.exists() {
            return candidate;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tools")
}

#[cfg(test)]
mod tests {
    use super::human_size;

    #[test]
    fn sizes() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(1536), "1.5 KB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
