---
translationKey: providers.local
locale: de
slug: ki-anbieter/lokal
title: Ein lokales Antwortmodell verwenden
summary: Verbinde Chronacle mit einem Ollama-Modell auf deinem Rechner und richte den Suchindex getrennt davon ein.
section: ai-providers
order: 3
headings:
  - id: ollama-verbinden
    text: Ollama verbinden
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: hinweise-zu-lokalen-modellen
    text: Hinweise zu lokalen Modellen
    level: 2
---

<!-- German proofreading requested -->

<h2 id="ollama-verbinden">Ollama verbinden</h2>

Starte ein Modell in Ollama und verbinde Chronacle über die lokale Adresse und den genauen Modellnamen damit.

1. Installiere Ollama nach der aktuellen Anleitung des Projekts und lade dort ein unterstütztes Chatmodell herunter.
2. Achte darauf, dass Ollama läuft, und notiere die genaue Kennung des Modells.
3. Öffne in Chronacle **Einstellungen** und wähle unter **Anbieter** die Option **Ollama (lokal)**.
4. Trage die Kennung unter **Modell** ein.
5. Lass die **Basis-URL** auf `http://localhost:11434`, sofern dein Ollama-Dienst keine andere Adresse verwendet.
6. Wähle **Speichern und verbinden** und achte auf **Verbunden: ollama**.
7. Richte den **Einbettungsanbieter** getrennt ein. Ollama liefert Antworten, legt aber nicht den Indizierungsmodus von Chronacle fest.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Unter **Verbindungsstatus** stehen Ollama und das ausgewählte Modell. Neue Fragen im Orakel verwenden den laufenden lokalen Dienst.

<h2 id="beispiel">Beispiel</h2>

Verbinde ein bereits in Ollama vorhandenes Modell und frage in der Kampagne **Das Winterbuch**:

> Welches Versprechen gab Wächterin Eska am gefrorenen Tor?

Chronacle findet die passende Textstelle und lässt Ollama die Antwort formulieren. Sie kann `[Source: "Das Winterbuch.pdf", p.54]` enthalten.

<h2 id="hinweise-zu-lokalen-modellen">Hinweise zu lokalen Modellen</h2>

- Downloadgröße, Speicherbedarf, Geschwindigkeit und Antwortqualität unterscheiden sich je nach Modell. Vergleiche die Anforderungen mit deinem Rechner.
- Chronacle lädt oder startet das Ollama-Chatmodell nicht für dich. Ollama muss beim Verbinden bereit sein.
- `http://localhost:11434` ist die in Chronacle angezeigte Standardadresse, aber deine Ollama-Einrichtung kann abweichen.
- Die lokalen Nomic- und E5-Optionen unter **Einbettungsanbieter** bauen den Suchindex auf. Sie sind vom Ollama-Antwortmodell getrennt.
