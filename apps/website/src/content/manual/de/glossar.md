---
translationKey: glossary.main
locale: de
slug: glossar
title: Glossar
summary: Einfache Bedeutungen für Chronacles Begriffe rund um Quellen, Kampagnen, Kodex und Tresordateien.
section: glossary
order: 1
headings:
  - id: quellen-und-antworten
    text: Quellen und Antworten
    level: 2
  - id: kampagnen-und-kodex
    text: Kampagnen und Kodex
    level: 2
  - id: tresorbegriffe
    text: Tresorbegriffe
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
---

<!-- German proofreading requested -->

Nutze diese Bedeutungen, wenn dir auf einem Chronacle-Bildschirm oder in diesem Handbuch ein Begriff unbekannt ist.

<h2 id="quellen-und-antworten">Quellen und Antworten</h2>

**Quelle.** Eine importierte PDF mit ihrem Verarbeitungsstand. Siehe [Die Quellenbibliothek verwenden](/de/handbuch/quellenbibliothek/ueberblick).

**Sammlung.** Eine benannte Gruppe von Quellen, die mit mehreren Kampagnen verbunden werden kann. Siehe [Sammlungen ordnen](/de/handbuch/quellenbibliothek/sammlungen).

**Kampagne.** Dein spielbarer Arbeitsbereich mit eigenen Entitäten, Sitzungen, Chatverlauf und Zugriff auf ausgewählte Quellensammlungen. Siehe [Kampagnen und ihre Grenzen](/de/handbuch/kampagnen/ueberblick).

**Abschnitt oder Passage.** Ein kleines Textstück aus einer Quelle oder Notiz. So findet Chronacle die relevante Stelle, statt bei jeder Frage ein ganzes Buch zu verwenden.

**Einbettung.** Eine Zahlenbeschreibung von durchsuchbarem Text. Je nach Vorgang erstellt Chronacle sie für Quellabschnitte; Namen, Zusammenfassungen und Notizen von Entitäten sowie kompilierte Codex-Artikel; Sitzungstitel und -notizen; kompilierte Regeln; und Frage- oder Suchtext, damit es passendes Material vergleichen kann.

**Index.** Die bei der PDF-Verarbeitung vorbereiteten, durchsuchbaren Passagen. Eine Neuindizierung baut sie aus der gespeicherten Quelle neu auf. Siehe [Indizierung verstehen](/de/handbuch/quellenbibliothek/indizierung).

**Antwortanbieter.** Der eingerichtete KI-Dienst oder das lokale Programm, das aus deiner Frage und dem von Chronacle bereitgestellten Kontext die endgültige Antwort schreibt. Dieser Kontext kann relevante Quellenauszüge; Namen von Entitäten, Zusammenfassungen, Notizen und kompilierte Codex-Artikel; Spielernamen sowie Klasse, Stufe und Status; Start- und Enddaten von Ereignissen; Sitzungsnummern, -titel, Spieldaten und -notizen; sowie kompilierte Regeln umfassen. Siehe [Einen KI-Anbieter auswählen](/de/handbuch/ki-anbieter/auswahl).

**Quellenangabe.** Ein Verweis an einer Antwort, der Quelle und Seite hinter einer Aussage nennt. Siehe [Quellenangaben prüfen](/de/handbuch/notizen-und-sitzungen/quellenangaben).

<h2 id="kampagnen-und-kodex">Kampagnen und Kodex</h2>

**Kodexartikel.** Erzeugter Nachschlagetext zu einer Entität. Beim Kompilieren kann er ersetzt werden, deine Zusammenfassung und Notizen dagegen nicht. Siehe [Artikel und eigene Notizen trennen](/de/handbuch/kodex/artikel-und-notizen).

**Tischnotizen.** Deine dauerhaften Anmerkungen an einer kompilierten Regel. Sie bleiben erhalten, wenn ihr erzeugter Inhalt aktualisiert wird.

**Befund.** Ein Wartungseintrag zu einem möglichen Problem mit Namen, Verweisen, Typen oder kompilierten Inhalten, den du prüfen sollst. Er ist nicht automatisch ein bestätigter Fehler. Siehe [Den Kodex warten](/de/handbuch/kodex/wartung).

**Alias oder alternativer Name.** Ein weiterer Name für dieselbe Entität oder Regel, etwa „Die Laterne“ für Mara Venn. Siehe [Alternative Namen ergänzen](/de/handbuch/vault/alternative-namen).

**Sitzung.** Ein nummerierter Spielbericht mit Titel, Spieldatum, Notizen und verknüpften Ereignissen. Siehe [Ein Sitzungsprotokoll führen](/de/handbuch/notizen-und-sitzungen/sitzungsprotokoll).

<h2 id="tresorbegriffe">Tresorbegriffe</h2>

**Tresor.** Ein lokaler Ordner, in den Chronacle unterstützte Datensätze als Markdown-Dateien spiegelt und aus dem es unterstützte Änderungen übernehmen kann. Siehe [Einen Markdown-Tresor führen](/de/handbuch/vault/ueberblick).

**Konflikt.** Ein Datensatzstatus, der entsteht, wenn sich Chronacle und seine Markdown-Datei seit der letzten gemeinsamen Fassung unterschiedlich geändert haben. Beide Fassungen bleiben erhalten, während der Datensatz eingefroren ist. Siehe [Tresorkonflikte lösen](/de/handbuch/vault/konflikte).

<h2 id="beispiel">Beispiel</h2>

Du importierst die Quelle _Aufzeichnungen der Hafenmeisterin_ in die Sammlung **Valdris-Referenzen** und verbindest sie mit der Kampagne **Shadows of Valdris**. Chronacle teilt die Quelle in Passagen und indiziert sie. Der Antwortanbieter verwendet den von Chronacle bereitgestellten Kontext – einschließlich passender Passagen und verfügbarem Kampagnenkontext –, um „Warum meidet Mara Venn North Quay?“ zu beantworten, und liefert eine Quellenangabe. Du speicherst Mara als Entität, hältst Tischfakten in Notizen fest, kompilierst einen Kodexartikel, ergänzt „Die Laterne“ als Alias und protokollierst die Entdeckung in Sitzung 012. Ändern sich später Maras Chronacle-Notiz und Tresordatei getrennt voneinander, ist das ein Konflikt.
