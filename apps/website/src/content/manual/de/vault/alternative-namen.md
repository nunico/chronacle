---
translationKey: vault.aliases
locale: de
slug: vault/alternative-namen
title: Alternative Namen im Tresor ergänzen
navTitle: Alternative Namen
summary: Halte Spitznamen und Titel auffindbar, ohne die Identität eines Datensatzes zu ändern.
section: vault
order: 3
headings:
  - id: aliases-bearbeiten
    text: Aliases bearbeiten
    level: 2
  - id: wie-namen-uebernommen-werden
    text: Wie Namen übernommen werden
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: tipps
    text: Tipps
    level: 2
---

<!-- German proofreading requested -->

Ergänze Spitznamen, Titel und frühere Namen in der Frontmatter-Liste `aliases`. Chronacle übernimmt diese alternativen Namen und setzt den eigentlichen Namen an die erste Stelle, damit `[[Name]]`-Verweise in Markdown-Editoren mit Alias-Unterstützung funktionieren.

<h2 id="aliases-bearbeiten">Aliases bearbeiten</h2>

1. Öffne die verwaltete Datei einer Entität oder Regel.
2. Suche im Metadatenblock nach `aliases`.
3. Behalte den eigentlichen Namen und ergänze Alternativen, zum Beispiel `aliases: ["Mara Venn", "Die Laterne", "Kapitänin Venn"]`.
4. Speichere und wähle **Jetzt synchronisieren** oder lass Chronacle die Änderung am Ordner erkennen.

**Ergebnis:** Chronacle speichert „Die Laterne“ und „Kapitänin Venn“ als alternative Namen und schreibt danach eine bereinigte Liste mit „Mara Venn“ an erster Stelle.

<h2 id="wie-namen-uebernommen-werden">Wie Namen übernommen werden</h2>

Der eigentliche Name in `aliases` unterstützt Verweise, wird aber nicht als alternativer Name gespeichert. Beim Entfernen von Dopplungen unterscheidet Chronacle nur bei ASCII-Zeichen nicht zwischen Groß- und Kleinschreibung. Varianten mit Zeichen wie `Ä` und `ä` können daher getrennt bleiben. Sitzungsdateien führen ihren Titel ebenfalls in `aliases`; geänderte Sitzungs-Aliases ändern jedoch kein Feld in Chronacle.

Das Umbenennen einer Datei benennt den Datensatz nicht um. Die `id` kennzeichnet den Datensatz und Chronacle kann einen von dir geänderten Pfad beibehalten, solange er in einem unterstützten verwalteten Ordner liegt.

<h2 id="beispiel">Beispiel</h2>

Die Gruppe kennt Seraphina Aldric als „Hüterin des Dämmerungsbuchs“. Ergänze diese Bezeichnung in `aliases`. Eine Notiz mit `[[Hüterin des Dämmerungsbuchs]]` kann dann auf Seraphina verweisen, während ihr Hauptname gleich bleibt.

<h2 id="tipps">Tipps</h2>

- Setze jeden alternativen Namen in Anführungszeichen und trenne Einträge mit Kommas.
- Bearbeite `id` nicht, um Datensätze zusammenzuführen oder umzuleiten; nutze dafür Chronacles Entitätswerkzeuge.
- Beanspruchen zwei Datensätze denselben alternativen Namen, entscheide anhand des Chronacle-Befunds, welcher ihn behalten soll.
- Mehr dazu findest du unter [Namen und Duplikate](/de/handbuch/kodex/namen-und-duplikate).
