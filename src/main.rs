use glib::prelude::*;
use glib::ExitCode;
use gtk4::prelude::ApplicationExt;
use gtk4::prelude::ApplicationExtManual;
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
    padding: 0;
    margin: 0;
    border-spacing: 0;
}
windowcontrols > button,
windowcontrols > button.titlebutton {
    margin: 0;
    padding: 4px;
    min-width: 0;
    min-height: 0;
    border-radius: 50%;
}
windowcontrols > button > image,
windowcontrols > button.titlebutton > image {
    margin: 0;
    padding: 0;
}
";

fn main() -> ExitCode {
    // Force client-side decorations before GTK initializes
    // (Cinnamon/Muffin defaults to SSD which bypasses our CSS)
    unsafe { std::env::set_var("GTK_CSD", "1"); }

    libadwaita::init().expect("Failed to init libadwaita");

    backend::image::cleanup_stale_tmp();

    // Register compiled GLib resources (includes icon.png)
    gtk4::gio::resources_register_include!("resources.gresource")
        .expect("Failed to register resources");

    let app = libadwaita::Application::builder()
        .application_id("com.amlflashtool.gui")
        .build();

    app.connect_activate(|app| {
        let mw = app::window::MainWindow::new(app);

        // Load app CSS
        let css = gtk4::CssProvider::new();
        css.load_from_string(include_str!("style.css"));
        let display = mw.window.display();
        gtk4::style_context_add_provider_for_display(
            &display,
            &css,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );

        // Load titlebutton override CSS at the same priority, as a separate
        // provider so it is guaranteed to be evaluated last and wins the
        // cascade over libadwaita's own stylesheet.
        let btn_css = gtk4::CssProvider::new();
        btn_css.load_from_string(TITLEBUTTON_CSS);
        gtk4::style_context_add_provider_for_display(
            &display,
            &btn_css,
            gtk4::STYLE_PROVIDER_PRIORITY_USER,
        );

        // Force client-side decorations (Cinnamon defaults to SSD)
        mw.window.set_decorated(false);

        // First programmatic pass: runs once the widget tree is realized
        // (more reliable than idle_add_local which can fire before layout).
        let window_ref = mw.window.clone();
        mw.window.connect_realize(move |_| {
            tighten_windowcontrols(&window_ref);
        });

        // Second pass after map (window is on-screen, all sizes are known)
        let window_ref = mw.window.clone();
        mw.window.connect_map(move |_| {
            tighten_windowcontrols(&window_ref);
        });

        // Final safety pass 1 s after map, in case libadwaita re-styles late
        let window_ref = mw.window.clone();
        glib::timeout_add_seconds_local(1, move || {
            tighten_windowcontrols(&window_ref);
            glib::ControlFlow::Break
        });

        mw.window.present();
    });

    app.run()
}

/// Walk the entire widget tree from `root` and, for every `windowcontrols`
/// node found:
///   1. Set `BoxLayout` spacing to 0.
///   2. Zero all margins on each direct child button.
///   3. Zero all margins on each grandchild (icon images inside the buttons).
fn tighten_windowcontrols(root: &impl IsA<gtk4::Widget>) {
    fn walk(widget: &gtk4::Widget) {
        if widget.css_name() == "windowcontrols" {
            // 1. Kill BoxLayout spacing
            let lm: glib::Object = widget.property("layout-manager");
            if let Some(bl) = lm.downcast_ref::<gtk4::BoxLayout>() {
                bl.set_spacing(0);
            }

            // 2 & 3. Kill margins on buttons and their icon children
            let mut child = widget.first_child();
            while let Some(btn) = child {
                btn.set_margin_start(0);
                btn.set_margin_end(0);
                btn.set_margin_top(0);
                btn.set_margin_bottom(0);

                let mut inner = btn.first_child();
                while let Some(ic) = inner {
                    ic.set_margin_start(0);
                    ic.set_margin_end(0);
                    ic.set_margin_top(0);
                    ic.set_margin_bottom(0);
                    inner = ic.next_sibling();
                }

                child = btn.next_sibling();
            }
            // Don't return early — there may be more windowcontrols nodes
            // (e.g. one on each side of the header bar).
        }

        let mut child = widget.first_child();
        while let Some(w) = child {
            walk(&w);
            child = w.next_sibling();
        }
    }
    walk(root.upcast_ref::<gtk4::Widget>());
}
