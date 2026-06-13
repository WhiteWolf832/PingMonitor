// Ping Monitor — network latency monitor (Rust).
// Copyright (C) 2026  WhiteWolf832
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.
//
// Ping Monitor — version Rust (étape 5d : config + i18n + paramètres)
//
// Les threads de ping envoient des résultats bruts ; le thread UI tient à jour
// le modèle (les Host) et rafraîchit les vues. Les hôtes surveillés et les
// seuils proviennent désormais de la configuration (config.json).

mod config;
mod graph_view;
mod i18n;
mod model;
mod ping;
mod route;
mod route_view;
mod settings_view;
mod storage;
mod table_view;
mod tiles_view;
mod widgets;

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

use adw::prelude::*;
use gtk::glib;

use config::Config;
use graph_view::GraphView;
use i18n::tr;
use model::{Host, QualityCfg, Sample, now_ts};
use ping::{Update, start_monitor};
use route_view::RouteView;
use settings_view::SettingsView;
use storage::Storage;
use table_view::TableView;
use tiles_view::TilesView;

const APP_ID: &str = "fr.wolf.PingMonitorRs";

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id(APP_ID)
        .build();
    app.connect_startup(|_| {
        adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
        // Icône par défaut de toutes les fenêtres : GTK la cherche dans le thème
        // d'icônes par ce nom (= l'icône installée fr.wolf.PingMonitorRs.svg).
        // Sans ça, lancée depuis un terminal, l'appli n'a pas d'icône sous X11.
        gtk::Window::set_default_icon_name(APP_ID);
        // Langue active déduite de la config dès le démarrage (avant les vues).
        i18n::set_language(&Config::load().language);
    });
    app.connect_activate(build_ui);
    app.run()
}

