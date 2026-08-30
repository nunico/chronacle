---
translationKey: codex.identity
locale: de
slug: kodex/namen-und-duplikate
title: Namen und Duplikate klären
summary: Nutze alternative Namen, prüfe unsichere Verknüpfungen und führe Datensätze nur bei derselben Sache zusammen.
section: codex
order: 6
headings:
  - id: alternativen-namen-hinzufuegen
    text: Alternativen Namen hinzufügen
    level: 2
  - id: konflikte-pruefen
    text: Konflikte prüfen
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: folgen-des-zusammenfuehrens
    text: Folgen des Zusammenführens
    level: 2
---

<!-- German proofreading requested -->

Alternative Namen lassen mehrere Bezeichnungen auf dieselbe Entität zeigen; in der Wartung prüfst du mehrdeutige Namen und mögliche Duplikate vor jeder Änderung.

<h2 id="alternativen-namen-hinzufuegen">Alternativen Namen hinzufügen</h2>

Öffne eine Entität, ergänze einen Wert unter **Alternative Namen** und speichere. Verknüpfungen mit Haupt- oder Alternativnamen können danach diese Entität finden. Eine eindeutige automatische Zuordnung erscheint unter **Automatisch verknüpft** in der Wartung; **Rückgängig** entfernt den hinzugefügten Alias.

<h2 id="konflikte-pruefen">Konflikte prüfen</h2>

1. Öffne **Wartung → Befunde** und wähle **Kampagne prüfen** für eine neue Prüfung.
2. Bei einer möglichen Namensabweichung kannst du **Vorschlag verwenden**, **Artikel erstellen**, **Quelle öffnen** oder **Schließen** wählen.
3. Bei einem **Namenskonflikt** bestimmst du über die angebotene Aktion, welche Entität den umstrittenen Alias behält.
4. Bei einem **Möglichen Duplikat** öffnest du erst **A öffnen** und **B öffnen**, bevor du **Zusammenführen** wählst.

<h2 id="beispiel">Beispiel</h2>

Deine Notiz verknüpft `[[Glockenwartin]]`, der gespeicherte NSC heißt **Iria Pell, Hüterin der Glocken**. Ergänze **Glockenwartin** als alternativen Namen oder bestätige den Vorschlag in der Wartung. Gibt es einen zweiten Datensatz **Iria Pell**, vergleiche beide und führe sie nur zusammen, wenn sie dieselbe Person sind.

<h2 id="folgen-des-zusammenfuehrens">Folgen des Zusammenführens</h2>

- Du wählst den erhaltenen Datensatz und wie Zusammenfassung und Notizen kombiniert werden.
- Beziehungen und Namen wandern zum erhaltenen Datensatz; doppelte Kanten werden zusammengelegt.
- Der andere Datensatz verschwindet aus der normalen Nutzung. Der Artikel des erhaltenen Datensatzes wird zur erneuten Kompilierung markiert.
