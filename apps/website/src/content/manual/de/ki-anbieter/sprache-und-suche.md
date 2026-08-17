---
translationKey: providers.language-search
locale: de
slug: ki-anbieter/sprache-und-suche
title: Sprache und Suche
summary: Passe Oberfläche und Antwortsprache an deine Runde an und wähle einen Index für die Sprachen deiner Quellen.
section: ai-providers
order: 5
headings:
  - id: sprache-und-indizierung-einstellen
    text: Sprache und Indizierung einstellen
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: was-die-einstellungen-aendern
    text: Was die Einstellungen ändern
    level: 2
---

<!-- German proofreading requested -->

<h2 id="sprache-und-indizierung-einstellen">Sprache und Indizierung einstellen</h2>

Lege zuerst die Anzeigesprache fest und wähle dann ein Indizierungsmodell, das zu den Sprachen deiner PDFs und Fragen passt.

1. Stelle unter **Einstellungen** die **Anzeigesprache** auf **Automatisch**, **English**, **Deutsch**, **Français** oder **Español**. Die Oberfläche wechselt sofort.
2. Wähle unter **Einbettungsmodus** entweder **Klein lokal — Nomic (offline)** für eine englisch ausgerichtete Suche oder **Mehrsprachig lokal — E5 Base (offline)** für Deutsch, Französisch, Spanisch und sprachübergreifende Suche.
3. Bei **Cloud — OpenAI-kompatible API** trägst du Zugangsdaten und ein Modell ein, das – wie in den Einstellungen verlangt – 768 Dimensionen liefert.
4. Wähle **Einbettungsanbieter speichern**. Falls Chronacle nach dem gewählten lokalen Modell fragt, wähle **Ausgewähltes Modell herunterladen**.
5. Wähle **Alle Quellen neu indizieren**, damit bestehende PDFs das neue Modell verwenden.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Die Oberfläche verwendet deine Auswahl. Bei neuen Fragen antwortet Chronacle in einer klar erkannten unterstützten Fragesprache; bei sehr kurzen oder mehrdeutigen Fragen verwendet es die Sprache der Oberfläche.

<h2 id="beispiel">Beispiel</h2>

Stelle die Oberfläche auf **Deutsch**, wähle **Mehrsprachig lokal — E5 Base (offline)**, indiziere das englische `Atlas of Quiet Stars.pdf` neu und frage:

> Warum meidet Serin den Nordturm?

Chronacle kann die englische Stelle finden, auf Deutsch antworten und `[Source: "Atlas of Quiet Stars.pdf", p.36]` zitieren. Quellenname und zitierter Text bleiben so erhalten, wie sie im PDF stehen.

<h2 id="was-die-einstellungen-aendern">Was die Einstellungen ändern</h2>

- **Anzeigesprache** ändert die Bedienelemente und dient bei kurzen oder mehrdeutigen Fragen als Ersatz für die Antwortsprache.
- **Einbettungsmodus** legt fest, wie Quellentext und Fragen für die Suche indiziert werden. Nach einem Wechsel musst du bestehende Quellen neu indizieren.
- **LLM-Anbieter** bestimmt, wer die Antwort formuliert. Bei einem Online-Anbieter werden Frage und passende gefundene Auszüge an diesen Anbieter gesendet.
- Ein mehrsprachiger Index verbessert die Suche über unterstützte Sprachen hinweg. Er übersetzt weder das gespeicherte PDF noch seine Namen.
