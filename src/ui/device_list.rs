use crate::backend::types::*;
use gtk4::gio;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub struct DeviceListView {
    pub column_view: gtk4::ColumnView,
    store: gio::ListStore,
    pub devices: Rc<RefCell<Vec<Rc<RefCell<DeviceState>>>>>,
}

impl DeviceListView {
    pub fn new() -> Self {
        let store = gio::ListStore::new::<gtk4::StringObject>();
        let model = gtk4::NoSelection::new(Some(store.clone()));
        let column_view = gtk4::ColumnView::new(Some(model));
        column_view.set_vexpand(true);
        column_view.set_hexpand(true);

        let hub_col = Self::hub_column();
        column_view.append_column(&hub_col);

        let devices = Rc::new(RefCell::new(Vec::new()));

        let status_col = Self::status_column(devices.clone());
        column_view.append_column(&status_col);

        Self {
            column_view,
            store,
            devices,
        }
    }

    fn hub_column() -> gtk4::ColumnViewColumn {
        let factory = gtk4::SignalListItemFactory::new();

        factory.connect_setup(|_, list_item| {
            if let Some(item) = list_item.downcast_ref::<gtk4::ListItem>() {
                let row = gtk4::Box::builder()
                    .spacing(10)
                    .margin_start(8)
                    .margin_end(8)
                    .margin_top(6)
                    .margin_bottom(6)
                    .build();
                row.append(&gtk4::Image::from_icon_name("drive-removable-media-symbolic"));
                let label = gtk4::Label::builder()
                    .halign(gtk4::Align::Start)
                    .ellipsize(gtk4::pango::EllipsizeMode::End)
                    .css_classes(["device-id"])
                    .build();
                row.append(&label);
                item.set_child(Some(&row));
            }
        });

        factory.connect_bind(|_, list_item| {
            if let Some(item) = list_item.downcast_ref::<gtk4::ListItem>() {
                if let Some(obj) = item.item().and_downcast::<gtk4::StringObject>() {
                    if let Some(label) = item
                        .child()
                        .and_then(|c| c.last_child())
                        .and_downcast::<gtk4::Label>()
                    {
                        label.set_text(&obj.string());
                        label.set_tooltip_text(Some(&obj.string()));
                    }
                }
            }
        });

        gtk4::ColumnViewColumn::builder()
            .title("Device")
            .factory(&factory)
            .resizable(true)
            .build()
    }

    fn status_column(
        devices: Rc<RefCell<Vec<Rc<RefCell<DeviceState>>>>>,
    ) -> gtk4::ColumnViewColumn {
        let factory = gtk4::SignalListItemFactory::new();
        let devs = devices.clone();

        factory.connect_setup(|_, list_item| {
            if let Some(item) = list_item.downcast_ref::<gtk4::ListItem>() {
                let pb = gtk4::ProgressBar::builder()
                    .hexpand(true)
                    .show_text(true)
                    .margin_start(8)
                    .margin_end(8)
                    .valign(gtk4::Align::Center)
                    .build();
                item.set_child(Some(&pb));
            }
        });

        factory.connect_bind(move |_, list_item| {
            if let Some(item) = list_item.downcast_ref::<gtk4::ListItem>() {
                if let Some(pb) = item.child().and_downcast::<gtk4::ProgressBar>() {
                    let pos = item.position();
                    let state = devs.borrow().get(pos as usize).cloned();
                    pb.remove_css_class("status-ready");
                    pb.remove_css_class("status-burning");
                    pb.remove_css_class("status-success");
                    pb.remove_css_class("status-error");
                    pb.set_tooltip_text(None);
                    if let Some(dev_rc) = state {
                        let dev = dev_rc.borrow();
                        match &dev.status {
                            BurnStatus::Disconnected => {
                                pb.set_fraction(0.0);
                                pb.set_text(Some("Disconnected"));
                                pb.add_css_class("status-ready");
                            }
                            BurnStatus::Ready => {
                                pb.set_fraction(0.0);
                                pb.set_text(Some("Ready"));
                                pb.add_css_class("status-ready");
                            }
                            BurnStatus::Burning(pct) => {
                                pb.set_fraction(*pct);
                                pb.set_text(Some(&format!("Burning {:.0}%", pct * 100.0)));
                                pb.add_css_class("status-burning");
                            }
                            BurnStatus::Success => {
                                pb.set_fraction(1.0);
                                pb.set_text(Some("Success"));
                                pb.add_css_class("status-success");
                            }
                            BurnStatus::Error(e) => {
                                pb.set_fraction(0.0);
                                pb.set_text(Some(&format!("Failed — {e}")));
                                pb.set_tooltip_text(Some(e));
                                pb.add_css_class("status-error");
                            }
                        }
                    }
                }
            }
        });

        gtk4::ColumnViewColumn::builder()
            .title("Status")
            .factory(&factory)
            .resizable(true)
            .expand(true)
            .build()
    }

