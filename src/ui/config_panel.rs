use gtk4::prelude::*;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

// First word is the SoC id passed to the burn scripts; the rest is a hint.
// Kept short so the selection fits beside the row title.
static SOCS: &[&str] = &[
    "gxl · S905X/W, S912",
    "g12a · S905D2/X2",
    "g12b · S922X, A311D",
    "sm1 · S905D3/X3",
    "t5d · T5D",
    "t7 · T7",
    "s4 · S4",
    "s5 · S5",
    "axg · A113X",
    "a1 · A1",
    "m8 · S805, A111",
    "txlx · T962E",
    "sc2 · S905X4",
    "t3 · T3",
];

pub struct ConfigPanel {
    pub page: libadwaita::PreferencesPage,
    pub reset_switch: libadwaita::SwitchRow,
    pub erase_flash_switch: libadwaita::SwitchRow,
    pub erase_bootloader_switch: libadwaita::SwitchRow,
    soc_row: libadwaita::ComboRow,
    partitions_row: libadwaita::ActionRow,
    partitions_list: gtk4::ListBox,
    partitions_summary: gtk4::Label,
    security_summary: gtk4::Label,
    partition_checks: Rc<RefCell<Vec<gtk4::CheckButton>>>,
    partition_names: RefCell<Vec<String>>,
    password_row: libadwaita::ActionRow,
    password_clear: gtk4::Button,
    efuse_row: libadwaita::ActionRow,
    efuse_clear: gtk4::Button,
    password_path: RefCell<String>,
    efuse_path: RefCell<String>,
    image_path: RefCell<String>,
}

