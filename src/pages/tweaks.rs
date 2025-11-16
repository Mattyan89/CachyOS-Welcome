use crate::systemd_units::SystemdUnits;
use crate::ui::{MessageType, UI};
use crate::{fl, systemd_units, utils};

use std::str;
use std::sync::Mutex;

use gtk::prelude::*;

use glib::translate::FromGlib;
use gtk::glib;
use once_cell::sync::Lazy;
use subprocess::Exec;
use tokio::runtime::Runtime;
use tracing::error;

#[macro_export]
macro_rules! create_tweak_checkbox {
    ($tweak_msg:literal,$action_data:literal,$action_type:literal,$alpm_pkg_name:literal) => {{
        let temp_btn =
            gtk::CheckButton::with_label(&fl!("tweak-enabled-title", tweak = $tweak_msg));
        temp_btn.set_widget_name($tweak_msg);

        set_tweak_check_data(&temp_btn, $action_data, $action_type, $alpm_pkg_name);
        connect_tweak(&temp_btn, $action_data);
        temp_btn
    }};
}

static G_LOCAL_UNITS: Lazy<Mutex<SystemdUnits>> = Lazy::new(|| Mutex::new(SystemdUnits::new()));
static G_GLOBAL_UNITS: Lazy<Mutex<SystemdUnits>> = Lazy::new(|| Mutex::new(SystemdUnits::new()));

pub(crate) fn load_enabled_units() {
    G_LOCAL_UNITS.lock().unwrap().enabled_units.clear();

    let rt = Runtime::new().expect("Failed to initialize tokio runtime");
    let res = rt.block_on(async move {
        let units = systemd_units::get_enabled_global_units().await?;
        G_LOCAL_UNITS.lock().unwrap().enabled_units = units;

        anyhow::Ok(())
    });

    if let Err(res_err) = res {
        error!("Failed to load systemd units: {res_err}");
    }
}

pub(crate) fn load_global_enabled_units() {
    G_GLOBAL_UNITS.lock().unwrap().enabled_units.clear();

    let rt = Runtime::new().expect("Failed to initialize tokio runtime");
    let res = rt.block_on(async move {
        let units = systemd_units::get_enabled_user_units().await?;
        G_GLOBAL_UNITS.lock().unwrap().enabled_units = units;

        anyhow::Ok(())
    });

    if let Err(res_err) = res {
        error!("Failed to load user systemd units: {res_err}");
    }
}

fn set_tweak_check_data(
    check_btn: &gtk::CheckButton,
    action_data: &'static str,
    action_type: &'static str,
    alpm_package_name: &'static str,
) {
    unsafe {
        check_btn.set_data("actionData", action_data);
        check_btn.set_data("actionType", action_type);
        check_btn.set_data("alpmPackage", alpm_package_name);
    }
}

fn connect_tweak(check_btn: &gtk::CheckButton, action_data: &'static str) {
    let action_data_str = action_data.to_owned();
    if G_LOCAL_UNITS.lock().unwrap().enabled_units.contains(&action_data_str)
        || G_GLOBAL_UNITS.lock().unwrap().enabled_units.contains(&action_data_str)
    {
        check_btn.set_active(true);
    }
    connect_clicked_and_save(check_btn, on_servbtn_clicked);
}

pub(crate) fn create_options_section() -> gtk::Box {
    let topbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let box_collection = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let box_collection_s = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    let label = gtk::Label::new(None);
    label.set_line_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_text(&fl!("tweaks"));

    let psd_btn = create_tweak_checkbox!(
        "Profile-sync-daemon",
        "psd.service",
        "user_service",
        "profile-sync-daemon"
    );
    let systemd_oomd_btn =
        create_tweak_checkbox!("Systemd-oomd", "systemd-oomd.service", "service", "");
    let bpftune_btn =
        create_tweak_checkbox!("Bpftune", "bpftune.service", "service", "bpftune-git");
    let bluetooth_btn =
        create_tweak_checkbox!("Bluetooth", "bluetooth.service", "service", "bluez");
    let ananicy_cpp_btn =
        create_tweak_checkbox!("Ananicy Cpp", "ananicy-cpp.service", "service", "ananicy-cpp");
    let cachy_update_btn = create_tweak_checkbox!(
        "Cachy Update",
        "arch-update.timer arch-update-tray.service",
        "user_service",
        "cachy-update"
    );

    // set tooltips
    psd_btn.set_tooltip_text(Some(&fl!("tweak-psd-tooltip")));
    systemd_oomd_btn.set_tooltip_text(Some(&fl!("tweak-oomd-tooltip")));
    bpftune_btn.set_tooltip_text(Some(&fl!("tweak-bpftune-tooltip")));
    bluetooth_btn.set_tooltip_text(Some(&fl!("tweak-bluetooth-tooltip")));
    ananicy_cpp_btn.set_tooltip_text(Some(&fl!("tweak-ananicycpp-tooltip")));
    cachy_update_btn.set_tooltip_text(Some(&fl!("tweak-cachyupdate-tooltip")));

    topbox.pack_start(&label, true, false, 1);
    box_collection.pack_start(&psd_btn, true, false, 2);
    box_collection_s.pack_start(&systemd_oomd_btn, true, false, 2);
    box_collection_s.pack_start(&bpftune_btn, true, false, 2);
    box_collection.pack_start(&ananicy_cpp_btn, true, false, 2);
    box_collection.pack_start(&cachy_update_btn, true, false, 2);
    box_collection_s.pack_start(&bluetooth_btn, true, false, 2);
    box_collection.set_halign(gtk::Align::Fill);
    box_collection_s.set_halign(gtk::Align::Fill);
    topbox.pack_end(&box_collection_s, true, false, 1);
    topbox.pack_end(&box_collection, true, false, 1);

    topbox.set_hexpand(true);
    topbox
}