    pub fn update_devices(&self, device_strings: &[String]) {
        {
            let mut devices = self.devices.borrow_mut();
            let mut new_devices: Vec<Rc<RefCell<DeviceState>>> = Vec::new();
            for s in device_strings {
                let hub_id = s.split(" | ").next().unwrap_or(s).to_string();
                if let Some(existing) = devices.iter().find(|d| d.borrow().hub_id == hub_id) {
                    new_devices.push(existing.clone());
                } else {
                    new_devices.push(Rc::new(RefCell::new(DeviceState {
                        hub_id,
                        serial: String::new(),
                        status: BurnStatus::Ready,
                        cancel_tx: None,
                    })));
                }
            }
            if device_strings.is_empty() {
                for d in devices.iter() {
                    if matches!(d.borrow().status, BurnStatus::Burning(_)) {
                        new_devices.push(d.clone());
                    }
                }
            }
            *devices = new_devices;
        }

        let ids: Vec<String> = self
            .devices
            .borrow()
            .iter()
            .map(|d| d.borrow().hub_id.clone())
            .collect();
        let current: Vec<String> = (0..self.store.n_items())
            .filter_map(|i| self.store.item(i).and_downcast::<gtk4::StringObject>())
            .map(|o| o.string().to_string())
            .collect();
        if ids != current {
            self.store.remove_all();
            for id in &ids {
                self.store.append(&gtk4::StringObject::new(id));
            }
        }
    }

    pub fn device_count(&self) -> usize {
        self.devices.borrow().len()
    }

    /// Re-bind every row so status/progress changes show up, without
    /// Re-uses the model (one splice) rather than clearing it.
    pub fn refresh_display(&self) {
        let n = self.store.n_items();
        if n as usize != self.device_count() {
            self.store.remove_all();
            for d in self.devices.borrow().iter() {
                self.store
                    .append(&gtk4::StringObject::new(&d.borrow().hub_id));
            }
        } else if n > 0 {
            // GTK skips re-binding rows whose item object is unchanged, so
            // swap in fresh objects to force the status column to redraw.
            let fresh: Vec<gtk4::StringObject> = self
                .devices
                .borrow()
                .iter()
                .map(|d| gtk4::StringObject::new(&d.borrow().hub_id))
                .collect();
            self.store.splice(0, n, &fresh);
        }
    }

    pub fn any_burning(&self) -> bool {
        self.devices
            .borrow()
            .iter()
            .any(|d| matches!(d.borrow().status, BurnStatus::Burning(_)))
    }

    /// Devices that finished (OK or failed) and are still attached become
    /// eligible for another run.
    pub fn reset_finished(&self) {
        for d in self.devices.borrow().iter() {
            let mut dev = d.borrow_mut();
            if matches!(dev.status, BurnStatus::Success | BurnStatus::Error(_)) {
                dev.status = BurnStatus::Ready;
            }
        }
        self.refresh_display();
    }
}
