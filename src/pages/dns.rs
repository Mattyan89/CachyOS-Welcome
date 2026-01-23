use crate::ui::UI;
use crate::{actions, create_gtk_button, dns, fl, utils};

use gtk::prelude::*;

use gtk::{glib, Builder};

fn selection_index_for_connection(conn_name: &str) -> usize {
    if let Some((ipv4_dns, ipv6_dns)) = actions::get_dns_for_connection(conn_name) {
        for (key_index, (_name, (ipv4_map, ipv6_map))) in dns::G_DNS_SERVERS.entries().enumerate() {
            if (!ipv4_dns.is_empty() && &ipv4_dns == ipv4_map)
                || (!ipv6_dns.is_empty() && &ipv6_dns == ipv6_map)
            {
                return key_index;
            }
        }
    }

    // fallback to Cloudflare
    dns::G_DNS_SERVERS.get_index("Cloudflare").unwrap()
}

fn create_connections_section() -> gtk::Box {
    let topbox = gtk::Box::new(gtk::Orientation::Vertical, 2);
    let connection_box = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let dnsservers_box = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 2);
    let label = gtk::Label::new(None);
    label.set_line_wrap(true);
    label.set_justify(gtk::Justification::Center);
    label.set_text(&fl!("dns-settings"));

    let connections_label = gtk::Label::new(None);
    connections_label.set_justify(gtk::Justification::Left);
    connections_label.set_text(&fl!("select-connection"));
    connections_label.set_widget_name("select-connection");
    let servers_label = gtk::Label::new(None);
    servers_label.set_justify(gtk::Justification::Left);
    servers_label.set_text(&fl!("select-dns-server"));
    servers_label.set_widget_name("select-dns-server");
    let apply_btn = create_gtk_button!("apply");
    let reset_btn = create_gtk_button!("reset");

    let combo_conn = {
        let store = gtk::ListStore::new(&[String::static_type()]);
        let nm_connections = actions::get_nm_connections();
        for nm_connection in &nm_connections {
            store.set(&store.append(), &[(0, nm_connection)]);
        }
        utils::create_combo_with_model(&store)
    };
    let combo_servers = {
        let store = gtk::ListStore::new(&[String::static_type()]);
        for dns_server in dns::G_DNS_SERVERS.keys() {
            store.set(&store.append(), &[(0, dns_server)]);
        }
        utils::create_combo_with_model(&store)
    };

    combo_conn.set_widget_name("connections_combo");
    combo_servers.set_widget_name("servers_combo");

    // preset the current active connection
    if let Some(active_conn_name) = actions::get_active_connection_name() {
        let model = combo_conn.model().unwrap();
        if let Some(iter) = utils::find_iter_in_model(&model, &active_conn_name) {
            combo_conn.set_active_iter(Some(&iter));

            let selected_dns_index = selection_index_for_connection(&active_conn_name);
            combo_servers.set_active(Some(selected_dns_index as u32));
        }
    }

    // select used dns option value on connection change
    let combo_servers_clone = combo_servers.clone();
    combo_conn.connect_changed(move |combo| {
        // use empty string which will trigger fallback
        let conn_name: String = combo.active_text().map(Into::into).unwrap_or_default();

        let selected_dns_index = selection_index_for_connection(&conn_name);
        combo_servers_clone.set_active(Some(selected_dns_index as u32));
    });

    // Create context channel.
    let (dialog_tx, dialog_rx) = glib::MainContext::channel(glib::Priority::default());

    // Connect signals.
    let dialog_tx_clone = dialog_tx.clone();
    let combo_conn_clone = combo_conn.clone();
    let combo_serv_clone = combo_servers.clone();
    apply_btn.connect_clicked(move |_| {
        let conn_name: String = combo_conn_clone.active_text().map(Into::into).unwrap_or_default();
        let server_name: String =
            combo_serv_clone.active_text().map(Into::into).unwrap_or_default();
        let server_addr = dns::G_DNS_SERVERS.get(&server_name).unwrap();

        let dialog_tx_clone = dialog_tx_clone.clone();
        std::thread::spawn(move || {
            actions::change_dns_server(&conn_name, server_addr.0, server_addr.1, dialog_tx_clone);
        });
    });
    let dialog_tx_clone = dialog_tx.clone();
    let combo_conn_clone = combo_conn.clone();
    reset_btn.connect_clicked(move |_| {
        let dialog_tx_clone = dialog_tx_clone.clone();
        let conn_name: String = combo_conn_clone.active_text().map(Into::into).unwrap_or_default();
        std::thread::spawn(move || {
            actions::reset_dns_server(&conn_name, dialog_tx_clone);
        });
    });

    // Setup receiver
    let apply_btn_clone = apply_btn.clone();
    dialog_rx.attach(None, move |msg| {
        let widget_obj = &apply_btn_clone;
        let widget_window =
            utils::get_window_from_widget(widget_obj).expect("Failed to retrieve window");
        let ui_comp = crate::gui::GUI::new(widget_window);

        ui_comp.show_message(msg.msg_type, &msg.msg, msg.msg_type.to_string());
        glib::ControlFlow::Continue
    });

    topbox.pack_start(&label, true, false, 1);
    connection_box.pack_start(&connections_label, true, true, 2);
    connection_box.pack_end(&combo_conn, true, true, 2);
    dnsservers_box.pack_start(&servers_label, true, true, 2);
    dnsservers_box.pack_end(&combo_servers, true, true, 2);
    button_box.pack_start(&reset_btn, true, true, 2);
    button_box.pack_end(&apply_btn, true, true, 2);
    connection_box.set_halign(gtk::Align::Fill);
    dnsservers_box.set_halign(gtk::Align::Fill);
    button_box.set_halign(gtk::Align::Fill);
    topbox.pack_start(&connection_box, true, true, 5);
    topbox.pack_start(&dnsservers_box, true, true, 5);
    topbox.pack_start(&button_box, true, true, 5);

    topbox.set_hexpand(true);
    topbox
}

