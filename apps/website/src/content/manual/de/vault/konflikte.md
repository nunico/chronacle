---
translationKey: vault.conflicts
locale: de
slug: vault/konflikte
title: Tresorkonflikte lösen
navTitle: Konflikte
summary: Vergleiche beide erhaltenen Fassungen, führe sie bewusst zusammen und löse die Sperre sicher.
section: vault
order: 4
headings:
  - id: konflikt-erkennen
    text: Einen Konflikt erkennen
    level: 2
  - id: konflikt-loesen
    text: Den Konflikt lösen
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: wenn-die-loesung-scheitert
    text: Wenn die Lösung scheitert
    level: 2
---

<!-- German proofreading requested -->

Ein Konflikt bedeutet, dass sich sowohl der Chronacle-Datensatz als auch seine Markdown-Datei seit der letzten gemeinsamen Fassung geändert haben. Chronacle friert diesen Datensatz ein, lässt deine Datei unverändert und schreibt seine eigene aktuelle Fassung daneben als `<name>.conflict.md`.

<h2 id="konflikt-erkennen">Einen Konflikt erkennen</h2>

Öffne **Einstellungen → Markdown-Tresor**. Die Liste **Konflikte** zeigt Name, Art, normalen Pfad und `.conflict.md`-Pfad. Im Editor einer betroffenen Entität erscheint ebenfalls ein Konflikthinweis. Solange die Begleitdatei vorhanden ist, bestimmt auch eine wiederholte Prüfung keinen Gewinner.

<h2 id="konflikt-loesen">Den Konflikt lösen</h2>

1. Sichere beide Dateien, bevor du eine davon bearbeitest oder löschst.
2. Vergleiche die normale `.md`-Datei — deine Ordnerfassung — mit der benachbarten `.conflict.md`-Datei — Chronacles Fassung.
3. Schreibe den endgültigen Text in die unterstützten, dir gehörenden Bereiche der normalen Datei.
4. Speichere die normale Datei und lösche anschließend nur die zugehörige `.conflict.md`-Datei.
5. Wähle **Jetzt synchronisieren**.

**Ergebnis:** Chronacle übernimmt die unterstützten Felder aus der normalen Datei, hebt die Sperre auf und prüft den Datensatz wieder normal.

<h2 id="beispiel">Beispiel</h2>

Du ergänzt „Mara traf den Fährmann“ in `mara-venn.md` und änderst gleichzeitig Maras Notizen in Chronacle. Bei der nächsten Prüfung entsteht `mara-venn.conflict.md`. Vergleiche beide Dateien, vereine die Fakten unter `## Notes` in `mara-venn.md`, speichere, lösche `mara-venn.conflict.md` und starte **Jetzt synchronisieren**.

<h2 id="wenn-die-loesung-scheitert">Wenn die Lösung scheitert</h2>

- Kann Chronacle die normale Datei nach dem Löschen der Begleitdatei nicht lesen, stellt es die Begleitdatei wieder her und hält den Datensatz eingefroren. Repariere Metadaten oder Inhalt und wiederhole die sicheren Schritte.
- Entspricht die normale Datei wieder exakt Chronacles Fassung, kann der Konflikt verschwinden und die Begleitdatei automatisch entfernt werden.
- Lösche die Begleitdatei erst, wenn du benötigten Chronacle-Text übernommen hast. Das Löschen ist das ausdrückliche Signal, die normale Datei vorzuziehen.
- Ein Konflikt zeigt zwei geänderte Fassungen, aber nicht automatisch, welche Bearbeitung oder welches Programm sie verursacht hat.
