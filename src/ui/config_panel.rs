use libadwaita::prelude::ActionRowExt;
use libadwaita::prelude::PreferencesGroupExt;
use libadwaita::prelude::PreferencesPageExt;
use std::cell::RefCell;

static SOCS: &[&str] = &[
    "gxl (S905X/S905W/S912)",
    "g12a (S905D2/S905X2)",
    "g12b (S922X/A311D, uses g12a protocol)",
    "sm1 (S905D3/S905X3, uses g12a protocol)",
    "t5d (T5D)",
    "t7 (T7)",
    "s4 (S4)",
    "s5 (S5)",
    "axg (A113X)",
    "a1 (A1)",
    "m8 (S805/A111)",
    "txlx (T962E)",
    "sc2 (S905X4)",
    "t3 (T3)",
];

pub struct ConfigPanel {
    pub page: libadwaita::PreferencesPage,
    pub reset_switch: gtk4::Switch,
    pub erase_flash_switch: gtk4::Switch,
    pub erase_bootloader_switch: gtk4::Switch,
    soc_dropdown: gtk4::DropDown,
    password_path: RefCell<String>,
    efuse_path: RefCell<String>,
    image_path: RefCell<String>,
}

impl ConfigPanel {
    pub fn new() -> Self {
        let page = libadwaita::PreferencesPage::new();

        // ── Options group ──────────────────────────────────────────────────
        let options_group = libadwaita::PreferencesGroup::builder()
            .title("Options")
            .build();

        let erase_flash_row = libadwaita::ActionRow::builder()
            .title("Erase Flash")
            .subtitle("Wipe all data before burning")
            .build();
        let erase_flash_switch = gtk4::Switch::builder().valign(gtk4::Align::Center).build();
        erase_flash_row.add_suffix(&erase_flash_switch);
        erase_flash_row.set_activatable_widget(Some(&erase_flash_switch));
        options_group.add(&erase_flash_row);

        let erase_boot_row = libadwaita::ActionRow::builder()
            .title("Erase Bootloader")
            .subtitle("Remove bootloader (may brick device)")
            .build();
        let erase_bootloader_switch = gtk4::Switch::builder().valign(gtk4::Align::Center).build();
        erase_boot_row.add_suffix(&erase_bootloader_switch);
        erase_boot_row.set_activatable_widget(Some(&erase_bootloader_switch));
        options_group.add(&erase_boot_row);

        let reset_row = libadwaita::ActionRow::builder()
            .title("Reset After Burn")
            .subtitle("Automatically reboot device when done")
            .build();
        let reset_switch = gtk4::Switch::builder()
            .valign(gtk4::Align::Center)
            .active(true)
            .build();
        reset_row.add_suffix(&reset_switch);
        reset_row.set_activatable_widget(Some(&reset_switch));
        options_group.add(&reset_row);

        // ── SoC group ──────────────────────────────────────────────────────
        let soc_group = libadwaita::PreferencesGroup::builder()
            .title("SOC Type")
            .build();

        let soc_model = gtk4::StringList::new(SOCS);
        let soc_dropdown = gtk4::DropDown::builder()
            .model(&soc_model)
            .selected(0u32)
            .valign(gtk4::Align::Center)
            .build();
        let soc_row = libadwaita::ActionRow::builder().title("SoC").build();
        soc_row.add_suffix(&soc_dropdown);
        soc_row.set_activatable_widget(Some(&soc_dropdown));
        soc_group.add(&soc_row);

        page.add(&options_group);
        page.add(&soc_group);

        Self {
            page,
            reset_switch,
            erase_flash_switch,
            erase_bootloader_switch,
            soc_dropdown,
            password_path: RefCell::new(String::new()),
            efuse_path: RefCell::new(String::new()),
            image_path: RefCell::new(String::new()),
        }
    }

    pub fn set_image_path(&self, path: &str) {
        *self.image_path.borrow_mut() = path.to_string();
    }

    pub fn image_path(&self) -> String {
        self.image_path.borrow().clone()
    }

    pub fn set_soc(&self, soc: &str) {
        let lower = soc.to_lowercase();
        if let Some(pos) = SOCS
            .iter()
            .position(|s| s.to_lowercase().starts_with(&lower))
        {
            self.soc_dropdown.set_selected(pos as u32);
        }
    }

    pub fn soc_raw(&self) -> String {
        let pos = self.soc_dropdown.selected();
        let entry = SOCS.get(pos as usize).unwrap_or(&"gxl");
        entry.split_whitespace().next().unwrap_or("gxl").to_string()
    }

    pub fn soc(&self) -> String {
        match self.soc_raw().as_str() {
            "g12b" | "sm1" => "g12a".to_string(),
            other => other.to_string(),
        }
    }

    pub fn set_password_path(&self, path: &str) {
        *self.password_path.borrow_mut() = path.to_string();
    }

    pub fn password_path(&self) -> String {
        self.password_path.borrow().clone()
    }

    pub fn set_efuse_path(&self, path: &str) {
        *self.efuse_path.borrow_mut() = path.to_string();
    }

    pub fn efuse_path(&self) -> String {
        self.efuse_path.borrow().clone()
    }
}