pub fn create_connections_page(builder: &Builder) {
    let viewport = gtk::Viewport::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    let image = gtk::Image::from_icon_name(Some("go-previous"), gtk::IconSize::Button);
    let back_btn = gtk::Button::new();
    back_btn.set_image(Some(&image));
    back_btn.set_widget_name("tweaksBrowser");

    back_btn.connect_clicked(glib::clone!(@weak builder => move |button| {
        let name = button.widget_name();
        let stack: gtk::Stack = builder.object("stack").unwrap();
        stack.set_visible_child_name(&format!("{name}page"));
    }));

    let connections_section_box = create_connections_section();

    let child_name = "dnsConnectionsBrowserpage";
    connections_section_box.set_widget_name(&format!("{child_name}_connections"));

    let grid = gtk::Grid::new();
    grid.set_hexpand(true);
    grid.set_margin_start(10);
    grid.set_margin_end(10);
    grid.set_margin_top(5);
    grid.set_margin_bottom(5);
    grid.attach(&back_btn, 0, 1, 1, 1);
    let box_collection_s = gtk::Box::new(gtk::Orientation::Vertical, 5);
    let box_collection = gtk::Box::new(gtk::Orientation::Vertical, 5);
    box_collection.set_widget_name(child_name);

    box_collection.pack_start(&connections_section_box, false, false, 10);

    box_collection.set_valign(gtk::Align::Center);
    box_collection.set_halign(gtk::Align::Center);
    box_collection_s.pack_start(&grid, false, false, 0);
    box_collection_s.pack_start(&box_collection, false, false, 10);
    viewport.add(&box_collection_s);
    viewport.show_all();

    let stack: gtk::Stack = builder.object("stack").unwrap();
    stack.add_named(&viewport, child_name);
}
