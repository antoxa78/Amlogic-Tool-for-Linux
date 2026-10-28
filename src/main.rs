use glib::ExitCode;
use std::cell::RefCell;
use std::rc::Rc;
use gtk4::prelude::ApplicationExt;
use gtk4::prelude::ApplicationExtManual;
use gtk4::prelude::FileExt;
use gtk4::prelude::GtkWindowExt;
use gtk4::prelude::WidgetExt;

mod app;
mod backend;
mod ui;

/// CSS injected at the highest possible priority to override libadwaita's
/// own titlebutton spacing.  `STYLE_PROVIDER_PRIORITY_USER` (800) beats
/// libadwaita's application stylesheet (600) and the GTK default (400).
const TITLEBUTTON_CSS: &str = "
windowcontrols {
    padding: 0 4px;
    margin: 0;
    border-spacing: 4px;
}
windowcontrols > button,
windowcontrols > button.titlebutton {
    margin: 0 2px;
    padding: 6px;
    min-width: 16px;
    min-height: 16px;
    border-radius: 50%;
}
windowcontrols > button > image,
windowcontrols > button.titlebutton > image {
    margin: 0;
    padding: 0;
    -gtk-icon-size: 16px;
}
";

fn main() -> ExitCode {
    // Force client-side decorations before GTK initializes
    // (Cinnamon/Muffin defaults to SSD which bypasses our CSS)
    unsafe {
        std::env::set_var("GTK_CSD", "1");
    }

    libadwaita::init().expect("Failed to init libadwaita");

    backend::image::cleanup_stale_tmp();

    // Register compiled GLib resources (includes icon.png)
    gtk4::gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    let app = libadwaita::Application::builder()
        .application_id("com.amlflashtool.gui")
        // The .desktop file passes files (%U): accept them instead of
        // failing with "This application can not open files".
        .flags(gtk4::gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let main_window: Rc<RefCell<Option<app::window::MainWindow>>> = Rc::new(RefCell::new(None));

    {
        let main_window = main_window.clone();
        app.connect_activate(move |app| {
            if let Some(mw) = main_window.borrow().as_ref() {
                mw.window.present();
                return;
            }
            let mw = build_window(app);
            mw.window.present();
            *main_window.borrow_mut() = Some(mw);
        });
    }

    {
        let main_window = main_window.clone();
        app.connect_open(move |app, files, _hint| {
            app.activate();
            if let (Some(mw), Some(path)) = (
                main_window.borrow().as_ref(),
                files.iter().find_map(|f| f.path()),
            ) {
                mw.open_file(path);
            }
        });
    }

    app.run()
}

fn build_window(app: &libadwaita::Application) -> app::window::MainWindow {
    let mw = app::window::MainWindow::new(app);
    let display = mw.window.display();

    // Register icon resource path so icon_name("aml-flash-tool") resolves
    gtk4::IconTheme::for_display(&display).add_resource_path("/com/amlflashtool/gui/icons");

    // Load app CSS
    let css = gtk4::CssProvider::new();
    css.load_from_string(include_str!("style.css"));
    gtk4::style_context_add_provider_for_display(&display, &css, gtk4::STYLE_PROVIDER_PRIORITY_USER);

    // Titlebutton overrides as a separate provider so they are evaluated last
    // and win the cascade over libadwaita's own stylesheet.
    let btn_css = gtk4::CssProvider::new();
    btn_css.load_from_string(TITLEBUTTON_CSS);
    gtk4::style_context_add_provider_for_display(
        &display,
        &btn_css,
        gtk4::STYLE_PROVIDER_PRIORITY_USER,
    );

    // Force client-side decorations (Cinnamon defaults to SSD)
    mw.window.set_decorated(false);
    mw
}
