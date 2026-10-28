use gtk4::gio;
use gtk4::prelude::*;
use std::path::PathBuf;

pub fn show_image_chooser<F>(parent: &impl IsA<gtk4::Window>, callback: F)
where
    F: FnOnce(Option<PathBuf>) + 'static,
{
    let dialog = gtk4::FileDialog::builder()
        .title("Select Amlogic Firmware Image")
        .accept_label("Open")
        .modal(true)
        .build();

    let filter = gtk4::FileFilter::new();
    filter.add_pattern("*.img");
    filter.add_pattern("*.IMG");
    filter.set_name(Some("Amlogic firmware (*.img)"));
    let all = gtk4::FileFilter::new();
    all.add_pattern("*");
    all.set_name(Some("All files"));
    let filters = gio::ListStore::new::<gtk4::FileFilter>();
    filters.append(&filter);
    filters.append(&all);
    dialog.set_filters(Some(&filters));
    dialog.set_default_filter(Some(&filter));

    dialog.open(
        Some(parent),
        Option::<&gio::Cancellable>::None,
        move |result| match result {
            Ok(file) => callback(file.path().map(|p| p.to_path_buf())),
            Err(_) => callback(None),
        },
    );
}

pub fn show_file_dialog<F>(
    parent: &impl IsA<gtk4::Window>,
    title: &str,
    accept_label: &str,
    callback: F,
) where
    F: FnOnce(Option<PathBuf>) + 'static,
{
    let dialog = gtk4::FileDialog::builder()
        .title(title)
        .accept_label(accept_label)
        .modal(true)
        .build();

    dialog.open(
        Some(parent),
        Option::<&gio::Cancellable>::None,
        move |result| match result {
            Ok(file) => callback(file.path().map(|p| p.to_path_buf())),
            Err(_) => callback(None),
        },
    );
}

pub fn show_save_dialog<F>(parent: &impl IsA<gtk4::Window>, name: &str, callback: F)
where
    F: FnOnce(Option<PathBuf>) + 'static,
{
    let dialog = gtk4::FileDialog::builder()
        .title("Save File")
        .accept_label("Save")
        .modal(true)
        .initial_name(name)
        .build();

    dialog.save(
        Some(parent),
        Option::<&gio::Cancellable>::None,
        move |result| match result {
            Ok(file) => callback(file.path().map(|p| p.to_path_buf())),
            Err(_) => callback(None),
        },
    );
}

pub fn show_confirm_dialog<W, F>(
    parent: &W,
    title: &str,
    body: &str,
    confirm_label: &str,
    callback: F,
)
where
    W: IsA<gtk4::Window> + IsA<gtk4::Widget>,
    F: FnOnce(bool) + 'static,
{
    use libadwaita::prelude::AlertDialogExt;
    use libadwaita::prelude::AlertDialogExtManual;

    let dialog = libadwaita::AlertDialog::new(Some(title), Some(body));
    dialog.add_response("cancel", "Cancel");
    dialog.add_response("confirm", confirm_label);
    dialog.set_response_appearance("confirm", libadwaita::ResponseAppearance::Destructive);
    dialog.set_default_response(Some("cancel"));
    dialog.set_close_response("cancel");

    dialog.choose(parent, Option::<&gio::Cancellable>::None, move |response| {
        callback(response == "confirm");
    });
}
