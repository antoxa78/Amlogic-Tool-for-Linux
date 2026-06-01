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
                let label = gtk4::Label::builder()
                    .halign(gtk4::Align::Start)
                    .margin_start(8)
                    .margin_end(8)
                    .margin_top(4)
                    .margin_bottom(4)
                    .build();
                item.set_child(Some(&label));
            }
        });

        factory.connect_bind(|_, list_item| {
            if let Some(item) = list_item.downcast_ref::<gtk4::ListItem>() {
                if let Some(obj) = item.item().and_downcast::<gtk4::StringObject>() {
                    if let Some(label) = item.child().and_downcast::<gtk4::Label>() {
                        label.set_text(&obj.string());
                    }
                }
            }
        });

        gtk4::ColumnViewColumn::builder()
            .title("HUB / Port")
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
                    .margin_top(4)
                    .margin_bottom(4)
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
                                pb.set_text(Some(&format!(
                                    "Burning {:.0}%",
                                    pct * 100.0
                                )));
                                pb.add_css_class("status-burning");
                            }
                            BurnStatus::Success => {
                                pb.set_fraction(1.0);
                                pb.set_text(Some("Success"));
                                pb.add_css_class("status-success");
                            }
                            BurnStatus::Error(e) => {
                                pb.set_fraction(0.0);
                                pb.set_text(Some(&format!("Error: {e}")));
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
            *devices = new_devices;
        }

        self.store.remove_all();
        for d in self.devices.borrow().iter() {
            let state = d.borrow();
            self.store
                .append(&gtk4::StringObject::new(&state.hub_id));
        }
    }

    pub fn device_count(&self) -> usize {
        self.devices.borrow().len()
    }

    pub fn refresh_display(&self) {
        self.store.remove_all();
        for d in self.devices.borrow().iter() {
            let state = d.borrow();
            self.store
                .append(&gtk4::StringObject::new(&state.hub_id));
        }
    }
}
