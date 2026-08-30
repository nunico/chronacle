---
translationKey: settings.overview
locale: de
slug: einstellungen/ueberblick
title: Einstellungen sicher verwenden
navTitle: Einstellungsüberblick
summary: Richte Sprache, Antwort- und Suchanbieter, Wartungsaktionen, Extraktion und Markdown-Tresor ein.
section: settings
order: 1
headings:
  - id: sprache-und-antwortanbieter-waehlen
    text: Sprache und Antwortanbieter wählen
    level: 2
  - id: suchanbieter-waehlen
    text: Suchanbieter wählen
    level: 2
  - id: wartung-und-tresor
    text: Wartung und Tresor
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: sicher-vorgehen
    text: Sicher vorgehen
    level: 2
---

<!-- German proofreading requested -->

In den Einstellungen legst du fest, wie Chronacle spricht, antwortet, passende Textstellen findet, Verweise pflegt und Markdown-Dateien spiegelt. Nutze für jeden Abschnitt seine eigene Speichern- oder Aktionsschaltfläche; das bloße Ändern eines Feldes aktiviert es nicht unbedingt.

<h2 id="sprache-und-antwortanbieter-waehlen">Sprache und Antwortanbieter wählen</h2>

Die **Anzeigesprache** wird sofort gespeichert. Scheitert das Speichern, stellt Chronacle die letzte gespeicherte Sprache wieder her und zeigt einen Fehler. **Automatisch** folgt einer unterstützten Systemsprache und verwendet sonst Englisch.

Unter **LLM-Anbieter** wählst du OpenAI, Anthropic, Ollama (lokal) oder einen registrierten eigenen Anbieter. Dieser Dienst formuliert Antworten aus dem von Chronacle bereitgestellten Zusammenhang. **Einstellungen speichern** versucht, Anbieter, API-Schlüssel, Modell und eine sichtbare Basis-URL zu speichern. **Speichern und verbinden** führt denselben Speicherversuch aus und ersetzt danach den aktiven Anbieter anhand der gespeicherten Werte ohne Neustart. Erscheint `Speichern fehlgeschlagen: {error}`, gehe nicht davon aus, dass deine geänderten Werte aktiv sind, selbst wenn danach `Verbunden: {provider}` erscheint; behebe den Speicherfehler und versuche es erneut. Eine fehlgeschlagene Aktivierung erscheint als `Verbindung fehlgeschlagen: {error}`.

OpenAI, Anthropic und eigene Anbieter benötigen in diesem Verbindungsformular einen API-Schlüssel; Ollama nicht. Eine nicht leere Basis-URL muss eine gültige URL sein. Bei einem eingebauten Anbieter verwendet ein leeres Modellfeld Chronacles fest einprogrammierten Kompatibilitätsstandard, nicht einen vom Anbieter gewählten Standard. Wähle bei einem eigenen Anbieter ein Modell, das du registriert hast und das der Endpunkt unterstützt. Beginne mit [Einen KI-Anbieter auswählen](/de/handbuch/ki-anbieter/auswahl) oder [Einen eigenen Anbieter einrichten](/de/handbuch/ki-anbieter/eigene-anbieter).

<h2 id="suchanbieter-waehlen">Suchanbieter wählen</h2>

Der **Einbettungsanbieter** legt fest, wie Dokument- und Fragetext für die Relevanzsuche vorbereitet wird. Wähle **Klein lokal — Nomic (offline)**, **Mehrsprachig lokal — E5 Base (offline)** oder **Cloud — OpenAI-kompatible API**. Der Cloud-Modus benötigt Zugangsdaten und ein Modell mit 768 Werten; seine Basis-URL ist optional.

