---
translationKey: troubleshooting.common
locale: de
slug: fehlerbehebung/haeufige-probleme
title: Häufige Probleme
navTitle: Häufige Probleme
summary: Nutze sichtbare Zustände und genaue Fehler, um Probleme bei Einrichtung, Anbietern, Quellen, Kampagnen, Kodex, Suche und Tresor sicher zu beheben.
section: troubleshooting
order: 1
headings:
  - id: modell-download-scheitert
    text: Der erste Modell-Download scheitert
    level: 2
  - id: antwortanbieter-funktioniert-nicht
    text: Der Antwortanbieter funktioniert nicht
    level: 2
  - id: pdf-import-scheitert
    text: Eine PDF lässt sich nicht importieren
    level: 2
  - id: indizierung-braucht-aufmerksamkeit
    text: Die Indizierung braucht Aufmerksamkeit
    level: 2
  - id: keine-ergebnisse
    text: Die Suche liefert keine Ergebnisse
    level: 2
  - id: suche-nicht-verfuegbar
    text: Die Suche ist nicht verfügbar
    level: 2
  - id: kampagneninhalt-fehlt
    text: Kampagneninhalt fehlt
    level: 2
  - id: kodexartikel-aendert-sich-nicht
    text: Ein Kodexartikel ändert sich nicht
    level: 2
  - id: tresor-kommt-nicht-zur-ruhe
    text: Der Markdown-Tresor kommt nicht zur Ruhe
    level: 2
---

<!-- German proofreading requested -->

Beginne mit dem genauen Zustand oder Fehler, den du sehen kannst. Eine Meldung benennt den gescheiterten Schritt; wenn sie nichts Weiteres sagt, beweist sie nicht die zugrunde liegende Ursache.

<h2 id="modell-download-scheitert">Der erste Modell-Download scheitert</h2>

**Symptom.** Die Einrichtung zeigt **Download fehlgeschlagen** oder das gewählte lokale Suchmodell ist weiterhin nicht bereit.

**Wahrscheinliche Ursache.** Chronacle konnte den Modell-Download nicht abschließen oder bestätigen. Die Meldung allein verrät nicht, ob Verbindung, Speicherplatz oder eine andere lokale Bedingung die Unterbrechung verursacht hat.

**Sichere Prüfungen.** Lass die App geöffnet, kopiere einen ausführlichen Fehler, prüfe freien Speicherplatz und eine gewöhnliche Netzwerkverbindung und sieh nach, ob **Erneut versuchen** verfügbar ist. Verschiebe oder bearbeite Chronacles Datenordner nicht.

**Wiederherstellung.** Wähle **Erneut versuchen**. Bei einer späteren Modellauswahl nutze **Einstellungen → Einbettungsanbieter → Ausgewähltes Modell herunterladen** und speichere den Anbieter erneut. Scheitern Downloads weiterhin, bewahre den exakten Fehler auf und wähle mit [Einen KI-Anbieter auswählen](/de/handbuch/ki-anbieter/auswahl) eine andere unterstützte Sucheinstellung.

<h2 id="antwortanbieter-funktioniert-nicht">Der Antwortanbieter funktioniert nicht</h2>

**Symptom.** Die Einstellungen zeigen `Verbindung fehlgeschlagen: {error}` oder eine Frage liefert einen Anbieterfehler, nachdem dort `Verbunden: {provider}` stand.

**Wahrscheinliche Ursache.** `Speichern fehlgeschlagen: {error}` bedeutet, dass die bearbeiteten Felder nicht vollständig gespeichert wurden. `Verbindung fehlgeschlagen: {error}` bedeutet, dass die Aktivierung scheiterte. `Verbunden: {provider}` bestätigt nur, dass ein Anbieter aus gespeicherten Werten aktiviert wurde; es beweist weder die Übernahme eines vorher gescheiterten Speichervorgangs noch den Erfolg einer späteren Anfrage an den Dienst.

**Sichere Prüfungen.** Prüfe, ob vor `Verbunden: {provider}` ein Speicherfehler erschien. Prüfe gewählten Anbieter, exaktes Modell und Basis-URL. OpenAI, Anthropic und eigene Anbieter benötigen im Verbindungsformular einen API-Schlüssel; Ollama nicht. Kopiere den Schlüssel niemals in Chat oder Supporttext.

