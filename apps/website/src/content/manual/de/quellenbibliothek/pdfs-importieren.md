---
translationKey: sources.upload
locale: de
slug: quellenbibliothek/pdfs-importieren
title: PDFs importieren
summary: Importiere ein PDF in eine Sammlung und verfolge den Fortschritt, bis es durchsuchbar ist.
section: source-library
order: 3
headings:
  - id: pdf-importieren
    text: PDF importieren
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: geeignete-inhalte-und-fehler
    text: Geeignete Inhalte und Fehler
    level: 2
---

<!-- German proofreading requested -->

<h2 id="pdf-importieren">PDF importieren</h2>

Wähle ein einzelnes PDF aus, lege es in einer Sammlung ab und warte, bis die Fortschrittsanzeige die Bereitschaft meldet.

1. Wähle **PDF hochladen** in der Seitenleiste, **Regelbuch anhängen** neben dem Fragefeld des Orakels oder **Buch hinzufügen** in einer aufgeklappten Sammlung.
2. Wähle im Dateifenster eine einzelne `.pdf`-Datei aus.
3. Wenn du über Seitenleiste oder Orakel begonnen hast, wähle unter **„Dateiname“ zur Sammlung hinzufügen** eine Sammlung aus. Dort kannst du auch **Neue Sammlung erstellen** wählen.
4. Wähle **Hochladen**. Nach **Buch hinzufügen** verwendet Chronacle direkt die geöffnete Sammlung.
5. Beobachte unter der Hauptansicht Dateiname, Statustext und **Upload-Fortschritt**.
6. Öffne **Kampagne und Quellen**, klappe die Sammlung auf und prüfe, ob beim Buch **Indiziert** steht.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Die Fortschrittsanzeige erreicht **Bereit!** und verschwindet nach einer kurzen Pause. Die Quelle bleibt in ihrer Sammlung mit dem Status **Indiziert** erhalten.

<h2 id="beispiel">Beispiel</h2>

Wähle **PDF hochladen**, dann `Die violette Fähre.pdf`, erstelle die Sammlung **Flussrätsel** und wähle **Hochladen**. Sobald beim Buch **Indiziert** steht, abonnierst du die Sammlung für **Die letzte Überfahrt** und fragst:

> Was muss ein Fahrgast auf der violetten Fähre zurücklassen?

Chronacle kann mit `[Source: "Die violette Fähre.pdf", p.12]` antworten.

<h2 id="geeignete-inhalte-und-fehler">Geeignete Inhalte und Fehler</h2>

- Chronacle liest die Textebene eines PDFs. Reine Bildscans enthalten keinen lesbaren Text; der Import kann mit **Fehler** enden oder ohne brauchbare Suchstellen abgeschlossen werden.
- Spalten, Tabellen und verzierte Seiten können die ausgelesene Reihenfolge beeinflussen. Prüfe bei genauen Formulierungen die zitierte Textstelle.
- Es läuft immer nur ein Upload. Startest du währenddessen einen weiteren, erscheint **Ein Upload läuft bereits — bitte warten.**
- Ein Fehler bleibt sichtbar, bis du ihn schließt. Nutze die angezeigte Meldung für deinen nächsten Versuch und unterstelle nicht automatisch, dass die Datei die Ursache ist.