impl ConfigPanel {
    pub fn new() -> Rc<Self> {
        let page = libadwaita::PreferencesPage::new();
        page.add_css_class("compact-panel");

        // ── Firmware ───────────────────────────────────────────────────────
        let firmware_group = libadwaita::PreferencesGroup::builder()
            .title("Firmware")
            .build();

        let soc_model = gtk4::StringList::new(SOCS);
        let soc_row = libadwaita::ComboRow::builder()
            .title("SoC")
            .model(&soc_model)
            .tooltip_text("g12b and sm1 are burned with the g12a protocol")
            .build();
        firmware_group.add(&soc_row);

        // Partitions open in a drop-down so the panel keeps a fixed height
        // (an inline expander pushes the options off small screens).
        let partitions_list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(["partitions-list"])
            .build();
        let parts_scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(340)
            .child(&partitions_list)
            .build();
        let select_all = gtk4::Button::builder()
            .label("All")
            .css_classes(["flat", "small-button"])
            .build();
        let select_none = gtk4::Button::builder()
            .label("None")
            .css_classes(["flat", "small-button"])
            .build();
        let pop_header = gtk4::Box::builder().spacing(4).build();
        pop_header.append(
            &gtk4::Label::builder()
                .label("Partitions to burn")
                .css_classes(["heading"])
                .hexpand(true)
                .xalign(0.0)
                .build(),
        );
        pop_header.append(&select_all);
        pop_header.append(&select_none);
        let pop_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(6)
            .width_request(240)
            .build();
        pop_box.append(&pop_header);
        pop_box.append(&parts_scroll);
        let popover = gtk4::Popover::builder().child(&pop_box).build();

        let partitions_summary = summary_label("—");
        let parts_button = gtk4::MenuButton::builder()
            .child(&partitions_summary)
            .popover(&popover)
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();
        let partitions_row = libadwaita::ActionRow::builder()
            .title("Partitions")
            .activatable_widget(&parts_button)
            .sensitive(false)
            .build();
        partitions_row.add_suffix(&parts_button);
        firmware_group.add(&partitions_row);

        // ── Options ────────────────────────────────────────────────────────
        let options_group = libadwaita::PreferencesGroup::builder()
            .title("Options")
            .build();

        let erase_flash_switch = libadwaita::SwitchRow::builder()
            .title("Erase Flash")
            .tooltip_text("Wipe all data before burning")
            .build();
        options_group.add(&erase_flash_switch);

        let erase_bootloader_switch = libadwaita::SwitchRow::builder()
            .title("Erase Bootloader")
            .tooltip_text("Can brick the device if the burn is interrupted")
            .build();
        let warn_icon = gtk4::Image::builder()
            .icon_name("dialog-warning-symbolic")
            .css_classes(["warning"])
            .tooltip_text("Dangerous")
            .build();
        erase_bootloader_switch.add_prefix(&warn_icon);
        options_group.add(&erase_bootloader_switch);

        let reset_switch = libadwaita::SwitchRow::builder()
            .title("Reset After Burn")
            .tooltip_text("Reboot the device when done")
            .active(true)
            .build();
        options_group.add(&reset_switch);

        // ── Security files (collapsed: rarely needed) ──────────────────────
        let security_row = libadwaita::ExpanderRow::builder()
            .title("Secure Boot Files")
            .tooltip_text("Password / eFuse files — only needed for locked devices")
            .build();
        let security_summary = summary_label("None");
        security_row.add_suffix(&security_summary);

        let (password_row, password_clear) =
            file_row("Password", "win.password_file", "dialog-password-symbolic");
        security_row.add_row(&password_row);
        let (efuse_row, efuse_clear) =
            file_row("eFuse", "win.efuse_file", "security-high-symbolic");
        security_row.add_row(&efuse_row);
        options_group.add(&security_row);

        page.add(&firmware_group);
        page.add(&options_group);

        let panel = Rc::new(Self {
            page,
            reset_switch,
            erase_flash_switch,
            erase_bootloader_switch,
            soc_row,
            partitions_row,
            partitions_list,
            partitions_summary,
            security_summary,
            partition_checks: Rc::new(RefCell::new(Vec::new())),
            partition_names: RefCell::new(Vec::new()),
            password_row,
            password_clear,
            efuse_row,
            efuse_clear,
            password_path: RefCell::new(String::new()),
            efuse_path: RefCell::new(String::new()),
            image_path: RefCell::new(String::new()),
        });

        {
            let weak = Rc::downgrade(&panel);
            panel.password_clear.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.set_password_path("");
                }
            });
            let weak = Rc::downgrade(&panel);
            panel.efuse_clear.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.set_efuse_path("");
                }
            });
            let weak = Rc::downgrade(&panel);
            select_all.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.partition_checks.borrow().iter().for_each(|c| c.set_active(true));
                }
            });
            let weak = Rc::downgrade(&panel);
            select_none.connect_clicked(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.partition_checks.borrow().iter().for_each(|c| c.set_active(false));
                }
            });
        }
        panel.set_password_path("");
        panel.set_efuse_path("");

        panel
    }

    // ── Image / partitions ────────────────────────────────────────────────

    pub fn set_image_path(&self, path: &str) {
        *self.image_path.borrow_mut() = path.to_string();
    }

    pub fn image_path(&self) -> String {
        self.image_path.borrow().clone()
    }

    pub fn set_partitions(self: &Rc<Self>, names: &[String]) {
        self.partitions_list.remove_all();
        self.partition_checks.borrow_mut().clear();
        *self.partition_names.borrow_mut() = names.to_vec();

        for name in names {
            let check = gtk4::CheckButton::builder()
                .label(name.as_str())
                .active(true)
                .build();
            let weak = Rc::downgrade(self);
            check.connect_toggled(move |_| {
                if let Some(p) = weak.upgrade() {
                    p.update_partition_summary();
                }
            });
            self.partitions_list.append(&check);
            self.partition_checks.borrow_mut().push(check);
        }

        self.partitions_row.set_sensitive(!names.is_empty());
        self.update_partition_summary();
    }

    fn update_partition_summary(&self) {
        let checks = self.partition_checks.borrow();
        let total = checks.len();
        let selected = checks.iter().filter(|c| c.is_active()).count();
        let text = match (total, selected) {
            (0, _) => "—".to_string(),
            (t, s) if s == t => format!("All {t}"),
            (_, 0) => "None".to_string(),
            (t, s) => format!("{s} / {t}"),
        };
        self.partitions_summary.set_text(&text);
        self.partitions_row
            .set_tooltip_text((total == 0).then_some("Load an image to choose partitions"));
    }

    /// Returns the selected partition names and the total partition count.
    pub fn selected_partitions(&self) -> (Vec<String>, usize) {
        let checks = self.partition_checks.borrow();
        let names = self.partition_names.borrow();
        let selected = checks
            .iter()
            .zip(names.iter())
            .filter(|(c, _)| c.is_active())
            .map(|(_, n)| n.clone())
            .collect();
        (selected, checks.len())
    }

    // ── SoC ───────────────────────────────────────────────────────────────

    pub fn set_soc(&self, soc: &str) {
        let lower = soc.trim().to_lowercase();
        if lower.is_empty() {
            return;
        }
        if let Some(pos) = SOCS
            .iter()
            .position(|s| s.split_whitespace().next() == Some(lower.as_str()))
        {
            self.soc_row.set_selected(pos as u32);
        }
    }

    pub fn soc_raw(&self) -> String {
        let pos = self.soc_row.selected();
        let entry = SOCS.get(pos as usize).unwrap_or(&"gxl");
        entry.split_whitespace().next().unwrap_or("gxl").to_string()
    }

    pub fn soc(&self) -> String {
        match self.soc_raw().as_str() {
            "g12b" | "sm1" => "g12a".to_string(),
            other => other.to_string(),
        }
    }

    // ── Security files ────────────────────────────────────────────────────

    pub fn set_password_path(&self, path: &str) {
        *self.password_path.borrow_mut() = path.to_string();
        show_path(&self.password_row, &self.password_clear, path);
        self.update_security_summary();
    }

    pub fn password_path(&self) -> String {
        self.password_path.borrow().clone()
    }

    pub fn set_efuse_path(&self, path: &str) {
        *self.efuse_path.borrow_mut() = path.to_string();
        show_path(&self.efuse_row, &self.efuse_clear, path);
        self.update_security_summary();
    }

    fn update_security_summary(&self) {
        let pw = !self.password_path.borrow().is_empty();
        let ef = !self.efuse_path.borrow().is_empty();
        let text = match (pw, ef) {
            (false, false) => "None",
            (true, false) => "Password",
            (false, true) => "eFuse",
            (true, true) => "Both",
        };
        self.security_summary.set_text(text);
        // Draw attention when files are set so they are not used unknowingly.
        if pw || ef {
            self.security_summary.add_css_class("accent");
        } else {
            self.security_summary.remove_css_class("accent");
        }
    }

    pub fn efuse_path(&self) -> String {
        self.efuse_path.borrow().clone()
    }

    /// Lock option widgets while a burn is running.
    pub fn set_locked(&self, locked: bool) {
        self.page.set_sensitive(!locked);
    }
}

