---
translationKey: vault.switching
locale: de
slug: vault/ordner-wechseln
title: Den Tresorordner wechseln
navTitle: Ordner wechseln
summary: Wechsle zu einem anderen lokalen Ordner, ohne dass ein leerer Zielordner wie eine Massenlöschung wirkt.
section: vault
order: 6
headings:
  - id: wechsel-vorbereiten
    text: Den Wechsel vorbereiten
    level: 2
  - id: neuen-ordner-waehlen
    text: Den neuen Ordner wählen
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: nach-fehler-wiederherstellen
    text: Nach einem Fehler wiederherstellen
    level: 2
---

<!-- German proofreading requested -->

Sichere beide Ordner und wähle dann den neuen Ordner unter **Einstellungen → Markdown-Tresor**. Einen tatsächlich anderen Pfad behandelt Chronacle als neue Vergleichsbasis und führt dort sofort eine vollständige Prüfung aus.

<h2 id="wechsel-vorbereiten">Den Wechsel vorbereiten</h2>

1. Schließe offene Arbeiten an `.conflict.md`-Dateien im bisherigen Ordner ab oder kopiere sie beiseite.
2. Beende größere Änderungen anderer Programme an beiden Ordnern.
3. Sichere den bisherigen Ordner und den Zielordner.
4. Prüfe, ob das Ziel leer ist oder bereits Chronacle-Dateien enthält. Bestehende Dateien mit passender `id` werden verglichen und nicht blind überschrieben.

<h2 id="neuen-ordner-waehlen">Den neuen Ordner wählen</h2>

1. Wähle **Ordner wählen…** und dann das Ziel.
2. Warte die sofortige Prüfung ab.
3. Prüfe Ordner und Konfliktliste, bevor du weiterarbeitest.

**Ergebnis:** Ein leeres Ziel erhält einen frischen Export. Chronacle verwirft vor der Prüfung des neuen Ordners die Vergleichsbasis des alten, damit fehlende Zieldateien nicht als Löschsignale gelten. Der alte Ordner wird nicht mehr beobachtet; beim Wechsel werden seine Dateien nicht gelöscht.

Wenn du denselben Pfad erneut wählst, bleibt die bestehende Vergleichsbasis erhalten. Chronacle prüft normale Änderungen, statt den Ordner als frisches Ziel zu behandeln.

<h2 id="beispiel">Beispiel</h2>

Du wechselst von `Valdris Notes` zum leeren Ordner `Campaign Archive`. Chronacle exportiert Mara Venn, den Iron Tower und Sitzung 012 dorthin. Die Dateien in `Valdris Notes` bleiben bestehen, spätere Chronacle-Änderungen landen aber nur in `Campaign Archive`.

<h2 id="nach-fehler-wiederherstellen">Nach einem Fehler wiederherstellen</h2>

- Kann Chronacle das Ziel nicht prüfen, wird der neue Pfad nicht gespeichert. Der vorige Pfad und seine Vergleichsbasis bleiben aktiv; auch die Beobachtung des alten Ordners wird wiederhergestellt.
- Meldet der Fehler, dass die bisherige Vergleichsbasis nicht wiederhergestellt werden konnte, kehre zum alten Ordner zurück, führe **Jetzt synchronisieren** aus und löse entstehende Konflikte. Bewahre die Sicherungen auf, bis der Ordner wieder bereinigt ist.
- **Trennen** leert den aktiven Pfad und beendet die Ordneraktivität, ohne einen der Ordner zu löschen.
- Ein erfolgreicher Wechsel kann Konflikte erzeugen, wenn Zieldateien mit passenden Identitäten andere Änderungen enthalten. Löse sie normal, statt Dateien vollständig zu ersetzen.
