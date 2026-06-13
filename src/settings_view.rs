// Module "settings_view" — vue Paramètres (= settings_view.py)
//
// Réglages globaux + édition de la liste d'hôtes, enregistrés dans config.json.
// Les changements d'hôtes et de langue prennent effet au prochain démarrage
// (on l'indique par une note) ; on garde l'architecture simple.
//
// Concepts Rust : on partage la config via Rc<RefCell<Config>> (mutation
// possible depuis les callbacks), et on lit les champs des SpinButton/Entry
// au moment de l'enregistrement.

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*; // ré-exporte aussi les traits de gtk
use gtk::glib;

use crate::config::{Config, HostCfg};
use crate::i18n::{SUPPORTED, native_name, tr};

// Codes du sélecteur : "auto" puis les langues prises en charge.
fn lang_codes() -> Vec<&'static str> {
    let mut v = vec!["auto"];
    v.extend_from_slice(SUPPORTED);
    v
}

pub struct SettingsView {
    // Fenêtre dédiée aux Paramètres (créée une fois, masquée à la fermeture).
    window: adw::Window,
}

impl SettingsView {
    pub fn new(config: Rc<RefCell<Config>>) -> Self {
        let outer = gtk::Box::new(gtk::Orientation::Vertical, 18);
        outer.set_margin_top(18);
        outer.set_margin_bottom(18);
        outer.set_margin_start(24);
        outer.set_margin_end(24);

        let cfg = config.borrow();

        // --- Langue ---
        outer.append(&section_label(&tr("Language")));
        let lang_combo = {
            let mut names = vec![tr("System default")];
            names.extend(SUPPORTED.iter().map(|c| native_name(c).to_string()));
            let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
            let combo = gtk::DropDown::from_strings(&refs);
            let codes = lang_codes();
            let sel = codes.iter().position(|c| *c == cfg.language).unwrap_or(0);
            combo.set_selected(sel as u32);
            outer.append(&row(&tr("Language"), &combo));
            combo
        };

        // --- Mesure ---
        outer.append(&section_label(&tr("Measurement")));
        let interval = spin(0.5, 60.0, 0.5, cfg.interval, 1);
        let window = spin(10.0, 600.0, 10.0, cfg.window as f64, 0);
        let deg_lat = spin(10.0, 2000.0, 5.0, cfg.degraded_latency, 0);
        let deg_loss = spin(0.0, 100.0, 1.0, cfg.degraded_loss, 0);
        outer.append(&row(&tr("Interval between pings (s)"), &interval));
        outer.append(&row(&tr("Samples kept (live)"), &window));
        outer.append(&row(&tr("Degraded threshold — latency (ms)"), &deg_lat));
        outer.append(&row(&tr("Degraded threshold — loss (%)"), &deg_loss));

        // --- Qualité ---
        outer.append(&section_label(&tr("Quality score (0..5)")));
        let q_lat = spin(5.0, 1000.0, 5.0, cfg.quality_good_latency, 0);
        let q_jit = spin(1.0, 500.0, 1.0, cfg.quality_good_jitter, 0);
        let q_pp = spin(0.5, 50.0, 0.5, cfg.quality_loss_per_point, 1);
        outer.append(&row(&tr("Ideal latency (ms)"), &q_lat));
        outer.append(&row(&tr("Ideal jitter (ms)"), &q_jit));
        outer.append(&row(&tr("Loss costing one point (%)"), &q_pp));

        // --- Hôtes ---
        outer.append(&section_label(&tr("Monitored hosts")));
        let hosts_box = gtk::Box::new(gtk::Orientation::Vertical, 8);
        for h in &cfg.hosts {
            hosts_box.append(&host_row(&h.name, &h.address));
        }
        outer.append(&hosts_box);

        let add_btn = gtk::Button::with_label(&tr("Add a host"));
        add_btn.add_css_class("flat");
        add_btn.set_halign(gtk::Align::Start);
        add_btn.connect_clicked(glib::clone!(
            #[weak]
            hosts_box,
            move |_| hosts_box.append(&host_row("", ""))
        ));
        outer.append(&add_btn);

        drop(cfg); // libère l'emprunt en lecture avant les callbacks (qui empruntent en écriture)

        // --- Note + Enregistrer ---
        let note = gtk::Label::new(Some(""));
        note.set_xalign(0.0);
        note.add_css_class("dim-label");
        note.set_wrap(true);
        outer.append(&note);

        let save = gtk::Button::with_label(&tr("Save"));
        save.add_css_class("suggested-action");
        save.set_halign(gtk::Align::End);
        save.connect_clicked(glib::clone!(
            #[strong]
            config,
            #[weak]
            hosts_box,
            #[weak]
            note,
            move |_| {
                let mut c = config.borrow_mut();
                c.interval = interval.value();
                c.window = window.value() as u32;
                c.degraded_latency = deg_lat.value();
                c.degraded_loss = deg_loss.value();
                c.quality_good_latency = q_lat.value();
                c.quality_good_jitter = q_jit.value();
                c.quality_loss_per_point = q_pp.value();
                let codes = lang_codes();
                let idx = lang_combo.selected() as usize;
                c.language = codes.get(idx).copied().unwrap_or("auto").to_string();

                // Relit les lignes d'hôtes (parcours des enfants de hosts_box).
                let mut hosts = collect_hosts(&hosts_box);
                if hosts.is_empty() {
                    hosts.push(HostCfg {
                        address: "8.8.8.8".to_string(),
                        name: "Google DNS".to_string(),
                    });
                }
                c.hosts = hosts;

                let _ = c.save();
                note.set_text(&tr("Restart required to apply hosts and language."));
            }
        ));
        outer.append(&save);

        let scroller = gtk::ScrolledWindow::builder()
            .child(&outer)
            .vexpand(true)
            .build();

        // La fenêtre = une barre de titre (adw::HeaderBar) + le contenu défilant,
        // assemblés par une adw::ToolbarView (gère l'empilement haut/contenu).
        let toolbar = adw::ToolbarView::new();
        toolbar.add_top_bar(&adw::HeaderBar::new());
        toolbar.set_content(Some(&scroller));

        let window = adw::Window::builder()
            .title(tr("Settings"))
            .modal(true) // bloque la fenêtre principale tant qu'elle est ouverte
            .hide_on_close(true) // fermer = masquer (on garde l'état, pas de re-création)
            .default_width(540)
            .default_height(640)
            .content(&toolbar)
            .build();

        Self { window }
    }