fn summary_label(text: &str) -> gtk4::Label {
    gtk4::Label::builder()
        .label(text)
        .valign(gtk4::Align::Center)
        .css_classes(["dim-label", "caption", "row-summary"])
        .build()
}

fn file_row(title: &str, action: &str, icon: &str) -> (libadwaita::ActionRow, gtk4::Button) {
    let row = libadwaita::ActionRow::builder()
        .title(title)
        .subtitle_lines(1)
        .build();
    row.add_prefix(&gtk4::Image::from_icon_name(icon));

    let clear = gtk4::Button::builder()
        .icon_name("edit-clear-symbolic")
        .tooltip_text("Clear")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    let choose = gtk4::Button::builder()
        .icon_name("document-open-symbolic")
        .tooltip_text("Choose file…")
        .valign(gtk4::Align::Center)
        .action_name(action)
        .css_classes(["flat"])
        .build();
    row.add_suffix(&clear);
    row.add_suffix(&choose);
    row.set_activatable_widget(Some(&choose));
    (row, clear)
}

fn show_path(row: &libadwaita::ActionRow, clear: &gtk4::Button, path: &str) {
    if path.is_empty() {
        row.set_subtitle("Not set");
        row.set_tooltip_text(None);
        clear.set_visible(false);
    } else {
        let name = std::path::Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.to_string());
        row.set_subtitle(&glib::markup_escape_text(&name));
        row.set_tooltip_text(Some(path));
        clear.set_visible(true);
    }
}
