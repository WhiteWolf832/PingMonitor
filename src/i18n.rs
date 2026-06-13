// Module "i18n" — internationalisation légère (= i18n.py)
//
// L'anglais est la langue de base : les chaînes sources dans le code sont en
// anglais et `tr()` renvoie leur traduction selon la langue active.
//
// Concepts Rust introduits :
//   - état global mutable et sûr : RwLock<&'static str> (lecture concurrente)
//   - initialisation paresseuse d'une table : OnceLock<HashMap<...>>
//   - durées de vie : on ne manipule que des &'static str (littéraux du binaire)

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

pub const SUPPORTED: &[&str] = &["en", "fr", "de", "it", "es", "pt"];

/// Nom natif d'une langue (affiché tel quel, non traduit).
pub fn native_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "fr" => "Français",
        "de" => "Deutsch",
        "it" => "Italiano",
        "es" => "Español",
        "pt" => "Português",
        _ => "English",
    }
}

// Langue active. RwLock = plusieurs lecteurs simultanés, un seul écrivain.
// `&'static str` : on ne stocke qu'un des codes de SUPPORTED (vit tout le programme).
static EFFECTIVE: RwLock<&'static str> = RwLock::new("en");

/// Définit la langue active. `code` peut être "auto" ou un code ISO.
pub fn set_language(code: &str) {
    *EFFECTIVE.write().unwrap() = resolve(code);
}

/// Code réellement appliqué (jamais "auto").
pub fn effective_language() -> &'static str {
    *EFFECTIVE.read().unwrap()
}

// Déduit la langue : code explicite, sinon locale système (LANG/LC_*).
fn resolve(code: &str) -> &'static str {
    if !code.is_empty() && code != "auto" {
        return match_supported(code);
    }
    let loc = std::env::var("LC_ALL")
        .or_else(|_| std::env::var("LC_MESSAGES"))
        .or_else(|_| std::env::var("LANG"))
        .unwrap_or_default();
    match_supported(loc.get(..2).unwrap_or(""))
}

// Renvoie le &'static str de SUPPORTED correspondant, ou "en".
fn match_supported(two: &str) -> &'static str {
    let two = two.to_lowercase();
    SUPPORTED
        .iter()
        .copied()
        .find(|c| *c == two)
        .unwrap_or("en")
}

/// Traduit `msg` (chaîne source anglaise) vers la langue active.
pub fn tr(msg: &str) -> String {
    let lang = effective_language();
    if lang == "en" {
        return msg.to_string();
    }
    catalog()
        .get(lang)
        .and_then(|m| m.get(msg))
        .copied()
        .unwrap_or(msg)
        .to_string()
}

// Table des traductions, construite une seule fois (OnceLock = paresseux + sûr).
fn catalog() -> &'static HashMap<&'static str, HashMap<&'static str, &'static str>> {
    static CATALOG: OnceLock<HashMap<&str, HashMap<&str, &str>>> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut all = HashMap::new();
        all.insert("fr", to_map(FR));
        all.insert("de", to_map(DE));
        all.insert("it", to_map(IT));
        all.insert("es", to_map(ES));
        all.insert("pt", to_map(PT));
        all
    })
}

fn to_map(pairs: &[(&'static str, &'static str)]) -> HashMap<&'static str, &'static str> {
    pairs.iter().copied().collect()
}

// --- catalogues (clé anglaise, traduction) ---------------------------------