Bei einer Cloud-Einbettung erhält der eingerichtete entfernte Einbettungsendpunkt jeden durchsuchbaren Text, den Chronacle gerade vorbereitet. Das sind je nach Vorgang: Quellabschnitte; Namen, Zusammenfassungen und Notizen von Entitäten sowie kompilierte Codex-Artikel; Sitzungstitel und -notizen; kompilierte Regeln; und Frage- oder Suchtext bei der Suche. Diese Kategorien werden gesendet, wenn Chronacle sie einbetten muss, nicht alle gemeinsam bei jeder Anfrage. Eine lokale Einbettung führt diese Berechnung auf diesem Computer aus. Dadurch wird der getrennte Antwortanbieter nicht automatisch lokal. Beim Antworten erhält ein entfernter Antwortanbieter die Frage und den von Chronacle bereitgestellten Kontext: relevante Quellenauszüge; Namen von Entitäten, Zusammenfassungen, Notizen und kompilierte Codex-Artikel; Spielernamen sowie Klasse, Stufe und Status; Start- und Enddaten von Ereignissen; Sitzungsnummern, -titel, Spieldaten und -notizen; sowie kompilierte Regeln, soweit sie für diese Frage und Kampagne verfügbar sind. Kampagnenentitäten und Sitzungen können als vollständiger kampagnenbezogener Kontext statt als relevanzgefilterte Ergebnisse enthalten sein.

**Einbettungsanbieter speichern** speichert alle vier Einbettungsfelder und aktiviert den Anbieter ohne Neustart. Bei einem noch nicht heruntergeladenen lokalen Modell bleibt der bisherige Anbieter aktiv und **Ausgewähltes Modell herunterladen** erscheint. Nach einem erfolgreichen Anbieter- oder Modellwechsel musst du **Alle Quellen neu indizieren**, damit bestehende PDFs ihn ebenfalls verwenden. Chronacle löscht die alten Passagen einer Quelle, bevor es sie neu aufbaut; es gibt keine Rücksetzung. Diese Quelle ist während des Versuchs nicht durchsuchbar und bleibt es nach einem gescheiterten Versuch, bis eine Wiederholung gelingt. Andere Quellen bleiben verfügbar.

<h2 id="wartung-und-tresor">Wartung und Tresor</h2>

- **Benutzerdefinierte Anbieter** registriert kompatible Dienste und ihre Modell-IDs. Verwende die exakte ID des jeweiligen Dienstes.
- **Beziehungsverweise neu aufbauen** liest `[[links]]` in Notizen erneut. Das hilft nach einem Notizimport oder bei älteren Entitäten.
- **Verwandte Entitäten anreichern** speichert sofort beim Umschalten. Dadurch läuft ein langsamerer zweiter Extraktionsdurchgang mit mehr Aufrufen des Antwortanbieters, begrenzt auf 20 verwandte Entitäten pro Extraktion.
- **Markdown-Tresor** verbindet einen lokalen Ordner, prüft ihn, listet Konflikte und bietet **Jetzt synchronisieren**. Lies [Einen Markdown-Tresor führen](/de/handbuch/vault/ueberblick), bevor du einen gefüllten Ordner auswählst.

<h2 id="beispiel">Beispiel</h2>

Deine Valdris-Kampagne verwendet deutsche Notizen. Stelle die **Anzeigesprache** auf Deutsch, wähle **Mehrsprachig lokal — E5 Base (offline)**, lade es bei Bedarf herunter, speichere und starte **Alle Quellen neu indizieren**. Frage danach: „Was weiß Mara Venn über den Iron Tower?“ Nach der Neuindizierung kann Chronacle die deutsche Frage mit dem Kampagnenmaterial über das gewählte Suchmodell abgleichen.

<h2 id="sicher-vorgehen">Sicher vorgehen</h2>

- Füge einen API-Schlüssel niemals in Notizen, Chat, Screenshots oder Supportnachrichten ein. Trage ihn nur in das Passwortfeld der Einstellungen ein.
- **Einstellungen speichern** und **Speichern und verbinden** haben unterschiedliche Ergebnisse; nutze die zweite Aktion, wenn der Antwortanbieter sofort aktiv sein soll.
- Kopiere den genauen angezeigten Fehler, bevor du mehrere Felder änderst. Er kann zeigen, dass Speichern, Verbindung, Download oder Neuindizierung scheiterten, beweist aber keine tieferliegende Ursache.
- Ein Wechsel des Antwortanbieters braucht keinen Neustart. Nach einem Wechsel des Suchmodells müssen bestehende Quellen für einheitliche Ergebnisse neu indiziert werden.
