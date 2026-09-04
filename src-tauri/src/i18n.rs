//! Textos de la app en varios idiomas (barra de menú, estados y errores).
//!
//! La interfaz web tiene su propia tabla en `ui/i18n.js`; aquí solo viven los textos que
//! genera Rust: el menú de la barra, los estados y los mensajes de error.
//!
//! Para añadir un idioma: agrégalo a `LANGS`, amplía el array de cada clave y ajusta `pick`.

/// Idiomas soportados por la interfaz, en el mismo orden que los arrays de `t`.
pub const LANGS: [&str; 6] = ["es", "en", "pt", "fr", "de", "it"];

fn pick(lang: &str, v: [&'static str; 6]) -> &'static str {
    match lang {
        "en" => v[1],
        "pt" => v[2],
        "fr" => v[3],
        "de" => v[4],
        "it" => v[5],
        _ => v[0],
    }
}

/// Resuelve "auto" al idioma del sistema, y cualquier idioma no soportado a inglés.
pub fn resolve(lang: &str) -> String {
    let code = if lang.is_empty() || lang.eq_ignore_ascii_case("auto") {
        crate::platform::system_language()
    } else {
        lang.to_string()
    };
    let short = code.split(['-', '_']).next().unwrap_or("en").to_lowercase();
    if LANGS.contains(&short.as_str()) {
        short
    } else {
        "en".to_string()
    }
}

/// Devuelve el texto de `key` en `lang`. Las claves desconocidas se devuelven tal cual.
pub fn t<'a>(lang: &str, key: &'a str) -> &'a str {
    match key {
        // --- Estados ---
        "status.idle" => pick(lang, ["Listo", "Ready", "Pronto", "Prêt", "Bereit", "Pronto"]),
        "status.recording" => pick(lang, ["Grabando…", "Recording…", "Gravando…", "Enregistrement…", "Aufnahme…", "Registrazione…"]),
        "status.transcribing" => pick(lang, ["Transcribiendo…", "Transcribing…", "Transcrevendo…", "Transcription…", "Transkription…", "Trascrizione…"]),
        "status.pasting" => pick(lang, ["Pegando…", "Pasting…", "Colando…", "Collage…", "Einfügen…", "Incollaggio…"]),
        "status.error" => pick(lang, ["Error", "Error", "Erro", "Erreur", "Fehler", "Errore"]),
        "status.done" => pick(lang, ["Hecho", "Done", "Pronto", "Terminé", "Fertig", "Fatto"]),

        // --- Menú de la barra ---
        "tray.hint" => pick(lang, [
            "Mantén {k} y habla",
            "Hold {k} and speak",
            "Segure {k} e fale",
            "Maintenez {k} et parlez",
            "{k} halten und sprechen",
            "Tieni premuto {k} e parla",
        ]),
        "tray.settings" => pick(lang, ["Configuración…", "Settings…", "Configurações…", "Réglages…", "Einstellungen…", "Impostazioni…"]),
        "tray.retry" => pick(lang, [
            "Reintentar última transcripción",
            "Retry last transcription",
            "Tentar novamente a última transcrição",
            "Réessayer la dernière transcription",
            "Letzte Transkription wiederholen",
            "Riprova l'ultima trascrizione",
        ]),
        "tray.autopaste" => pick(lang, ["Pegado automático", "Auto-paste", "Colagem automática", "Collage automatique", "Automatisch einfügen", "Incolla automatico"]),
        "tray.quit" => pick(lang, ["Salir de Dictado", "Quit Dictado", "Sair do Dictado", "Quitter Dictado", "Dictado beenden", "Esci da Dictado"]),

        // --- Mensajes de resultado ---
        "msg.pasted" => pick(lang, ["Texto pegado", "Text pasted", "Texto colado", "Texte collé", "Text eingefügt", "Testo incollato"]),
        "msg.pasted_kept" => pick(lang, [
            "Texto pegado (el portapapeles cambió y no se restauró)",
            "Text pasted (clipboard changed, not restored)",
            "Texto colado (a área de transferência mudou e não foi restaurada)",
            "Texte collé (le presse-papiers a changé, non restauré)",
            "Text eingefügt (Zwischenablage geändert, nicht wiederhergestellt)",
            "Testo incollato (gli appunti sono cambiati e non sono stati ripristinati)",
        ]),
        "msg.copied" => pick(lang, [
            "Texto copiado al portapapeles",
            "Text copied to clipboard",
            "Texto copiado para a área de transferência",
            "Texte copié dans le presse-papiers",
            "Text in die Zwischenablage kopiert",
            "Testo copiato negli appunti",
        ]),
        "msg.too_short" => pick(lang, ["Grabación demasiado corta", "Recording too short", "Gravação muito curta", "Enregistrement trop court", "Aufnahme zu kurz", "Registrazione troppo breve"]),
        "msg.no_speech" => pick(lang, ["No se detectó voz", "No speech detected", "Nenhuma fala detectada", "Aucune voix détectée", "Keine Sprache erkannt", "Nessuna voce rilevata"]),
        "msg.nothing_retry" => pick(lang, ["No hay nada que reintentar", "Nothing to retry", "Nada para tentar novamente", "Rien à réessayer", "Nichts zu wiederholen", "Niente da riprovare"]),

        // --- Errores del flujo ---
        "err.api_key_missing" => pick(lang, [
            "Configura tu API key de {p} en Configuración",
            "Set up your {p} API key in Settings",
            "Configure sua chave de API de {p} nas Configurações",
            "Configurez votre clé API {p} dans les Réglages",
            "Richte deinen {p}-API-Schlüssel in den Einstellungen ein",
            "Configura la tua API key di {p} nelle Impostazioni",
        ]),
        "err.keychain" => pick(lang, [
            "No se pudo leer la API key del llavero: {e}",
            "Could not read the API key from the keychain: {e}",
            "Não foi possível ler a chave de API do chaveiro: {e}",
            "Impossible de lire la clé API du trousseau : {e}",
            "API-Schlüssel konnte nicht aus dem Schlüsselbund gelesen werden: {e}",
            "Impossibile leggere l'API key dal portachiavi: {e}",
        ]),
        "err.mic_denied" => pick(lang, [
            "Sin acceso al micrófono. Actívalo en Ajustes del Sistema → Privacidad → Micrófono",
            "No microphone access. Enable it in System Settings → Privacy → Microphone",
            "Sem acesso ao microfone. Ative em Ajustes do Sistema → Privacidade → Microfone",
            "Pas d'accès au micro. Activez-le dans Réglages Système → Confidentialité → Microphone",
            "Kein Mikrofonzugriff. Aktiviere ihn in Systemeinstellungen → Datenschutz → Mikrofon",
            "Nessun accesso al microfono. Attivalo in Impostazioni di Sistema → Privacy → Microfono",
        ]),
        "err.mic_pending" => pick(lang, [
            "Concede acceso al micrófono y vuelve a intentarlo",
            "Grant microphone access and try again",
            "Conceda acesso ao microfone e tente novamente",
            "Autorisez le micro puis réessayez",
            "Erlaube den Mikrofonzugriff und versuche es erneut",
            "Concedi l'accesso al microfono e riprova",
        ]),
        "err.provider_unknown" => pick(lang, [
            "Proveedor desconocido: {p}",
            "Unknown provider: {p}",
            "Provedor desconhecido: {p}",
            "Fournisseur inconnu : {p}",
            "Unbekannter Anbieter: {p}",
            "Provider sconosciuto: {p}",
        ]),
        "err.ax_denied" => pick(lang, [
            "Sin permiso de Accesibilidad: el texto quedó copiado en el portapapeles",
            "No Accessibility permission: the text was copied to the clipboard",
            "Sem permissão de Acessibilidade: o texto foi copiado para a área de transferência",
            "Pas d'autorisation d'Accessibilité : le texte a été copié dans le presse-papiers",
            "Keine Bedienungshilfen-Berechtigung: Der Text wurde in die Zwischenablage kopiert",
            "Nessun permesso di Accessibilità: il testo è stato copiato negli appunti",
        ]),
        "err.paste_failed" => pick(lang, [
            "No se pudo pegar ({e}); el texto quedó en el portapapeles",
            "Could not paste ({e}); the text is in the clipboard",
            "Não foi possível colar ({e}); o texto ficou na área de transferência",
            "Impossible de coller ({e}) ; le texte est dans le presse-papiers",
            "Einfügen fehlgeschlagen ({e}); der Text liegt in der Zwischenablage",
            "Impossibile incollare ({e}); il testo è negli appunti",
        ]),
        "err.copy_failed" => pick(lang, [
            "No se pudo copiar al portapapeles: {e}",
            "Could not copy to the clipboard: {e}",
            "Não foi possível copiar para a área de transferência: {e}",
            "Impossible de copier dans le presse-papiers : {e}",
            "Kopieren in die Zwischenablage fehlgeschlagen: {e}",
            "Impossibile copiare negli appunti: {e}",
        ]),
        "err.temp_write" => pick(lang, [
            "No se pudo escribir el audio temporal: {e}",
            "Could not write the temporary audio: {e}",
            "Não foi possível gravar o áudio temporário: {e}",
            "Impossible d'écrire l'audio temporaire : {e}",
            "Temporäre Audiodatei konnte nicht geschrieben werden: {e}",
            "Impossibile scrivere l'audio temporaneo: {e}",
        ]),
        "err.retry_hint" => pick(lang, [
            "{e}. Puedes reintentar desde el menú de la barra.",
            "{e}. You can retry from the menu bar.",
            "{e}. Você pode tentar novamente pelo menu.",
            "{e}. Vous pouvez réessayer depuis la barre de menus.",
            "{e}. Du kannst es über die Menüleiste erneut versuchen.",
            "{e}. Puoi riprovare dal menu.",
        ]),
        "err.hotkey_failed" => pick(lang, [
            "No se pudo usar el atajo «{k}»; se usa {d}",
            "Could not use the shortcut “{k}”; using {d}",
            "Não foi possível usar o atalho «{k}»; usando {d}",
            "Impossible d'utiliser le raccourci « {k} » ; utilisation de {d}",
            "Kurzbefehl „{k}“ nicht verfügbar; {d} wird verwendet",
            "Impossibile usare la scorciatoia «{k}»; si usa {d}",
        ]),

        // --- Errores de audio ---
        "audio.no_device" => pick(lang, [
            "No se encontró ningún micrófono",
            "No microphone found",
            "Nenhum microfone encontrado",
            "Aucun micro trouvé",
            "Kein Mikrofon gefunden",
            "Nessun microfono trovato",
        ]),
        "audio.device_not_found" => pick(lang, [
            "No se encontró el micrófono «{d}»; revisa la configuración",
            "Microphone “{d}” not found; check your settings",
            "Microfone «{d}» não encontrado; verifique as configurações",
            "Micro « {d} » introuvable ; vérifiez les réglages",
            "Mikrofon „{d}“ nicht gefunden; prüfe die Einstellungen",
            "Microfono «{d}» non trovato; controlla le impostazioni",
        ]),
        "audio.permission" => pick(lang, [
            "Sin permiso para usar el micrófono",
            "No permission to use the microphone",
            "Sem permissão para usar o microfone",
            "Pas d'autorisation d'utiliser le micro",
            "Keine Berechtigung für das Mikrofon",
            "Nessun permesso per usare il microfono",
        ]),
        "audio.open" => pick(lang, [
            "No se pudo abrir el micrófono: {e}",
            "Could not open the microphone: {e}",
            "Não foi possível abrir o microfone: {e}",
            "Impossible d'ouvrir le micro : {e}",
            "Mikrofon konnte nicht geöffnet werden: {e}",
            "Impossibile aprire il microfono: {e}",
        ]),
        "audio.stream" => pick(lang, [
            "Error durante la grabación: {e}",
            "Error while recording: {e}",
            "Erro durante a gravação: {e}",
            "Erreur pendant l'enregistrement : {e}",
            "Fehler bei der Aufnahme: {e}",
            "Errore durante la registrazione: {e}",
        ]),
        "audio.unavailable" => pick(lang, [
            "El hilo de audio no responde",
            "The audio thread is not responding",
            "A thread de áudio não responde",
            "Le thread audio ne répond pas",
            "Der Audio-Thread reagiert nicht",
            "Il thread audio non risponde",
        ]),

        // --- Errores de transcripción ---
        "tr.missing_key" => pick(lang, [
            "Falta la API key del proveedor",
            "The provider API key is missing",
            "Falta a chave de API do provedor",
            "La clé API du fournisseur est manquante",
            "Der API-Schlüssel des Anbieters fehlt",
            "Manca l'API key del provider",
        ]),
        "tr.unauthorized" => pick(lang, [
            "API key inválida o sin autorización",
            "Invalid or unauthorized API key",
            "Chave de API inválida ou sem autorização",
            "Clé API invalide ou non autorisée",
            "Ungültiger oder nicht autorisierter API-Schlüssel",
            "API key non valida o non autorizzata",
        ]),
        "tr.rate" => pick(lang, [
            "Límite de uso del proveedor alcanzado; espera unos segundos",
            "Provider rate limit reached; wait a few seconds",
            "Limite do provedor atingido; aguarde alguns segundos",
            "Limite du fournisseur atteinte ; patientez quelques secondes",
            "Anbieter-Limit erreicht; warte einige Sekunden",
            "Limite del provider raggiunto; attendi qualche secondo",
        ]),
        "tr.network" => pick(lang, [
            "Sin conexión con el servicio de transcripción",
            "No connection to the transcription service",
            "Sem conexão com o serviço de transcrição",
            "Pas de connexion au service de transcription",
            "Keine Verbindung zum Transkriptionsdienst",
            "Nessuna connessione al servizio di trascrizione",
        ]),
        "tr.timeout" => pick(lang, [
            "El servicio tardó demasiado en responder",
            "The service took too long to respond",
            "O serviço demorou demais para responder",
            "Le service a mis trop de temps à répondre",
            "Der Dienst hat zu lange gebraucht",
            "Il servizio ha impiegato troppo tempo",
        ]),
        "tr.server" => pick(lang, [
            "Error del servidor ({s})",
            "Server error ({s})",
            "Erro do servidor ({s})",
            "Erreur du serveur ({s})",
            "Serverfehler ({s})",
            "Errore del server ({s})",
        ]),
        "tr.rejected" => pick(lang, [
            "El proveedor rechazó la petición: {e}",
            "The provider rejected the request: {e}",
            "O provedor rejeitou a solicitação: {e}",
            "Le fournisseur a rejeté la requête : {e}",
            "Der Anbieter hat die Anfrage abgelehnt: {e}",
            "Il provider ha rifiutato la richiesta: {e}",
        ]),
        "tr.invalid" => pick(lang, [
            "Respuesta inesperada del proveedor",
            "Unexpected response from the provider",
            "Resposta inesperada do provedor",
            "Réponse inattendue du fournisseur",
            "Unerwartete Antwort des Anbieters",
            "Risposta inattesa dal provider",
        ]),
        "tr.io" => pick(lang, [
            "No se pudo leer el audio: {e}",
            "Could not read the audio: {e}",
            "Não foi possível ler o áudio: {e}",
            "Impossible de lire l'audio : {e}",
            "Audio konnte nicht gelesen werden: {e}",
            "Impossibile leggere l'audio: {e}",
        ]),

        other => other,
    }
}

