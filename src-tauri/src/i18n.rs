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
        "err.cloud_signin_required" => pick(lang, [
            "Inicia sesión o activa tu licencia Pro para usar la nube de Dictámelo.",
            "Sign in or activate your Pro license to use Dictámelo Cloud.",
            "Entre ou ative sua licença Pro para usar a nuvem do Dictámelo.",
            "Connectez-vous ou activez votre licence Pro pour utiliser le cloud Dictámelo.",
            "Melde dich an oder aktiviere deine Pro-Lizenz, um die Dictámelo-Cloud zu nutzen.",
            "Accedi o attiva la tua licenza Pro per usare il cloud di Dictámelo.",
        ]),
        // --- Estados ---
        "status.idle" => pick(lang, ["Listo", "Ready", "Pronto", "Prêt", "Bereit", "Pronto"]),
        "status.recording" => pick(lang, ["Grabando…", "Recording…", "Gravando…", "Enregistrement…", "Aufnahme…", "Registrazione…"]),
        "status.transcribing" => pick(lang, ["Transcribiendo…", "Transcribing…", "Transcrevendo…", "Transcription…", "Transkription…", "Trascrizione…"]),
        "status.pasting" => pick(lang, ["Pegando…", "Pasting…", "Colando…", "Collage…", "Einfügen…", "Incollaggio…"]),
        "status.cleaning" => pick(lang, ["Limpiando…", "Cleaning up…", "Limpando…", "Nettoyage…", "Bereinigung…", "Pulizia…"]),
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
        "tray.check_updates" => pick(lang, [
            "Buscar actualizaciones…", "Check for Updates…", "Buscar atualizações…",
            "Rechercher des mises à jour…", "Nach Updates suchen…", "Cerca aggiornamenti…",
        ]),
        "tray.retry" => pick(lang, [
            "Reintentar última transcripción",
            "Retry last transcription",
            "Tentar novamente a última transcrição",
            "Réessayer la dernière transcription",
            "Letzte Transkription wiederholen",
            "Riprova l'ultima trascrizione",
        ]),
        "tray.autopaste" => pick(lang, ["Pegado automático", "Auto-paste", "Colagem automática", "Collage automatique", "Automatisch einfügen", "Incolla automatico"]),
        "tray.quit" => pick(lang, ["Salir de Dictámelo", "Quit Dictámelo", "Sair do Dictámelo", "Quitter Dictámelo", "Dictámelo beenden", "Esci da Dictámelo"]),

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
        "msg.cancelled" => pick(lang, ["Grabación cancelada", "Recording cancelled", "Gravação cancelada", "Enregistrement annulé", "Aufnahme abgebrochen", "Registrazione annullata"]),
        "msg.pasted_uncleaned" => pick(lang, [
            "Texto pegado sin limpiar (la limpieza con IA falló)",
            "Text pasted without cleanup (AI cleanup failed)",
            "Texto colado sem limpeza (a limpeza com IA falhou)",
            "Texte collé sans nettoyage (le nettoyage par IA a échoué)",
            "Text ohne Bereinigung eingefügt (KI-Bereinigung fehlgeschlagen)",
            "Testo incollato senza pulizia (la pulizia con IA è fallita)",
        ]),
        "msg.nothing_retry" => pick(lang, ["No hay nada que reintentar", "Nothing to retry", "Nada para tentar novamente", "Rien à réessayer", "Nichts zu wiederholen", "Niente da riprovare"]),

        // --- Errores del flujo ---
        // Variantes para Windows (Administrador de credenciales, Configuración, bandeja); en
        // macOS estas claves siguen usando los textos de más abajo.
        "err.keychain" if cfg!(target_os = "windows") => pick(lang, [
            "No se pudo leer la API key del Administrador de credenciales: {e}",
            "Could not read the API key from Credential Manager: {e}",
            "Não foi possível ler a chave de API do Gerenciador de Credenciais: {e}",
            "Impossible de lire la clé API du Gestionnaire d'informations d'identification : {e}",
            "API-Schlüssel konnte nicht aus der Anmeldeinformationsverwaltung gelesen werden: {e}",
            "Impossibile leggere l'API key da Gestione credenziali: {e}",
        ]),
        "err.mic_denied" if cfg!(target_os = "windows") => pick(lang, [
            "Sin acceso al micrófono. Actívalo en Configuración → Privacidad y seguridad → Micrófono",
            "No microphone access. Enable it in Settings → Privacy & security → Microphone",
            "Sem acesso ao microfone. Ative em Configurações → Privacidade e segurança → Microfone",
            "Pas d'accès au micro. Activez-le dans Paramètres → Confidentialité et sécurité → Microphone",
            "Kein Mikrofonzugriff. Aktiviere ihn unter Einstellungen → Datenschutz und Sicherheit → Mikrofon",
            "Nessun accesso al microfono. Attivalo in Impostazioni → Privacy e sicurezza → Microfono",
        ]),
        "err.retry_hint" if cfg!(target_os = "windows") => pick(lang, [
            "{e}. Puedes reintentar desde el ícono de la bandeja.",
            "{e}. You can retry from the tray icon.",
            "{e}. Você pode tentar novamente pelo ícone da bandeja.",
            "{e}. Vous pouvez réessayer depuis l'icône de la zone de notification.",
            "{e}. Du kannst es über das Symbol im Infobereich erneut versuchen.",
            "{e}. Puoi riprovare dall'icona nell'area di notifica.",
        ]),
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

        // --- Archivos ---
        "file.unsupported" => pick(lang, [
            "Formato no compatible. Conviértelo a MP3, M4A o WAV.",
            "Unsupported format. Convert it to MP3, M4A or WAV.",
            "Formato não suportado. Converta para MP3, M4A ou WAV.",
            "Format non pris en charge. Convertissez-le en MP3, M4A ou WAV.",
            "Format nicht unterstützt. Wandle es in MP3, M4A oder WAV um.",
            "Formato non supportato. Convertilo in MP3, M4A o WAV.",
        ]),
        "file.convert_failed" => pick(lang, [
            "No se pudo convertir el audio: {e}",
            "Could not convert the audio: {e}",
            "Não foi possível converter o áudio: {e}",
            "Impossible de convertir l'audio : {e}",
            "Audio konnte nicht konvertiert werden: {e}",
            "Impossibile convertire l'audio: {e}",
        ]),
        "file.read_failed" => pick(lang, [
            "No se pudo leer el archivo: {e}",
            "Could not read the file: {e}",
            "Não foi possível ler o arquivo: {e}",
            "Impossible de lire le fichier : {e}",
            "Datei konnte nicht gelesen werden: {e}",
            "Impossibile leggere il file: {e}",
        ]),
        "file.dialog_unavailable" => pick(lang, [
            "El selector de archivos del sistema no responde. Puedes importar por ruta o copiar la transcripción.",
            "The system file dialog is unavailable. You can import by path or copy the transcript.",
            "O seletor de arquivos do sistema não responde. Você pode importar pelo caminho ou copiar a transcrição.",
            "Le sélecteur de fichiers du système ne répond pas. Importez depuis un chemin ou copiez la transcription.",
            "Der Dateidialog des Systems antwortet nicht. Du kannst einen Pfad importieren oder die Transkription kopieren.",
            "Il selettore file del sistema non risponde. Puoi importare da un percorso o copiare la trascrizione.",
        ]),
        "file.path_not_found" => pick(lang, [
            "No encontramos un archivo en esa ruta. Pega la ruta completa de un archivo de audio.",
            "No file was found at that path. Paste the full path of an audio file.",
            "Nenhum arquivo foi encontrado nesse caminho. Cole o caminho completo de um arquivo de áudio.",
            "Aucun fichier trouvé à cet emplacement. Collez le chemin complet d’un fichier audio.",
            "Unter diesem Pfad wurde keine Datei gefunden. Füge den vollständigen Pfad einer Audiodatei ein.",
            "Nessun file trovato in quel percorso. Incolla il percorso completo di un file audio.",
        ]),
        "file.cleanup_failed" => pick(lang, [
            "No se pudo completar la limpieza con IA. Conservamos la transcripción original.",
            "AI cleanup could not finish. The original transcript was preserved.",
            "Não foi possível concluir a limpeza com IA. A transcrição original foi preservada.",
            "Le nettoyage IA n’a pas pu aboutir. La transcription originale a été conservée.",
            "Die KI-Bereinigung konnte nicht abgeschlossen werden. Das Originaltranskript wurde beibehalten.",
            "La pulizia IA non è stata completata. La trascrizione originale è stata conservata.",
        ]),
        "file.empty" => pick(lang, [
            "El archivo no contiene audio",
            "The file contains no audio",
            "O arquivo não contém áudio",
            "Le fichier ne contient pas d'audio",
            "Die Datei enthält kein Audio",
            "Il file non contiene audio",
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

        // Local inference and model management use safe, translated messages.
        "local.error.missing" => pick(lang, [
            "Descarga primero el modelo local seleccionado.", "Download the selected local model first.",
            "Baixe primeiro o modelo local selecionado.", "Téléchargez d'abord le modèle local sélectionné.",
            "Lade zuerst das ausgewählte lokale Modell herunter.", "Scarica prima il modello locale selezionato.",
        ]),
        "local.error.unknown" => pick(lang, [
            "Este modelo local ya no está disponible. Selecciona otro.", "This local model is no longer available. Select another one.",
            "Este modelo local não está mais disponível. Selecione outro.", "Ce modèle local n'est plus disponible. Sélectionnez-en un autre.",
            "Dieses lokale Modell ist nicht mehr verfügbar. Wähle ein anderes.", "Questo modello locale non è più disponibile. Selezionane un altro.",
        ]),
        "local.error.mac_only" => pick(lang, [
            "Este equipo no admite modelos locales.", "Local models are not supported on this device.",
            "Este dispositivo não oferece suporte a modelos locais.", "Cet appareil ne prend pas en charge les modèles locaux.",
            "Lokale Modelle werden auf diesem Gerät nicht unterstützt.", "Questo dispositivo non supporta i modelli locali.",
        ]),
        "local.error.language_required" => pick(lang, [
            "Selecciona el idioma del audio en las opciones de este modelo.", "Select the audio language in this model's options.",
            "Selecione o idioma do áudio nas opções deste modelo.", "Sélectionnez la langue de l'audio dans les options de ce modèle.",
            "Wähle die Audiosprache in den Optionen dieses Modells.", "Seleziona la lingua dell'audio nelle opzioni di questo modello.",
        ]),
        "local.error.language_unsupported" => pick(lang, [
            "Este modelo no admite el idioma seleccionado. Cambia el idioma o el modelo.", "This model does not support the selected language. Change the language or model.",
            "Este modelo não aceita o idioma selecionado. Altere o idioma ou o modelo.", "Ce modèle ne prend pas en charge la langue sélectionnée. Changez de langue ou de modèle.",
            "Dieses Modell unterstützt die ausgewählte Sprache nicht. Ändere die Sprache oder das Modell.", "Questo modello non supporta la lingua selezionata. Cambia lingua o modello.",
        ]),
        "local.error.disk_space" => pick(lang, [
            "No hay suficiente espacio en disco. Libera espacio e intenta descargar de nuevo.", "There is not enough disk space. Free up space and try downloading again.",
            "Não há espaço suficiente no disco. Libere espaço e tente baixar novamente.", "L'espace disque est insuffisant. Libérez de l'espace puis réessayez le téléchargement.",
            "Nicht genügend Speicherplatz. Gib Speicherplatz frei und versuche den Download erneut.", "Spazio su disco insufficiente. Libera spazio e riprova il download.",
        ]),
        "local.error.download_busy" => pick(lang, [
            "Ya hay una descarga en curso. Espera a que termine o cancélala.", "A download is already in progress. Wait for it to finish or cancel it.",
            "Já há um download em andamento. Aguarde ou cancele-o.", "Un téléchargement est déjà en cours. Attendez la fin ou annulez-le.",
            "Ein Download läuft bereits. Warte auf den Abschluss oder brich ihn ab.", "Un download è già in corso. Attendi che finisca o annullalo.",
        ]),
        "local.error.inference_busy" => pick(lang, [
            "El modelo local está transcribiendo. Espera a que termine e inténtalo de nuevo.", "The local model is transcribing. Wait for it to finish and try again.",
            "O modelo local está transcrevendo. Aguarde e tente novamente.", "Le modèle local effectue une transcription. Attendez la fin puis réessayez.",
            "Das lokale Modell transkribiert gerade. Warte auf den Abschluss und versuche es erneut.", "Il modello locale sta trascrivendo. Attendi che finisca e riprova.",
        ]),
        "local.error.cancel_download" => pick(lang, [
            "Cancela la descarga antes de eliminar este modelo.", "Cancel the download before removing this model.",
            "Cancele o download antes de remover este modelo.", "Annulez le téléchargement avant de supprimer ce modèle.",
            "Brich den Download ab, bevor du dieses Modell entfernst.", "Annulla il download prima di rimuovere questo modello.",
        ]),
        "local.error.store_busy" => pick(lang, [
            "Otra instancia de Dictámelo está usando los modelos locales. Ciérrala e inténtalo de nuevo.", "Another Dictámelo instance is using the local models. Close it and try again.",
            "Outra instância do Dictámelo está usando os modelos locais. Feche-a e tente novamente.", "Une autre instance de Dictámelo utilise les modèles locaux. Fermez-la puis réessayez.",
            "Eine andere Dictámelo-Instanz verwendet die lokalen Modelle. Schließe sie und versuche es erneut.", "Un'altra istanza di Dictámelo sta usando i modelli locali. Chiudila e riprova.",
        ]),
        "local.error.download_failed" => pick(lang, [
            "No se pudo descargar el modelo. Revisa tu conexión e inténtalo de nuevo.", "The model could not be downloaded. Check your connection and try again.",
            "Não foi possível baixar o modelo. Verifique sua conexão e tente novamente.", "Le modèle n'a pas pu être téléchargé. Vérifiez votre connexion puis réessayez.",
            "Das Modell konnte nicht heruntergeladen werden. Prüfe deine Verbindung und versuche es erneut.", "Impossibile scaricare il modello. Controlla la connessione e riprova.",
        ]),
        "local.error.damaged" => pick(lang, [
            "El archivo del modelo está incompleto o dañado. Vuelve a descargarlo desde Modelos.", "The model file is incomplete or damaged. Download it again from Models.",
            "O arquivo do modelo está incompleto ou danificado. Baixe-o novamente em Modelos.", "Le fichier du modèle est incomplet ou endommagé. Téléchargez-le à nouveau depuis Modèles.",
            "Die Modelldatei ist unvollständig oder beschädigt. Lade sie unter Modelle erneut herunter.", "Il file del modello è incompleto o danneggiato. Scaricalo di nuovo da Modelli.",
        ]),
        "local.error.save_failed" => pick(lang, [
            "No se pudo guardar el modelo. Revisa el espacio disponible y los permisos de la carpeta.", "The model could not be saved. Check available space and folder permissions.",
            "Não foi possível salvar o modelo. Verifique o espaço disponível e as permissões da pasta.", "Le modèle n'a pas pu être enregistré. Vérifiez l'espace disponible et les autorisations du dossier.",
            "Das Modell konnte nicht gespeichert werden. Prüfe den freien Speicherplatz und die Ordnerberechtigungen.", "Impossibile salvare il modello. Controlla lo spazio disponibile e i permessi della cartella.",
        ]),
        "local.error.load_failed" => pick(lang, [
            "No se pudo cargar el modelo. Cierra otras aplicaciones o prueba un modelo más pequeño.", "The model could not be loaded. Close other apps or try a smaller model.",
            "Não foi possível carregar o modelo. Feche outros aplicativos ou tente um modelo menor.", "Le modèle n'a pas pu être chargé. Fermez d'autres applications ou essayez un modèle plus petit.",
            "Das Modell konnte nicht geladen werden. Schließe andere Apps oder versuche ein kleineres Modell.", "Impossibile caricare il modello. Chiudi altre app o prova un modello più piccolo.",
        ]),
        "local.error.audio_invalid" => pick(lang, [
            "No se pudo leer el audio para la transcripción local. Selecciona de nuevo el archivo.", "The audio could not be read for local transcription. Select the file again.",
            "Não foi possível ler o áudio para transcrição local. Selecione o arquivo novamente.", "L'audio n'a pas pu être lu pour la transcription locale. Sélectionnez à nouveau le fichier.",
            "Die Audiodatei konnte nicht für die lokale Transkription gelesen werden. Wähle sie erneut aus.", "Impossibile leggere l'audio per la trascrizione locale. Seleziona di nuovo il file.",
        ]),
        "local.error.audio_long" => pick(lang, [
            "Este audio es demasiado largo para una sola transcripción local. Súbelo desde Archivos para dividirlo automáticamente.", "This audio is too long for a single local transcription. Add it through Files to split it automatically.",
            "Este áudio é longo demais para uma única transcrição local. Adicione-o em Arquivos para dividi-lo automaticamente.", "Cet audio est trop long pour une seule transcription locale. Ajoutez-le depuis Fichiers pour le diviser automatiquement.",
            "Diese Aufnahme ist für eine einzelne lokale Transkription zu lang. Füge sie unter Dateien hinzu, um sie automatisch aufzuteilen.", "Questo audio è troppo lungo per una singola trascrizione locale. Aggiungilo da File per dividerlo automaticamente.",
        ]),
        "local.error.inference_failed" => pick(lang, [
            "No se pudo completar la transcripción local. Inténtalo de nuevo o elige otro modelo.", "Local transcription could not finish. Try again or select another model.",
            "Não foi possível concluir a transcrição local. Tente novamente ou selecione outro modelo.", "La transcription locale n'a pas pu aboutir. Réessayez ou sélectionnez un autre modèle.",
            "Die lokale Transkription konnte nicht abgeschlossen werden. Versuche es erneut oder wähle ein anderes Modell.", "Impossibile completare la trascrizione locale. Riprova o seleziona un altro modello.",
        ]),
        "local.error.cancelled" => pick(lang, [
            "Operación cancelada.", "Operation cancelled.", "Operação cancelada.", "Opération annulée.", "Vorgang abgebrochen.", "Operazione annullata.",
        ]),
        "local.error.generic" => pick(lang, [
            "No se pudo completar la operación con el modelo local. Inténtalo de nuevo.", "The local model operation could not finish. Please try again.",
            "Não foi possível concluir a operação com o modelo local. Tente novamente.", "L'opération sur le modèle local n'a pas pu aboutir. Veuillez réessayer.",
            "Der Vorgang mit dem lokalen Modell konnte nicht abgeschlossen werden. Bitte versuche es erneut.", "Impossibile completare l'operazione con il modello locale. Riprova.",
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
        for key in ["status.idle", "tray.quit", "tray.check_updates", "msg.pasted", "err.ax_denied", "tr.timeout"] {
            for lang in LANGS {
                assert!(!t(lang, key).is_empty(), "{lang}/{key} vacío");
            }
        }
    }
}
