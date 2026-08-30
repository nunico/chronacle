---
translationKey: sources.overview
locale: de
slug: quellenbibliothek/ueberblick
title: Überblick über die Quellenbibliothek
summary: Ordne PDFs in wiederverwendbaren Sammlungen und lege fest, welche Sammlungen jede Kampagne durchsucht.
section: source-library
order: 1
headings:
  - id: so-funktioniert-die-bibliothek
    text: So funktioniert die Bibliothek
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: speicherung-und-online-antworten
    text: Speicherung und Online-Antworten
    level: 2
---

<!-- German proofreading requested -->

<h2 id="so-funktioniert-die-bibliothek">So funktioniert die Bibliothek</h2>

Lege jedes PDF in einer Sammlung ab und abonniere für jede Kampagne genau die Sammlungen, die sie durchsuchen soll.

1. Wähle in der Seitenleiste **Kampagne und Quellen**.
2. Erstelle oder wähle unter **Kampagnen verwalten** eine Kampagne.
3. Mit dem Schalter neben einer **Quellsammlung** abonnierst du sie für die aktive Kampagne oder entfernst das Abonnement.
4. Klappe eine Sammlung auf, um unter **Bücher** den Indizierungsstatus jedes Buchs zu sehen.
5. Mit **Buch hinzufügen** importierst du ein weiteres PDF direkt in diese Sammlung.
6. Kehre mit der aktiven Kampagne zu **Orakel** zurück. Fragen durchsuchen ihre abonnierten Sammlungen.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Die Kampagnenansicht zeigt die Anzahl ihrer Sammlungen und Bücher. Antworten des Orakels können Bücher mit dem Status **Indiziert** aus abonnierten Sammlungen verwenden.

<h2 id="beispiel">Beispiel</h2>

Erstelle **Gemeinsame Seeregeln** mit `Fahrten auf dem Opalmeer.pdf` und **Schwarzbrand-Wissen** mit `Geheimnisse von Schwarzbrand.pdf`. Abonniere für **Die Glocke von Schwarzbrand** beide Sammlungen, für **Inseln der Morgenröte** aber nur **Gemeinsame Seeregeln**. Frage in **Die Glocke von Schwarzbrand**:

> Wer bewacht die ertrunkene Glocke unter dem Leuchtturm von Schwarzbrand?

Die Antwort kann `[Source: "Geheimnisse von Schwarzbrand.pdf", p.74]` verwenden. Die Kampagne **Inseln der Morgenröte** durchsucht diese Sammlung erst, wenn du sie dort ebenfalls abonnierst.

<h2 id="speicherung-und-online-antworten">Speicherung und Online-Antworten</h2>

- Chronacle speichert das importierte PDF und seinen Suchindex in den lokalen Daten der Desktop-App.
- Ist ein Online-Antwortanbieter aktiv, sendet Chronacle ihm die Frage und den bereitgestellten Antwortkontext. Das kann relevante Quellenauszüge; Namen von Entitäten, Zusammenfassungen, Notizen und kompilierte Codex-Artikel; Spielernamen sowie Klasse, Stufe und Status; Start- und Enddaten von Ereignissen; Sitzungsnummern, -titel, Spieldaten und -notizen; sowie kompilierte Regeln umfassen. Kampagnenentitäten und Sitzungen können vollständiger kampagnenbezogener Kontext statt relevanzgefilterter Ergebnisse sein.
- Eine Sammlung kann mehreren Kampagnen dienen. Ein gemeinsam verwendetes PDF musst du deshalb nur einmal importieren und indizieren.
- Lies weiter bei [Sammlungen](/de/handbuch/quellenbibliothek/sammlungen), [PDF-Import](/de/handbuch/quellenbibliothek/pdfs-importieren) oder [Indizierung](/de/handbuch/quellenbibliothek/indizierung).
