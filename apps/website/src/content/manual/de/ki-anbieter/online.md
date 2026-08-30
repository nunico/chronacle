---
translationKey: providers.online
locale: de
slug: ki-anbieter/online
title: Einen Online-Anbieter verbinden
summary: Verbinde OpenAI oder Anthropic mit deinem API-Schlüssel und der genauen Kennung des gewünschten Modells.
section: ai-providers
order: 2
headings:
  - id: anbieter-verbinden
    text: Anbieter verbinden
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: wichtige-details
    text: Wichtige Details
    level: 2
---

<!-- German proofreading requested -->

<h2 id="anbieter-verbinden">Anbieter verbinden</h2>

Trage Zugangsdaten und Modell unter **Einstellungen** ein und lass Chronacle die Verbindung prüfen.

1. Besorge dir nach der Anleitung des jeweiligen Anbieters einen API-Schlüssel von OpenAI oder Anthropic.
2. Öffne in Chronacle **Einstellungen** und suche den Abschnitt **LLM-Anbieter**.
3. Wähle unter **Anbieter** entweder **OpenAI** oder **Anthropic**.
4. Füge den Schlüssel unter **API-Schlüssel** ein.
5. Trage unter **Modell** eine genaue Modellkennung ein, die für deinen Zugang freigeschaltet ist.
6. Wähle **Speichern und verbinden**.
7. Prüfe, ob **Verbunden: openai** oder **Verbunden: anthropic** erscheint, und kontrolliere die Angaben unter **Verbindungsstatus**.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Chronacle bestätigt die Verbindung und verwendet den gewählten Anbieter für die nächsten Antworten des Orakels.

<h2 id="beispiel">Beispiel</h2>

Verbinde Anthropic, wähle ein für dich verfügbares Modell, abonniere für **Das Messingobservatorium** die Sammlung **Himmelskarten** und frage:

> Warum verhüllt die Astronomin Nera um Mitternacht die östliche Linse?

Chronacle sendet die Frage und den bereitgestellten Antwortkontext an den Anbieter und kann danach mit `[Source: "Notizen des Observatoriums.pdf", p.23]` antworten. Dieser Kontext kann relevante Quellenauszüge; Namen von Entitäten, Zusammenfassungen, Notizen und kompilierte Codex-Artikel; Spielernamen sowie Klasse, Stufe und Status; Start- und Enddaten von Ereignissen; Sitzungsnummern, -titel, Spieldaten und -notizen; sowie kompilierte Regeln umfassen. Kampagnenentitäten und Sitzungen können als vollständiger kampagnenbezogener Kontext statt als relevanzgefilterte Ergebnisse bereitgestellt werden.

<h2 id="wichtige-details">Wichtige Details</h2>

- Verfügbarkeit, Modellzugang und Kosten legt der jeweilige Anbieter fest. Prüfe seine aktuelle Dokumentation.
- **Einstellungen speichern** speichert das Formular. **Speichern und verbinden** übernimmt die Angaben und prüft die Verbindung sofort.
- Der Abschnitt **Einbettungsanbieter** regelt den Suchindex getrennt. Dein Online-Antwortanbieter wird nicht automatisch zum Indizierungsanbieter.
- Eine Fehlermeldung gibt die bei Chronacle angekommene Störung wieder. Prüfe Schlüssel, genaue Modellkennung und eine eventuell angezeigte Basis-URL, bevor du es erneut versuchst.