const FR: &[(&str, &str)] = &[
    ("File", "Fichier"),
    ("View", "Affichage"),
    ("Help", "Aide"),
    ("Add Host", "Ajouter un hôte"),
    ("Settings", "Paramètres"),
    ("Quit", "Quitter"),
    ("Table", "Tableau"),
    ("Tiles", "Tuiles"),
    ("Route", "Route"),
    ("Refresh", "Rafraîchir"),
    ("About", "À propos"),
    ("Add", "Ajouter"),
    ("Graphs", "Graphiques"),
    ("%d online", "%d en ligne"),
    ("%d degraded", "%d dégradé(s)"),
    ("%d offline", "%d hors ligne"),
    ("Name", "Nom"),
    ("Address", "Adresse"),
    ("Cancel", "Annuler"),
    ("Measurement", "Mesure"),
    ("Interval between pings (s)", "Intervalle entre pings (s)"),
    ("Samples kept (live)", "Échantillons gardés (live)"),
    ("Degraded threshold — latency (ms)", "Seuil dégradé — latence (ms)"),
    ("Degraded threshold — loss (%)", "Seuil dégradé — perte (%)"),
    ("Quality score (0..5)", "Score de qualité (0..5)"),
    ("Ideal latency (ms)", "Latence idéale (ms)"),
    ("Ideal jitter (ms)", "Gigue idéale (ms)"),
    ("Loss costing one point (%)", "Perte coûtant 1 point (%)"),
    ("Monitored hosts", "Hôtes surveillés"),
    ("Add a host", "Ajouter un hôte"),
    ("Close", "Fermer"),
    ("Save", "Enregistrer"),
    ("Language", "Langue"),
    ("Address / IP / domain", "Adresse / IP / domaine"),
    ("Latency", "Latence"),
    ("History", "Historique"),
    ("Loss", "Perte"),
    ("Jitter", "Gigue"),
    ("Quality", "Qualité"),
    ("Live", "Direct"),
    ("No host selected", "Aucun hôte sélectionné"),
    ("No data for this period", "Pas de données sur cette période"),
    ("Destination:", "Destination :"),
    ("Run traceroute", "Lancer le traceroute"),
    ("Host", "Hôte"),
    ("Sent", "Envoyés"),
    ("Last", "Dernier"),
    ("Avg", "Moy."),
    ("Min", "Min"),
    ("Max", "Max"),
    ("* (no response)", "* (sans réponse)"),
    ("Analyzing path to {addr}…", "Analyse du chemin vers {addr}…"),
    ("Error: {err}", "Erreur : {err}"),
    ("%d hop(s)", "%d saut(s)"),
    ("System default", "Langue du système"),
    ("Restart required to apply hosts and language.",
        "Redémarrage nécessaire pour appliquer les hôtes et la langue."),
];

const DE: &[(&str, &str)] = &[
    ("File", "Datei"),
    ("View", "Ansicht"),
    ("Help", "Hilfe"),
    ("Add Host", "Host hinzufügen"),
    ("Settings", "Einstellungen"),
    ("Quit", "Beenden"),
    ("Table", "Tabelle"),
    ("Tiles", "Kacheln"),
    ("Route", "Route"),
    ("Refresh", "Aktualisieren"),
    ("About", "Über"),
    ("Add", "Hinzufügen"),
    ("Graphs", "Diagramme"),
    ("%d online", "%d online"),
    ("%d degraded", "%d beeinträchtigt"),
    ("%d offline", "%d offline"),
    ("Name", "Name"),
    ("Address", "Adresse"),
    ("Cancel", "Abbrechen"),
    ("Measurement", "Messung"),
    ("Interval between pings (s)", "Intervall zwischen Pings (s)"),
    ("Samples kept (live)", "Gespeicherte Messwerte (live)"),
    ("Degraded threshold — latency (ms)", "Schwelle beeinträchtigt — Latenz (ms)"),
    ("Degraded threshold — loss (%)", "Schwelle beeinträchtigt — Verlust (%)"),
    ("Quality score (0..5)", "Qualitätsbewertung (0..5)"),
    ("Ideal latency (ms)", "Ideale Latenz (ms)"),
    ("Ideal jitter (ms)", "Idealer Jitter (ms)"),
    ("Loss costing one point (%)", "Verlust pro Punkt (%)"),
    ("Monitored hosts", "Überwachte Hosts"),
    ("Add a host", "Host hinzufügen"),
    ("Close", "Schließen"),
    ("Save", "Speichern"),
    ("Language", "Sprache"),
    ("Address / IP / domain", "Adresse / IP / Domain"),
    ("Latency", "Latenz"),
    ("History", "Verlauf"),
    ("Loss", "Verlust"),
    ("Jitter", "Jitter"),
    ("Quality", "Qualität"),
    ("Live", "Live"),
    ("No host selected", "Kein Host ausgewählt"),
    ("No data for this period", "Keine Daten für diesen Zeitraum"),
    ("Destination:", "Ziel:"),
    ("Run traceroute", "Traceroute starten"),
    ("Host", "Host"),
    ("Sent", "Gesendet"),
    ("Last", "Letzte"),
    ("Avg", "Ø"),
    ("Min", "Min"),
    ("Max", "Max"),
    ("* (no response)", "* (keine Antwort)"),
    ("Analyzing path to {addr}…", "Analysiere Pfad zu {addr}…"),
    ("Error: {err}", "Fehler: {err}"),
    ("%d hop(s)", "%d Hop(s)"),
    ("System default", "Systemsprache"),
    ("Restart required to apply hosts and language.",
        "Neustart erforderlich, um Hosts und Sprache anzuwenden."),
];

