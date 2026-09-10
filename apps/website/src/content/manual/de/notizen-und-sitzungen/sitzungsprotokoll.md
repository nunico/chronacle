---
translationKey: notes.sessions
locale: de
slug: notizen-und-sitzungen/sitzungsprotokoll
title: Ein Sitzungsprotokoll führen
summary: Halte Titel, Spieldatum, verknüpfte Ereignisse und Rückblick jeder Sitzung in Kampagnenreihenfolge fest.
section: notes-and-sessions
order: 2
headings:
  - id: sitzung-festhalten
    text: Sitzung festhalten
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: speicherstatus-beobachten
    text: Speicherstatus beobachten
    level: 2
  - id: neue-sitzung-wiederherstellen
    text: Neue Sitzung wiederherstellen
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: aktuelle-grenzen
    text: Aktuelle Grenzen
    level: 2
---

<!-- German proofreading requested -->

Die Ansicht **Sitzungen** führt ein nummeriertes Kampagnenprotokoll; Titel, Datum und Notizen werden nach einem gewöhnlichen Fokuswechsel innerhalb der jeweiligen Sitzung automatisch gespeichert.

<h2 id="sitzung-festhalten">Sitzung festhalten</h2>

1. Wähle eine Kampagne und öffne **Sitzungen**.
2. Wähle **Neue Sitzung**. Chronacle setzt die nächste Sitzungsnummer, trägt ein Datum aus dem UTC-Datum des Computers ein und ergänzt einen Titel wie **Sitzung 4**. Prüfe das Datum und passe es an dein örtliches Spieldatum an.
3. Klappe die Zeile auf und bearbeite **Name**, **Gespieltes Datum** und **Notizen**.
4. Verwende `[[Entitätsname]]` im Rückblick. Wechsle den Fokus zu einem anderen Bedienelement derselben Sitzung, um das Speichern anzufordern.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Sitzungen erscheinen aufsteigend nach Sitzungsnummer. Ihr Text wird mit der Kampagne gespeichert und kann spätere Kampagnenfragen unterstützen. Beim Speichern nicht leerer Notizen können außerdem Vorschläge in **Wartung** entstehen, die du annimmst oder ablehnst.

<h2 id="speicherstatus-beobachten">Speicherstatus beobachten</h2>

Wenn du den Fokus zu einem anderen Bedienelement derselben Sitzung bewegst, fordert Chronacle automatisch das Speichern an. Wechselst du direkt zu einer anderen Sitzung oder Ansicht, bleibt die Änderung als **Ungespeicherte Änderungen** erhalten, ohne dass die Navigation als Speicheranforderung gilt. **Wird gespeichert…**, **Gespeichert** und **Speichern nicht möglich** zeigen den weiteren Fortschritt. Scheitert das Speichern, bleibt dein Text in dieser Sitzung erhalten und **Erneut versuchen** wiederholt den Vorgang für denselben Eintrag. Du kannst während des Speicherns weiterarbeiten; der Abschluss eines früheren Vorgangs kennzeichnet neueren Text nicht als gespeichert. Unter [Entwürfe behalten und Speichervorgänge wiederholen](/de/handbuch/notizen-und-sitzungen/speichern-und-wiederherstellen) findest du Hinweise zu Navigation und Schließen.

<h2 id="neue-sitzung-wiederherstellen">Neue Sitzung wiederherstellen</h2>

Wenn **Neue Sitzung** die Sitzung nicht erstellen kann, bleibt der fehlgeschlagene Versuch bei der Kampagne, in der du ihn begonnen hast. **Erneut versuchen** verwendet dieselbe vorgesehene Sitzungsnummer, denselben Titel, dasselbe Datum und dieselben Notizen. Wiederholtes Auslösen kann weder parallele Erstellungen starten noch eine doppelte Sitzung anlegen. **Änderungen verwerfen** gibt nur diesen fehlgeschlagenen Versuch auf.

Wurde die Sitzung erstellt, aber Chronacle zeigt **Erstellt, aber Eingriff erforderlich**, enthält die aufgelistete gespeicherte Sitzung bereits Änderungen, die du zuerst klären musst. Speichere oder verwirf diese widersprüchlichen Änderungen und wähle danach **Erneut versuchen** in der Meldung. Dadurch verbindet Chronacle den aufbewahrten Versuch mit der bereits erstellten Sitzung; es wird keine weitere Sitzung erstellt.

<h2 id="beispiel">Beispiel</h2>

Erstelle **Sitzung 6 — Brand am Nordkai** mit Datum 15.08.2026 und den Notizen:

> [[Mara Venn]] brachte die Gruppe mit der Fähre fort. [[Iria Pell]] barg die Messingglocke. Offen: Wer hat das Lagerhaus angezündet?

Frage später: „Was blieb nach dem Brand am Nordkai ungeklärt?“ Prüfe die Antwort anhand des gespeicherten Rückblicks.

<h2 id="aktuelle-grenzen">Aktuelle Grenzen</h2>

- Die Sitzungsnummer wird beim Erstellen vergeben und kann im aktuellen Formular nicht geändert werden.
- Die Ereigniszahl zeigt Ereignisse, die ausdrücklich dieser Sitzung zugewiesen sind, nicht jede in den Notizen verknüpfte Entität.
- **Löschen** entfernt die Sitzung nach einer Bestätigung dauerhaft.