**Wiederherstellung.** Korrigiere jeweils ein sichtbares Feld und nutze erneut **Speichern und verbinden**. Prüfe bei Ollama außerdem, ob der lokale Dienst und das Modell verfügbar sind. Folge [Einen Online-Anbieter einrichten](/de/handbuch/ki-anbieter/online), [Einen lokalen Anbieter verwenden](/de/handbuch/ki-anbieter/lokal) oder [Einen eigenen Anbieter einrichten](/de/handbuch/ki-anbieter/eigene-anbieter).

<h2 id="pdf-import-scheitert">Eine PDF lässt sich nicht importieren</h2>

**Symptom.** Chronacle zeigt `„{name}“ konnte nicht hochgeladen werden: {error}` oder `„{name}“ konnte nicht indiziert werden: {error}`.

**Wahrscheinliche Ursache.** Die erste Meldung bedeutet, dass die Datei nicht angenommen oder gespeichert werden konnte; die zweite, dass die Verarbeitung nach dem Hochladen scheiterte. Erst der angehängte `{error}` grenzt den Fehler weiter ein.

**Sichere Prüfungen.** Prüfe, ob du die PDF verwenden darfst, ob sie sich weiterhin normal öffnen lässt und ob nicht bereits ein anderer Upload läuft. Notiere Dateiname und vollständigen angehängten Fehler, ohne die PDF selbst weiterzugeben.

**Wiederherstellung.** Versuche diese Datei einmal erneut über **PDF hochladen**. Gelingt der Upload, aber die Indizierung scheitert wieder, bewahre Fehlerstatus und genaue Meldung der Quelle auf; lösche nicht wiederholt unbeteiligte Sammlungen. Siehe [PDFs importieren](/de/handbuch/quellenbibliothek/pdfs-importieren).

<h2 id="indizierung-braucht-aufmerksamkeit">Die Indizierung braucht Aufmerksamkeit</h2>

**Symptom.** Ein Banner meldet Quellen mit einem anderen Modell oder die Einstellungen zeigen `Neuindizierung fehlgeschlagen: {error}`.

**Wahrscheinliche Ursache.** Das Modell-Banner bedeutet, dass sich das aktive Suchmodell von dem in diesen Quellen gespeicherten unterscheidet. Die Fehlermeldung nennt nur den gescheiterten Schritt; ihr angehängter Fehler kann genauer sein.

**Sichere Prüfungen.** Prüfe unter **Einstellungen → Einbettungsanbieter** den beabsichtigten Modus und das aktive Modell. Prüfe, ob ein lokales Modell heruntergeladen ist oder die Cloud-Felder vollständig sind. Chronacle löscht die alten Passagen der aktuellen Quelle vor dem Neuaufbau; es gibt keine Rücksetzung. Deshalb ist diese Quelle während des gesamten Versuchs nicht durchsuchbar.

**Wiederherstellung.** Nutze **Jetzt neu indizieren** im Banner oder **Alle Quellen neu indizieren** in den Einstellungen und warte bis zum Abschluss des Zählers. Scheitert der Vorgang, bleibt diese Quelle aus der Suche ausgeschlossen, bis eine Wiederholung gelingt. Bewahre den genauen Fehler auf, korrigiere nur das angezeigte Anbieterproblem und versuche es erneut. Andere Quellen bleiben verfügbar. Siehe [Indizierung verstehen](/de/handbuch/quellenbibliothek/indizierung).

<h2 id="keine-ergebnisse">Die Suche liefert keine Ergebnisse</h2>

**Symptom.** Eine Handbuchsuche endet, ohne einen Artikel zu finden.

**Wahrscheinliche Ursache.** Der Suchbegriff kommt möglicherweise nicht im Index vor oder der passende Artikel steht in der anderen Handbuchsprache. Die Handbuchsuche verwendet nur die Sprache der Seite, die du gerade liest.

**Sichere Prüfungen.** Prüfe die Handbuchsprache, kürze die Suchanfrage und versuche den genauen Namen eines sichtbaren Bedienelements oder einer Funktion.

**Wiederherstellung.** Öffne den Handbuchüberblick oder navigiere direkt über die Abschnitte. Wechsle vor der Suche nach übersetzten Begriffen in die andere Handbuchsprache.

<h2 id="suche-nicht-verfuegbar">Die Suche ist nicht verfügbar</h2>

**Symptom.** Der Suchdialog des Handbuchs meldet, dass die Suche nicht verfügbar ist.

**Wahrscheinliche Ursache.** Die statischen Suchdateien wurden nicht geladen. Die Meldung nennt keine genauere Ursache.

