---
translationKey: providers.custom
locale: de
slug: ki-anbieter/eigene-anbieter
title: Einen eigenen Anbieter hinzufügen
summary: Registriere einen OpenAI- oder Anthropic-kompatiblen Endpunkt und hinterlege die Modelle, die Chronacle anbieten soll.
section: ai-providers
order: 4
headings:
  - id: anbieter-registrieren
    text: Anbieter registrieren
    level: 2
  - id: erwartetes-ergebnis
    text: Erwartetes Ergebnis
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: kompatibilitaet-pruefen
    text: Kompatibilität prüfen
    level: 2
---

<!-- German proofreading requested -->

<h2 id="anbieter-registrieren">Anbieter registrieren</h2>

Lege die Adresse des Dienstes einmal an, füge die gewünschten Modellkennungen hinzu und wähle ihn anschließend als Antwortanbieter aus. Ein Endpunkt ist hier einfach die Webadresse, die Chronacle kontaktiert.

1. Öffne unter **Einstellungen** den Abschnitt **Benutzerdefinierte Anbieter** und wähle **Benutzerdefinierten Anbieter hinzufügen**.
2. Trage einen eindeutigen **Anbieternamen** ein.
3. Wähle unter **API-Kompatibilität** entweder **OpenAI-kompatibel** oder **Anthropic-kompatibel**.
4. Trage die **Basis-URL** des Anbieters und einen API-Schlüssel ein. Obwohl das Registrierungsformular den Schlüssel als **API-Schlüssel (optional)** bezeichnet, verlangt **Speichern und verbinden** derzeit bei jedem benutzerdefinierten Anbieter einen nicht leeren Schlüssel.
5. Wähle **Anbieter speichern**.
6. Wähle auf der neuen Anbieterkarte **Modell hinzufügen**. Trage die genaue **Modell-ID** des Dienstes und einen verständlichen Anzeigenamen ein und wähle **Hinzufügen**.
7. Kehre zu **LLM-Anbieter** zurück, wähle **Benutzerdefiniert: dein Name**, dann das Modell und schließlich **Speichern und verbinden**.

<h2 id="erwartetes-ergebnis">Erwartetes Ergebnis</h2>

Der eigene Anbieter erscheint in der Anbieterliste, sein Modell in der Modellauswahl und eine erfolgreiche Verbindung unter **Verbindungsstatus**.

<h2 id="beispiel">Beispiel</h2>

Registriere einen Dienst als **Glut-Gateway**, wähle die von diesem Dienst angegebene Kompatibilität, trage `https://ai.glut.example/v1` ein und ergänze die tatsächliche Modell-ID `glut-chat-klein` mit dem Anzeigenamen **Glut Chat Klein**. Frage nach dem Verbinden:

> Was verbirgt Magistrat Oren unter dem Kupferpodest?

Wenn `Der Gluthof.pdf` in einer abonnierten Sammlung liegt, kann die Antwort `[Source: "Der Gluthof.pdf", p.91]` zitieren.

<h2 id="kompatibilitaet-pruefen">Kompatibilität prüfen</h2>

- Kompatibilität bezeichnet das Anfrageformat des Dienstes. Sie bedeutet nicht, dass jedes Modell oder jede Funktion identisch arbeitet.
- Verwende die dokumentierte Basis-URL und Modell-ID des Anbieters. Chronacle sucht Modellkennungen nicht automatisch.
- Ein Dienst ohne Schlüsselpflicht lässt sich derzeit nicht über **Speichern und verbinden** anbinden, weil Chronacle einen leeren Schlüssel für benutzerdefinierte Anbieter ablehnt, bevor es den Dienst prüft.
- Ein eigener Antwortanbieter richtet keine Cloud-Indizierung ein. Falls du sie brauchst, konfiguriere sie getrennt unter **Einbettungsanbieter**.
