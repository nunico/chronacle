---
translationKey: vault.files
locale: de
slug: vault/dateiformat
title: Tresordateien verstehen
navTitle: Dateiformat
summary: Bearbeite die Teile, die dir gehören, und lasse Identität sowie erzeugte Inhalte unverändert.
section: vault
order: 2
headings:
  - id: datei-lesen
    text: Die Datei lesen
    level: 2
  - id: sichere-bereiche-bearbeiten
    text: Sichere Bereiche bearbeiten
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: schaeden-vermeiden
    text: Schäden vermeiden
    level: 2
---

<!-- German proofreading requested -->

Bearbeite Zusammenfassungen, Notizen, Tischnotizen und alternative Namen. Lass `id`, erzeugte Metadaten und den abgegrenzten kompilierten Text unverändert: Nach der Übernahme deiner unterstützten Änderungen stellt Chronacle seine maßgebliche Fassung wieder her.

<h2 id="datei-lesen">Die Datei lesen</h2>

Jede verwaltete Datei beginnt mit Metadaten zwischen zwei `---`-Zeilen. Die `id` ist die feste Verbindung zum Chronacle-Datensatz; Dateiname und Ordnerpfad sind nicht seine Identität. Entitätsdateien können danach **Summary**, einen kompilierten Block und **Notes** enthalten. Regeldateien haben einen kompilierten Block und Tischnotizen. Der Inhalt einer Sitzungsdatei besteht vollständig aus deinen Notizen und hat keinen kompilierten Block.

Chronacle verwaltet diese exakten Begrenzungszeilen:

```text
<!-- chronacle:codex-article start -- compiled; edits are not applied -->
<!-- chronacle:codex-article end -->
```

Der englische Hinweis bedeutet, dass Änderungen im kompilierten Block nicht übernommen werden.

<h2 id="sichere-bereiche-bearbeiten">Sichere Bereiche bearbeiten</h2>

1. Lass den Metadatenblock am Anfang und seine `id` bestehen.
2. Bearbeite bei einer Entität Text unter `## Summary` oder `## Notes`.
3. Ergänze bei einer Entität oder Regel alternative Namen in der Liste `aliases`; behalte den eigentlichen Namen in dieser Liste.
4. Speichere die Datei und warte auf Chronacle oder wähle **Jetzt synchronisieren**.

**Ergebnis:** Unterstützte Felder werden übernommen, danach schreibt Chronacle die Datei in seiner Standardform neu. Änderungen im kompilierten Block und an erzeugten Metadaten werden nicht übernommen.

<h2 id="beispiel">Beispiel</h2>

In Seraphina Aldrics Datei änderst du die Zusammenfassung in „Archivarin des Iron Tower und Hüterin des Dämmerungsbuchs“, ergänzt eine Tischbeobachtung unter `## Notes` und lässt den kompilierten Block unangetastet. Chronacle übernimmt Zusammenfassung und Notizen, behält seinen kompilierten Artikel und vereinheitlicht die Metadaten wieder.

<h2 id="schaeden-vermeiden">Schäden vermeiden</h2>

- Eine verwaltete Datei mit fehlenden oder unlesbaren Metadaten wird als ungültig gezählt und bei dieser Prüfung weder übernommen noch überschrieben.
- Wenn du den dir gehörenden Abschnitt Summary oder Notes entfernst, wird dieses Feld in Chronacle geleert.
- Text in einem unbekannten Inhaltsabschnitt gilt als Notiz, damit er nicht unbemerkt verloren geht.
- Kopiere niemals die `id` einer Datei in eine andere. Sichere den Ordner vor größeren Änderungen.