fn toggle_service(
    action_type: &str,
    action_data: &str,
    alpm_package_name: &str,
    widget_window: gtk::Window,
    callback: std::boxed::Box<dyn Fn(bool)>,
) {
    let units_handle = if action_type == "user_service" { &G_GLOBAL_UNITS } else { &G_LOCAL_UNITS }
        .lock()
        .unwrap();

    let action_enabled =
        action_data.split(' ').all(|x| units_handle.enabled_units.contains(&x.to_owned()));
    let cmd = if !action_enabled {
        if action_type == "user_service" {
            format!("systemctl --user enable --now --force {action_data}")
        } else {
            format!("/sbin/pkexec bash -c \"systemctl enable --now --force {action_data}\"")
        }
    } else if action_type == "user_service" {
        format!("systemctl --user disable --now {action_data}")
    } else {
        format!("/sbin/pkexec bash -c \"systemctl disable --now {action_data}\"")
    };

    // Create context channel.
    let (tx, rx) = glib::MainContext::channel(glib::Priority::default());

    let dialog_text = fl!("package-not-installed", package_name = alpm_package_name);

    let action_type = action_type.to_owned();
    let alpm_package_name = alpm_package_name.to_owned();
    // Spawn child process in separate thread.
    std::thread::spawn(move || {
        if !alpm_package_name.is_empty() {
            if !utils::is_alpm_pkg_installed(&alpm_package_name) {
                let _ = utils::run_cmd_terminal(
                    crate::gui::run_command,
                    format!("pacman -S {alpm_package_name}"),
                    true,
                );
            }
            if !utils::is_alpm_pkg_installed(&alpm_package_name) {
                tx.send(false).expect("Couldn't send data to channel");
                return;
            }
        }
        Exec::shell(cmd).join().unwrap();

        if action_type == "user_service" {
            load_global_enabled_units();
        } else {
            load_enabled_units();
        }
    });

    rx.attach(None, move |msg| {
        if !msg {
            callback(msg);

            let ui_comp = crate::gui::GUI::new(widget_window.clone());
            ui_comp.show_message(MessageType::Error, &dialog_text, "Error".to_string());
        }
        glib::ControlFlow::Continue
    });
}

fn on_servbtn_clicked(button: &gtk::CheckButton) {
    // Get action data/type.
    let action_type: &str;
    let action_data: &str;
    let alpm_package_name: &str;
    let signal_handler: u64;
    unsafe {
        action_type = *button.data("actionType").unwrap().as_ptr();
        action_data = *button.data("actionData").unwrap().as_ptr();
        alpm_package_name = *button.data("alpmPackage").unwrap().as_ptr();
        signal_handler = *button.data("signalHandle").unwrap().as_ptr();
    }

    let widget_window = utils::get_window_from_widget(button).expect("Failed to retrieve window");

    let button_sh = button.clone();
    toggle_service(
        action_type,
        action_data,
        alpm_package_name,
        widget_window,
        Box::new(move |msg| {
            let sighandle_id_obj =
                unsafe { glib::signal::SignalHandlerId::from_glib(signal_handler) };
            button_sh.block_signal(&sighandle_id_obj);
            button_sh.set_active(msg);
            button_sh.unblock_signal(&sighandle_id_obj);
        }),
    );
}

fn connect_clicked_and_save<F>(passed_btn: &gtk::CheckButton, callback: F)
where
    F: Fn(&gtk::CheckButton) + 'static,
{
    let sighandle_id = passed_btn.connect_clicked(callback);
    unsafe {
        passed_btn.set_data("signalHandle", sighandle_id.as_raw());
    }
}