/// Como `t`, sustituyendo marcadores `{nombre}` por los valores dados.
pub fn tf(lang: &str, key: &str, args: &[(&str, &str)]) -> String {
    let mut out = t(lang, key).to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_spanish_and_english() {
        assert_eq!(t("es", "status.idle"), "Listo");
        assert_eq!(t("en", "status.idle"), "Ready");
        assert_eq!(t("de", "status.idle"), "Bereit");
        // Un idioma no listado usa español (valor por defecto de `pick`).
        assert_eq!(t("xx", "status.idle"), "Listo");
        // Una clave desconocida se devuelve tal cual, para que el fallo sea visible.
        assert_eq!(t("es", "clave.inexistente"), "clave.inexistente");
    }

    #[test]
    fn interpolates_placeholders() {
        assert_eq!(tf("en", "err.provider_unknown", &[("p", "groq")]), "Unknown provider: groq");
        assert_eq!(tf("es", "tray.hint", &[("k", "⌥⇧Espacio")]), "Mantén ⌥⇧Espacio y habla");
    }

    #[test]
    fn resolve_normalizes_codes() {
        assert_eq!(resolve("es-419"), "es");
        assert_eq!(resolve("pt_BR"), "pt");
        assert_eq!(resolve("ja"), "en");
        assert_eq!(resolve("IT"), "it");
    }

    #[test]
    fn every_language_has_all_keys() {
        // Comprueba que ninguna traducción quedó vacía.
        for key in ["status.idle", "tray.quit", "msg.pasted", "err.ax_denied", "tr.timeout"] {
            for lang in LANGS {
                assert!(!t(lang, key).is_empty(), "{lang}/{key} vacío");
            }
        }
    }
}