const IT: &[(&str, &str)] = &[
    ("File", "File"),
    ("View", "Visualizza"),
    ("Help", "Aiuto"),
    ("Add Host", "Aggiungi host"),
    ("Settings", "Impostazioni"),
    ("Quit", "Esci"),
    ("Table", "Tabella"),
    ("Tiles", "Riquadri"),
    ("Route", "Percorso"),
    ("Refresh", "Aggiorna"),
    ("About", "Informazioni"),
    ("Add", "Aggiungi"),
    ("Graphs", "Grafici"),
    ("%d online", "%d online"),
    ("%d degraded", "%d degradato/i"),
    ("%d offline", "%d offline"),
    ("Name", "Nome"),
    ("Address", "Indirizzo"),
    ("Cancel", "Annulla"),
    ("Measurement", "Misurazione"),
    ("Interval between pings (s)", "Intervallo tra i ping (s)"),
    ("Samples kept (live)", "Campioni conservati (live)"),
    ("Degraded threshold — latency (ms)", "Soglia degradato — latenza (ms)"),
    ("Degraded threshold — loss (%)", "Soglia degradato — perdita (%)"),
    ("Quality score (0..5)", "Punteggio qualità (0..5)"),
    ("Ideal latency (ms)", "Latenza ideale (ms)"),
    ("Ideal jitter (ms)", "Jitter ideale (ms)"),
    ("Loss costing one point (%)", "Perdita per punto (%)"),
    ("Monitored hosts", "Host monitorati"),
    ("Add a host", "Aggiungi un host"),
    ("Close", "Chiudi"),
    ("Save", "Salva"),
    ("Language", "Lingua"),
    ("Address / IP / domain", "Indirizzo / IP / dominio"),
    ("Latency", "Latenza"),
    ("History", "Cronologia"),
    ("Loss", "Perdita"),
    ("Jitter", "Jitter"),
    ("Quality", "Qualità"),
    ("Live", "Live"),
    ("No host selected", "Nessun host selezionato"),
    ("No data for this period", "Nessun dato per questo periodo"),
    ("Destination:", "Destinazione:"),
    ("Run traceroute", "Avvia traceroute"),
    ("Host", "Host"),
    ("Sent", "Inviati"),
    ("Last", "Ultimo"),
    ("Avg", "Media"),
    ("Min", "Min"),
    ("Max", "Max"),
    ("* (no response)", "* (nessuna risposta)"),
    ("Analyzing path to {addr}…", "Analisi del percorso verso {addr}…"),
    ("Error: {err}", "Errore: {err}"),
    ("%d hop(s)", "%d hop"),
    ("System default", "Lingua di sistema"),
    ("Restart required to apply hosts and language.",
        "Riavvio necessario per applicare host e lingua."),
];

