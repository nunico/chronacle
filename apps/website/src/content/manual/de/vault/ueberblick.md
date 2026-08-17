---
translationKey: vault.overview
locale: de
slug: vault/ueberblick
title: Einen Markdown-Tresor führen
navTitle: Tresorüberblick
summary: Halte Chronacle-Datensätze als lokale Markdown-Dateien fest und bearbeite sie auch im Texteditor.
section: vault
order: 1
headings:
  - id: ordner-verbinden
    text: Ordner verbinden
    level: 2
  - id: was-chronacle-verwaltet
    text: Was Chronacle verwaltet
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: vor-dem-start
    text: Vor dem Start
    level: 2
---

<!-- German proofreading requested -->

Ein Markdown-Tresor spiegelt Kampagnenentitäten und -sitzungen sowie Sammlungsentitäten und kompilierte Sammlungsregeln in einen Ordner auf diesem Computer. Änderungen an den unterstützten, dir gehörenden Teilen können zurück nach Chronacle fließen.

<h2 id="ordner-verbinden">Ordner verbinden</h2>

1. Sichere einen bestehenden Ordner, bevor du ihn verwendest.
2. Öffne **Einstellungen → Markdown-Tresor** und wähle **Ordner wählen…**.
3. Wähle einen lokalen Ordner. Chronacle prüft ihn sofort und schreibt seine Datensätze unter `campaigns/` und `collections/`.
4. Nach späteren Änderungen kannst du mit **Jetzt synchronisieren** eine vollständige Prüfung starten. Chronacle prüft außerdem beim Start und beobachtet den verbundenen Ordner, solange die App läuft.

**Ergebnis:** Der Bereich meldet, wie viele Datensätze exportiert, unverändert, übernommen, in Konflikt, gelöst, weich gelöscht, ungültig oder fehlgeschlagen sind.

<h2 id="was-chronacle-verwaltet">Was Chronacle verwaltet</h2>

Nur vier Ordnerformen werden verwaltet: Kampagnenentitäten, Kampagnensitzungen, Sammlungsentitäten und Sammlungsregeln. Dateien an anderen Stellen im gewählten Ordner — auch im Tresorhauptordner und in `.obsidian/` — werden ignoriert.

Deine Zusammenfassungen, Notizen, Tischnotizen und alternativen Namen gehören dir. Chronacle verwaltet die Dateiidentität und andere erzeugte Metadaten sowie Text im markierten kompilierten Block. Lies [Das Dateiformat verstehen](/de/handbuch/vault/dateiformat), bevor du Dateien bearbeitest, und [Konflikte behandeln](/de/handbuch/vault/konflikte), bevor du eine `.conflict.md`-Datei löschst.

<h2 id="beispiel">Beispiel</h2>

Nach dem Verbinden von `Valdris Notes` schreibt Chronacle Mara Venn nach `campaigns/shadows-of-valdris/entities/npc/mara-venn.md`. Du ergänzt unter **Notes** im Texteditor: „Hat der Gruppe sicheres Geleit durch North Quay versprochen.“ Bei der nächsten Prüfung erscheint die Notiz in Maras Chronacle-Datensatz und wird durchsuchbar.

<h2 id="vor-dem-start">Vor dem Start</h2>

- Diese Funktion arbeitet mit dem lokalen Ordner, den du in den Einstellungen auswählst.
- **Trennen** beendet Beobachtung und Schreiben durch Chronacle; die Dateien werden nicht entfernt.
- Das Verschieben, Löschen oder Bearbeiten verwalteter Dateien kann Chronacle-Datensätze ändern oder ausblenden. Lege vor größeren Dateiaktionen eine getrennte Sicherung an.
- Für Ordnerwechsel gelten besondere Wiederherstellungsregeln. Lies zuerst [Den Tresorordner wechseln](/de/handbuch/vault/ordner-wechseln).
