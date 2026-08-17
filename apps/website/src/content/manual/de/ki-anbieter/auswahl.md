---
translationKey: providers.choose
locale: de
slug: ki-anbieter/auswahl
title: Einen KI-Anbieter auswählen
summary: Wähle den Dienst für Antworten und getrennt davon den Modus für den Suchindex deiner Quellen.
section: ai-providers
order: 1
headings:
  - id: zwei-entscheidungen-treffen
    text: Zwei Entscheidungen treffen
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: entscheidungshilfe
    text: Entscheidungshilfe
    level: 2
---

<!-- German proofreading requested -->

<h2 id="zwei-entscheidungen-treffen">Zwei Entscheidungen treffen</h2>

Wähle einen Anbieter, der Antworten formuliert, und einen Indizierungsmodus, mit dem Chronacle passende Textstellen findet.

1. Öffne **Einstellungen** und suche den Abschnitt **LLM-Anbieter**. Ein LLM-Anbieter ist ein Dienst oder lokales Programm, das aus deiner Frage und passenden Quellenauszügen eine Antwort formuliert.
2. Wähle **OpenAI** oder **Anthropic** für die übliche [Online-Einrichtung](/de/handbuch/ki-anbieter/online), **Ollama (lokal)** für ein [lokales Antwortmodell](/de/handbuch/ki-anbieter/lokal) oder einen registrierten [benutzerdefinierten Anbieter](/de/handbuch/ki-anbieter/eigene-anbieter).
3. Trage unter **Modell** genau die Kennung ein, die dein Anbieter erwartet.
4. Wähle **Speichern und verbinden** und achte auf **Verbunden: …**
5. Lege unter **Einbettungsanbieter** fest, wie Chronacle den Suchindex aufbaut. Diese Einstellung ist vom Antwortanbieter unabhängig.
6. Wenn du das Indizierungsmodell nach dem Import von Quellen änderst, wähle **Alle Quellen neu indizieren**.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Unter **Verbindungsstatus** stehen der aktive Antwortanbieter und sein Modell. Der Abschnitt **Einbettungsanbieter** zeigt getrennt davon das aktive Indizierungsmodell und seine Dimension.

<h2 id="beispiel">Beispiel</h2>

Wähle für die Kampagne **Die Safranweiten** Anthropic als Antwortanbieter und **Mehrsprachig lokal — E5 Base (offline)** für die deutsche Quelle `Die Salzkrone.pdf`. Verbinde den Anbieter, importiere und indiziere das PDF und frage dann:

> Wem schuldet Kapitänin Vael noch einen Gefallen?

Wenn Chronacle die Stelle findet, kann die Antwort auf Deutsch erscheinen und `[Source: "Die Salzkrone.pdf", p.67]` zitieren.

<h2 id="entscheidungshilfe">Entscheidungshilfe</h2>

- Nimm einen Online-Anbieter, wenn du dessen unterstützte Modelle verwenden möchtest und die nötigen Zugangsdaten hast.
- Nimm Ollama, wenn du bewusst ein lokales Modell betreiben möchtest. Geschwindigkeit und Qualität hängen vom Modell und deinem Rechner ab.
- Für deutsche, französische oder spanische Quellen sowie sprachübergreifende Fragen eignet sich die mehrsprachige Indizierung.
- Bei einem Online-Antwortanbieter sendet Chronacle die Frage und die dafür gefundenen Quellenauszüge an diesen Anbieter, damit er die Antwort formuliert.