fn build_ui(app: &adw::Application) {
    let header = adw::HeaderBar::new();

    // Configuration partagée (Rc<RefCell> = lisible partout, modifiable par les
    // Paramètres). On la charge une fois ; la vue Paramètres l'écrira sur disque.
    let config = Rc::new(RefCell::new(Config::load()));
    let cfg = config.borrow();

    // Seuils de qualité issus de la config (appliqués à chaque Host).
    let quality_cfg = QualityCfg {
        good_latency: cfg.quality_good_latency,
        good_jitter: cfg.quality_good_jitter,
        loss_per_point: cfg.quality_loss_per_point,
    };

    // On construit la vue Tableau ET la vue Tuiles ; pour chaque hôte on crée un
    // modèle Host, une ligne de tableau, et une carte de tuile.
    let mut table = TableView::new();
    let mut tiles = TilesView::new();

    // Deux dictionnaires indexés par adresse :
    //   - models : l'état (mutable) de chaque hôte
    //   - rows   : la ligne d'UI correspondante
    let mut models: HashMap<String, Host> = HashMap::new();
    let mut rows = HashMap::new();

    for h in &cfg.hosts {
        let mut host = Host::new(&h.address, &h.name);
        host.quality_cfg = quality_cfg;
        host.degraded_loss = cfg.degraded_loss;
        host.degraded_latency = cfg.degraded_latency;
        let row = table.add_host(&h.address);
        row.update(&host); // affichage initial (état "inconnu")
        tiles.add_host(&host); // carte correspondante dans la vue Tuiles
        models.insert(h.address.clone(), host);
        rows.insert(h.address.clone(), row);
    }

    // Liste (nom, adresse) réutilisée par la vue Route et le graphique.
    let host_list: Vec<(String, String)> = cfg
        .hosts
        .iter()
        .map(|h| (h.name.clone(), h.address.clone()))
        .collect();
    let addresses: Vec<String> = cfg.hosts.iter().map(|h| h.address.clone()).collect();
    let interval = Duration::from_secs_f64(cfg.interval.max(0.5));
    drop(cfg); // libère l'emprunt en lecture avant de partager `config` ailleurs

    let route = RouteView::new(host_list.clone());
    let settings = SettingsView::new(Rc::clone(&config));

    // --- empilement des vues + sélecteur dans l'en-tête ---
    // adw::ViewStack ne montre qu'une page à la fois ; adw::ViewSwitcher fournit
    // les boutons (avec icônes) reliés automatiquement à ce stack.
    let stack = adw::ViewStack::new();
    stack.add_titled(&table.root, Some("table"), &tr("Table"));
    stack.add_titled(&tiles.root, Some("tiles"), &tr("Tiles"));
    stack.add_titled(&route.root, Some("route"), &tr("Route"));

    let switcher = adw::ViewSwitcher::new();
    switcher.set_stack(Some(&stack));
    switcher.set_policy(adw::ViewSwitcherPolicy::Wide);

    // Bouton "engrenage" dans l'en-tête : ouvre les Paramètres dans leur fenêtre.
    let settings = Rc::new(settings);
    let settings_btn = gtk::Button::from_icon_name("emblem-system-symbolic");
    settings_btn.set_tooltip_text(Some(&tr("Settings")));
    settings_btn.connect_clicked(glib::clone!(
        #[strong]
        settings,
        move |_| settings.present()
    ));
    header.pack_end(&settings_btn);

    // --- persistance SQLite ---
    // Storage partagé via Rc : la boucle de réception l'utilise pour écrire,
    // et le graphique pour lire. Les méthodes prennent &self, donc Rc suffit
    // (pas besoin de RefCell ni de Mutex). En cas d'échec disque, on se rabat
    // sur une base en mémoire pour que l'appli reste fonctionnelle.
    let storage = Rc::new(
        Storage::open()
            .inspect(|s| {
                let cutoff = now_ts() - 30.0 * 24.0 * 3600.0;
                let _ = s.prune(cutoff); // purge > 30 jours au démarrage
            })
            .or_else(|e| {
                eprintln!("Stockage disque indisponible ({e}) — base en mémoire");
                Storage::open_memory()
            })
            .expect("impossible d'initialiser le stockage"),
    );

    // --- graphique (sous les vues, dans un panneau redimensionnable) ---
    // Rc<GraphView> : partagé entre le callback de sélection du tableau ET la
    // boucle de réception (les deux appellent des méthodes &self).
    let graph = Rc::new(GraphView::new(Rc::clone(&storage), host_list));

    // Cliquer une ligne du tableau sélectionne cet hôte dans le graphique.
    let graph_sel = Rc::clone(&graph);
    table.connect_selected(move |addr| graph_sel.select_host(addr));
    table.select_first(); // surbrillance initiale + graphique synchronisé

    // Hauteurs minimales : la liste et le graphique ne peuvent pas être réduits
    // à rien (le séparateur du Paned se bloque sur ces seuils). -1 = pas de
    // contrainte de largeur.
    table.root.set_size_request(-1, 140);
    graph.root.set_size_request(-1, 150);

    // Les trois vues (le Stack) en haut, le graphique en bas.
    let paned = gtk::Paned::new(gtk::Orientation::Vertical);
    paned.set_start_child(Some(&stack));
    paned.set_end_child(Some(&graph.root));
    paned.set_resize_start_child(true);
    paned.set_resize_end_child(true);
    // Par défaut un Paned PEUT réduire ses enfants sous leur taille minimale.
    // On l'interdit : la poignée se bloque sur les size_request des enfants.
    paned.set_shrink_start_child(false);
    paned.set_shrink_end_child(false);
    paned.set_position(190);

    // Le sélecteur de vue prend la place du titre, au centre de l'en-tête.
    header.set_title_widget(Some(&switcher));

    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&paned));

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Ping Monitor (Rust)")
        .default_width(760)
        .default_height(560)
        .content(&toolbar)
        .build();
    // Taille minimale de la fenêtre (largeur + hauteur) : empêche de la réduire
    // au point de tout écraser. Reste bien en dessous de la taille par défaut.
    window.set_size_request(380, 460);
    window.present();

    // La fenêtre Paramètres est modale par rapport à la fenêtre principale.
    settings.set_parent(&window);

    // --- moteur de ping ---
    let (tx, rx) = async_channel::unbounded::<Update>();
    start_monitor(addresses, interval, tx);

    // --- réception sur le thread UI ---
    // La tâche async POSSÈDE models + rows + graph, et un clone Rc de storage.
    // À chaque message : on persiste, on met à jour le modèle + la ligne, et on
    // rafraîchit le graphique (s'il est en mode direct).
    let store_loop = Rc::clone(&storage);
    glib::spawn_future_local(async move {
        while let Ok(update) = rx.recv().await {
            let ts = now_ts();
            let _ = store_loop.add(&update.host, ts, update.latency_ms);
            if let Some(host) = models.get_mut(&update.host) {
                host.add(Sample {
                    latency: update.latency_ms,
                });
                if let Some(row) = rows.get(&update.host) {
                    row.update(host);
                }
                tiles.update_host(host); // met à jour la carte correspondante
            }
            graph.refresh();
        }
    });
}