**Sichere Prüfungen.** Lade die Seite einmal neu und prüfe, ob sich das Handbuch weiterhin normal öffnen lässt.

**Wiederherstellung.** Nutze den Handbuchüberblick oder die Abschnittsnavigation. Die Suche ist optional und blockiert die direkte Navigation nicht.

<h2 id="kampagneninhalt-fehlt">Kampagneninhalt fehlt</h2>

**Symptom.** **Keine Kampagne** wird angezeigt, Kampagnenseiten bitten um eine Auswahl oder eine Antwort verwendet eine erwartete Sammlung nicht.

**Wahrscheinliche Ursache.** Es ist keine Kampagne aktiv oder die aktive Kampagne hat diese Quellensammlung nicht abonniert. Eine Sammlung kann vorhanden sein, ohne jeder Kampagne zur Verfügung zu stehen.

**Sichere Prüfungen.** Prüfe die aktive Kampagne in der Kampagnenleiste. Öffne **Kampagne und Quellen** und prüfe, ob die erwartete Sammlung als **abonniert** und ihre Quelle als **Indiziert** statt **Wird indiziert…** oder **Fehler** markiert ist.

**Wiederherstellung.** Wähle oder erstelle die beabsichtigte Kampagne, abonniere die Sammlung und warte die Indizierung ab, bevor du erneut fragst. Siehe [Quellenzugriff einer Kampagne steuern](/de/handbuch/kampagnen/quellenzugriff).

<h2 id="kodexartikel-aendert-sich-nicht">Ein Kodexartikel ändert sich nicht</h2>

**Symptom.** Das Neukompilieren zeigt `Kein Quellkontext gefunden — Artikel unverändert`, **Artikel konnte nicht neu kompiliert werden** oder nach einer Sammlungskompilierung bleibt ein Eintrag veraltet.

**Wahrscheinliche Ursache.** Die erste Meldung bedeutet, dass Chronacle keine verwendbare Quellpassage für diese Entität fand und den bestehenden Artikel absichtlich beibehielt. Andere Fehler brauchen ihre angezeigten Einzelheiten oder den Sammlungsstatus; nimm nicht automatisch fehlendes Material als Ursache an.

**Sichere Prüfungen.** Prüfe, ob die passende Sammlung indiziert und für Kampagne oder Sammlung der Entität verfügbar ist. Prüfe Entitätsname und alternative Namen. Lass deine Zusammenfassung und Notizen während der Untersuchung unverändert.

**Wiederherstellung.** Korrigiere Quellenzugriff oder Identität und kompiliere dann den einzelnen Artikel oder die Sammlung neu. Kopiere erzeugten Text nicht allein zur Erzwingung der Kompilierung in deine Notizen; Notizen sind dein dauerhafter Datensatz. Siehe [Den Kodex kompilieren](/de/handbuch/kodex/kompilieren) und [Den Kodex warten](/de/handbuch/kodex/wartung).

<h2 id="tresor-kommt-nicht-zur-ruhe">Der Markdown-Tresor kommt nicht zur Ruhe</h2>

**Symptom.** Der Bereich zeigt `Synchronisierung fehlgeschlagen: {error}`, meldet ungültige oder fehlgeschlagene Dateien oder listet dauerhaft einen Konflikt.

**Wahrscheinliche Ursache.** `Synchronisierung fehlgeschlagen` bedeutet, dass die vollständige Ordnerprüfung scheiterte. Eine Zahl ungültiger Dateien bedeutet, dass mindestens eine verwaltete Datei nicht gelesen werden konnte. Ein gelisteter Konflikt bedeutet, dass sich beide Fassungen geändert haben. Nichts davon nennt allein die verursachende externe Aktion.

**Sichere Prüfungen.** Kopiere genauen Fehler und angezeigte Pfade, lege eine Sicherung an, prüfe, ob der gewählte Ordner noch existiert und ein Ordner ist, und kontrolliere verwaltete Dateien auf intakte `---`-Metadaten samt `id`. Vergleiche bei einem Konflikt normale Datei und `.conflict.md`-Begleitdatei.

**Wiederherstellung.** Repariere ungültige Metadaten aus einer Sicherung und wähle **Jetzt synchronisieren**. Löse einen Konflikt ausschließlich mit dem Ablauf unter [Tresorkonflikte lösen](/de/handbuch/vault/konflikte). Scheiterte ein Ordnerwechsel, bleibt der vorige Ordner aktiv; folge [Den Tresorordner wechseln](/de/handbuch/vault/ordner-wechseln), statt Dateien zu löschen.
