---
translationKey: sources.ingestion
locale: de
slug: quellenbibliothek/indizierung
title: Indizierung verstehen
summary: Verfolge die sichtbaren Zustände und erkenne, ob eine Quelle bereit ist, fehlgeschlagen ist oder neu indiziert werden muss.
section: source-library
order: 4
headings:
  - id: was-beim-indizieren-geschieht
    text: Was beim Indizieren geschieht
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: zustaende-und-neuindizierung
    text: Zustände und Neuindizierung
    level: 2
---

<!-- German proofreading requested -->

<h2 id="was-beim-indizieren-geschieht">Was beim Indizieren geschieht</h2>

Beim Indizieren liest Chronacle den Text des PDFs Seite für Seite, teilt ihn in durchsuchbare Textstellen und speichert einen lokalen Suchindex mit Quelle und Seitenangabe.

1. Starte einen Import und beobachte in der Fortschrittsanzeige Dateiname, Arbeitsschritt und Prozentwert.
2. Während der Verarbeitung können dort **Wird hochgeladen…**, **PDF wird indiziert…** oder genauere Arbeitsschritte stehen.
3. Warte auf **Bereit!**, bevor du das Orakel nach der neuen Quelle fragst.
4. Klappe unter **Kampagne und Quellen** die Sammlung auf und prüfe den Buchstatus: **Wird indiziert…**, **Indiziert** oder **Fehler**.
5. Nach einer Änderung des **Einbettungsmodus** öffnest du **Einstellungen** und wählst **Alle Quellen neu indizieren**. Dort siehst du Quellenzahl, Arbeitsschritt und Prozentwert.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Eine erfolgreiche Quelle endet mit **Indiziert** und kann von Kampagnen durchsucht werden, die ihre Sammlung abonniert haben. Bei einem Fehlschlag erscheint **Fehler** und eine sichtbare Meldung zur Prüfung.

<h2 id="beispiel">Beispiel</h2>

Importiere `Das Vermächtnis des Uhrmachers.pdf` in **Stadträtsel**. Warte, bis aus **Wird indiziert…** der Status **Indiziert** wird, abonniere die Sammlung für **Der dreizehnte Glockenschlag** und frage:

> Welches Zahnrad öffnet Meister Pells verborgene Werkstatt?

Chronacle kann die indizierte Stelle finden und `[Source: "Das Vermächtnis des Uhrmachers.pdf", p.48]` zitieren.

<h2 id="zustaende-und-neuindizierung">Zustände und Neuindizierung</h2>

- **Indiziert** bedeutet, dass der aktuelle Import abgeschlossen wurde. Bei komplexen Seitenaufteilungen kann die Lesereihenfolge trotzdem abweichen.
- **Fehler** bedeutet, dass die Indizierung nicht erfolgreich abgeschlossen wurde. Lies vor einem neuen Versuch die angezeigte Meldung.
- Reine Bild-PDFs haben keine Textebene für die aktuelle Texterkennung; eine automatische Schriftbilderkennung gehört nicht zum Importablauf.
- Meldet Chronacle Quellen mit einem anderen Indizierungsmodell, wähle im Hinweis **Jetzt neu indizieren** oder unter Einstellungen **Alle Quellen neu indizieren**. Bestehende Quellen bleiben währenddessen durchsuchbar; ihre gespeicherten Textstellen werden nach und nach ersetzt.