    /// Rattache la fenêtre Paramètres à la fenêtre principale (centrage + modalité).
    pub fn set_parent(&self, parent: &impl IsA<gtk::Window>) {
        self.window.set_transient_for(Some(parent));
    }

    /// Affiche (ou ramène au premier plan) la fenêtre Paramètres.
    pub fn present(&self) {
        self.window.present();
    }
}

// Parcourt les lignes d'hôtes et reconstruit la liste (adresse non vide requise).
fn collect_hosts(hosts_box: &gtk::Box) -> Vec<HostCfg> {
    let mut out = Vec::new();
    let mut child = hosts_box.first_child();
    while let Some(row) = child {
        // Une ligne = [name_entry, addr_entry, remove_btn].
        if let Some(name_e) = row.first_child().and_downcast::<gtk::Entry>() {
            if let Some(addr_e) = name_e.next_sibling().and_downcast::<gtk::Entry>() {
                let addr = addr_e.text().trim().to_string();
                if !addr.is_empty() {
                    let name = name_e.text().trim().to_string();
                    let name = if name.is_empty() { addr.clone() } else { name };
                    out.push(HostCfg { address: addr, name });
                }
            }
        }
        child = row.next_sibling();
    }
    out
}

// Une ligne d'édition d'hôte : nom + adresse + bouton supprimer.
fn host_row(name: &str, address: &str) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_e = gtk::Entry::new();
    name_e.set_placeholder_text(Some(&tr("Name")));
    name_e.set_text(name);
    name_e.set_hexpand(true);
    let addr_e = gtk::Entry::new();
    addr_e.set_placeholder_text(Some(&tr("Address / IP / domain")));
    addr_e.set_text(address);
    addr_e.set_hexpand(true);
    let remove = gtk::Button::from_icon_name("user-trash-symbolic");
    remove.add_css_class("flat");
    remove.connect_clicked(glib::clone!(
        #[weak]
        row,
        move |_| {
            if let Some(parent) = row.parent().and_downcast::<gtk::Box>() {
                parent.remove(&row);
            }
        }
    ));
    row.append(&name_e);
    row.append(&addr_e);
    row.append(&remove);
    row
}

fn section_label(text: &str) -> gtk::Label {
    let l = gtk::Label::new(Some(text));
    l.set_xalign(0.0);
    l.add_css_class("heading");
    l
}

// Une ligne "libellé … widget" alignée.
fn row(label: &str, widget: &impl IsA<gtk::Widget>) -> gtk::Box {
    let b = gtk::Box::new(gtk::Orientation::Horizontal, 14);
    let l = gtk::Label::new(Some(label));
    l.set_xalign(0.0);
    l.set_hexpand(true);
    b.append(&l);
    b.append(widget);
    b
}

fn spin(lo: f64, hi: f64, step: f64, value: f64, digits: u32) -> gtk::SpinButton {
    let sb = gtk::SpinButton::with_range(lo, hi, step);
    sb.set_digits(digits);
    sb.set_value(value);
    sb
}