const ES: &[(&str, &str)] = &[
    ("File", "Archivo"),
    ("View", "Ver"),
    ("Help", "Ayuda"),
    ("Add Host", "Añadir host"),
    ("Settings", "Ajustes"),
    ("Quit", "Salir"),
    ("Table", "Tabla"),
    ("Tiles", "Mosaicos"),
    ("Route", "Ruta"),
    ("Refresh", "Actualizar"),
    ("About", "Acerca de"),
    ("Add", "Añadir"),
    ("Graphs", "Gráficos"),
    ("%d online", "%d en línea"),
    ("%d degraded", "%d degradado(s)"),
    ("%d offline", "%d sin conexión"),
    ("Name", "Nombre"),
    ("Address", "Dirección"),
    ("Cancel", "Cancelar"),
    ("Measurement", "Medición"),
    ("Interval between pings (s)", "Intervalo entre pings (s)"),
    ("Samples kept (live)", "Muestras guardadas (live)"),
    ("Degraded threshold — latency (ms)", "Umbral degradado — latencia (ms)"),
    ("Degraded threshold — loss (%)", "Umbral degradado — pérdida (%)"),
    ("Quality score (0..5)", "Puntuación de calidad (0..5)"),
    ("Ideal latency (ms)", "Latencia ideal (ms)"),
    ("Ideal jitter (ms)", "Jitter ideal (ms)"),
    ("Loss costing one point (%)", "Pérdida por punto (%)"),
    ("Monitored hosts", "Hosts supervisados"),
    ("Add a host", "Añadir un host"),
    ("Close", "Cerrar"),
    ("Save", "Guardar"),
    ("Language", "Idioma"),
    ("Address / IP / domain", "Dirección / IP / dominio"),
    ("Latency", "Latencia"),
    ("History", "Historial"),
    ("Loss", "Pérdida"),
    ("Jitter", "Jitter"),
    ("Quality", "Calidad"),
    ("Live", "En vivo"),
    ("No host selected", "Ningún host seleccionado"),
    ("No data for this period", "Sin datos para este periodo"),
    ("Destination:", "Destino:"),
    ("Run traceroute", "Iniciar traceroute"),
    ("Host", "Host"),
    ("Sent", "Enviados"),
    ("Last", "Último"),
    ("Avg", "Media"),
    ("Min", "Mín"),
    ("Max", "Máx"),
    ("* (no response)", "* (sin respuesta)"),
    ("Analyzing path to {addr}…", "Analizando ruta hacia {addr}…"),
    ("Error: {err}", "Error: {err}"),
    ("%d hop(s)", "%d salto(s)"),
    ("System default", "Idioma del sistema"),
    ("Restart required to apply hosts and language.",
        "Se requiere reiniciar para aplicar hosts e idioma."),
];

const PT: &[(&str, &str)] = &[
    ("File", "Arquivo"),
    ("View", "Exibir"),
    ("Help", "Ajuda"),
    ("Add Host", "Adicionar host"),
    ("Settings", "Configurações"),
    ("Quit", "Sair"),
    ("Table", "Tabela"),
    ("Tiles", "Blocos"),
    ("Route", "Rota"),
    ("Refresh", "Atualizar"),
    ("About", "Sobre"),
    ("Add", "Adicionar"),
    ("Graphs", "Gráficos"),
    ("%d online", "%d online"),
    ("%d degraded", "%d degradado(s)"),
    ("%d offline", "%d offline"),
    ("Name", "Nome"),
    ("Address", "Endereço"),
    ("Cancel", "Cancelar"),
    ("Measurement", "Medição"),
    ("Interval between pings (s)", "Intervalo entre pings (s)"),
    ("Samples kept (live)", "Amostras mantidas (live)"),
    ("Degraded threshold — latency (ms)", "Limite degradado — latência (ms)"),
    ("Degraded threshold — loss (%)", "Limite degradado — perda (%)"),
    ("Quality score (0..5)", "Pontuação de qualidade (0..5)"),
    ("Ideal latency (ms)", "Latência ideal (ms)"),
    ("Ideal jitter (ms)", "Jitter ideal (ms)"),
    ("Loss costing one point (%)", "Perda por ponto (%)"),
    ("Monitored hosts", "Hosts monitorados"),
    ("Add a host", "Adicionar um host"),
    ("Close", "Fechar"),
    ("Save", "Salvar"),
    ("Language", "Idioma"),
    ("Address / IP / domain", "Endereço / IP / domínio"),
    ("Latency", "Latência"),
    ("History", "Histórico"),
    ("Loss", "Perda"),
    ("Jitter", "Jitter"),
    ("Quality", "Qualidade"),
    ("Live", "Ao vivo"),
    ("No host selected", "Nenhum host selecionado"),
    ("No data for this period", "Sem dados para este período"),
    ("Destination:", "Destino:"),
    ("Run traceroute", "Iniciar traceroute"),
    ("Host", "Host"),
    ("Sent", "Enviados"),
    ("Last", "Último"),
    ("Avg", "Média"),
    ("Min", "Mín"),
    ("Max", "Máx"),
    ("* (no response)", "* (sem resposta)"),
    ("Analyzing path to {addr}…", "Analisando caminho até {addr}…"),
    ("Error: {err}", "Erro: {err}"),
    ("%d hop(s)", "%d salto(s)"),
    ("System default", "Idioma do sistema"),
    ("Restart required to apply hosts and language.",
        "Reinício necessário para aplicar hosts e idioma."),
];
