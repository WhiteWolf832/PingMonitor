// Module "tiles_view" — vue Tuiles : une carte par hôte (= tiles_view.py)
//
// Une gtk::FlowBox qui réagence les cartes selon la largeur. Chaque Tile
// réutilise les mêmes widgets Cairo que le tableau (StatusDot/Sparkline/QualityBars).

use std::collections::HashMap;

use gtk::prelude::*;

use crate::model::Host;
use crate::widgets::{QualityBars, Sparkline, StatusDot};

/// Une carte d'hôte.
struct Tile {
    root: gtk::Box,
    dot: StatusDot,
    name: gtk::Label,
    address: gtk::Label,
    latency: gtk::Label,
    spark: Sparkline,
    loss: gtk::Label,
    jitter: gtk::Label,
    quality: QualityBars,
}

impl Tile {
    fn new() -> Self {
        // `root` porte le fond "carte" ; ses marges sont EXTERNES (espace entre
        // cartes voisines, en plus de l'espacement de la FlowBox).
        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.add_css_class("card"); // style "carte" de libadwaita
        root.set_size_request(220, -1);

        // `content` est le conteneur interne : ses marges créent le padding
        // visuel entre la bordure de la carte et les éléments.
        let content = gtk::Box::new(gtk::Orientation::Vertical, 6);
        content.set_margin_top(14);
        content.set_margin_bottom(14);
        content.set_margin_start(16);
        content.set_margin_end(16);
        root.append(&content);

        // Ligne du haut : pastille + (nom / adresse).
        let top = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let dot = StatusDot::new();
        top.append(&dot.area);
        let name_box = gtk::Box::new(gtk::Orientation::Vertical, 0);
        name_box.set_hexpand(true);
        let name = gtk::Label::new(Some(""));
        name.set_xalign(0.0);
        name.add_css_class("heading");
        let address = gtk::Label::new(Some(""));
        address.set_xalign(0.0);
        address.add_css_class("dim-label");
        name_box.append(&name);
        name_box.append(&address);
        top.append(&name_box);
        content.append(&top);

        let latency = gtk::Label::new(Some("—"));
        latency.set_xalign(0.0);
        latency.add_css_class("title-2");
        content.append(&latency);

        let spark = Sparkline::new();
        spark.area.set_content_width(200);
        spark.area.set_content_height(34);
        content.append(&spark.area);

        let metrics = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        let loss = gtk::Label::new(Some("Perte 0%"));
        loss.add_css_class("dim-label");
        let jitter = gtk::Label::new(Some("Gigue —"));
        jitter.add_css_class("dim-label");
        metrics.append(&loss);
        metrics.append(&jitter);
        let quality = QualityBars::new();
        quality.area.set_halign(gtk::Align::End);
        quality.area.set_hexpand(true);
        metrics.append(&quality.area);
        content.append(&metrics);

        Self {
            root,
            dot,
            name,
            address,
            latency,
            spark,
            loss,
            jitter,
            quality,
        }
    }

    fn update(&self, host: &Host) {
        self.dot.set_status(host.status);
        self.name.set_text(&host.name);
        self.address.set_text(&host.address);
        self.latency.set_text(&match host.last_latency() {
            Some(ms) => format!("{ms:.1} ms"),
            None => "—".to_string(),
        });
        self.spark.set_values(host.sparkline());
        self.loss.set_text(&format!("Perte {:.0}%", host.loss_pct()));
        self.jitter.set_text(&match host.jitter() {
            Some(j) => format!("Gigue {j:.0} ms"),
            None => "Gigue —".to_string(),
        });
        self.quality.set_score(host.quality());
        self.quality
            .area
            .set_tooltip_text(Some(&host.quality_tooltip()));
    }
}

pub struct TilesView {
    pub root: gtk::ScrolledWindow,
    flow: gtk::FlowBox,
    tiles: HashMap<String, Tile>,
}

impl TilesView {
    pub fn new() -> Self {
        let flow = gtk::FlowBox::new();
        flow.set_selection_mode(gtk::SelectionMode::None);
        flow.set_max_children_per_line(6);
        flow.set_min_children_per_line(1);
        flow.set_homogeneous(false);
        flow.set_row_spacing(12);
        flow.set_column_spacing(12);
        flow.set_margin_top(12);
        flow.set_margin_bottom(12);
        flow.set_margin_start(12);
        flow.set_margin_end(12);

        let root = gtk::ScrolledWindow::builder()
            .child(&flow)
            .vexpand(true)
            .hexpand(true)
            .build();

        Self {
            root,
            flow,
            tiles: HashMap::new(),
        }
    }

    pub fn add_host(&mut self, host: &Host) {
        let tile = Tile::new();
        tile.update(host);
        self.flow.append(&tile.root);
        self.tiles.insert(host.address.clone(), tile);
    }

    pub fn update_host(&self, host: &Host) {
        if let Some(tile) = self.tiles.get(&host.address) {
            tile.update(host);
        }
    }
}
