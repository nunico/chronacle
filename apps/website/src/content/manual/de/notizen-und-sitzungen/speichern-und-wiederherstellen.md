---
translationKey: notes.saving-recovery
locale: de
slug: notizen-und-sitzungen/speichern-und-wiederherstellen
title: Entwürfe behalten und Speichervorgänge wiederholen
summary: Wechsle sicher zwischen Kampagneninhalten, erkenne den Speicherstatus, wiederhole Fehler und schließe Chronacle ohne unbemerkten Entwurfsverlust.
section: notes-and-sessions
order: 6
headings:
  - id: speicherstatus-lesen
    text: Speicherstatus lesen
    level: 2
  - id: entwurf-beim-wechsel-behalten
    text: Entwurf beim Wechsel behalten
    level: 2
  - id: entitaet-speichern-oder-verwerfen
    text: Entität speichern oder verwerfen
    level: 2
  - id: automatisches-speichern-wiederholen
    text: Automatisches Speichern wiederholen
    level: 2
  - id: chronacle-sicher-schliessen
    text: Chronacle sicher schließen
    level: 2
  - id: grenze-der-wiederherstellung
    text: Grenze der Wiederherstellung kennen
    level: 2
---

Chronacle ordnet jeden Entwurf der jeweiligen Kampagne und genau dem Eintrag zu, in dem du ihn begonnen hast. Die Navigation selbst sendet, speichert oder verwirft diese Arbeit nicht.

<h2 id="speicherstatus-lesen">Speicherstatus lesen</h2>

- **Ungespeicherte Änderungen** bedeutet, dass Chronacle eine neuere Fassung aufbewahrt, die noch nicht von der Datenbank bestätigt wurde.
- **Wird gespeichert…** bedeutet, dass gerade geschrieben wird. Du kannst weitertippen; der Abschluss eines älteren Textstands kann neueren Text nicht als gespeichert kennzeichnen.
- **Gespeichert** bedeutet, dass die passende Fassung bestätigt wurde. Eine ungesendete Orakelfrage ist damit nicht gemeint.
- **Speichern nicht möglich** bewahrt Text und Fehler sichtbar auf. Wähle **Erneut versuchen**, um die neueste aufbewahrte Fassung in denselben Eintrag zu schreiben.

Wenn du genau zum bekannten gespeicherten Wert zurückkehrst, verschwindet der Status **Ungespeicherte Änderungen** ohne einen weiteren Schreibvorgang. Schnelle Speicheranforderungen für denselben Eintrag werden nacheinander verarbeitet. Chronacle führt keine parallelen Schreibvorgänge aus, durch die ein älterer Wert einen neueren ersetzen könnte.

<h2 id="entwurf-beim-wechsel-behalten">Entwurf beim Wechsel behalten</h2>

- Eine ungesendete Orakelfrage bleibt erhalten, wenn du ein Notizbuch oder eine andere Ansicht besuchst und zurückkehrst. Jede Kampagne und der Bereich **Keine Kampagne** haben jeweils ein eigenes Eingabefeld. Beim Zurückkehren wird die Frage niemals gesendet.
- Entwürfe für bestehende und neue Entitäten bleiben erhalten, wenn du Ansicht, Eintrag im Notizbuch oder Kampagne wechselst. Öffnest du dieselbe Kampagne und denselben Eintrag erneut, stellt Chronacle Entwurf und Status wieder her.
- Änderungen an Sitzungsfeldern und Tischnotizen bleiben mit ihrer genauen Kampagne und ihrem Eintrag verbunden, während du an eine andere Stelle wechselst – auch während eines ausstehenden Speichervorgangs.

Entwürfe erscheinen niemals in einer anderen Kampagne oder einem anderen Eintrag.

<h2 id="entitaet-speichern-oder-verwerfen">Entität speichern oder verwerfen</h2>

Entitätsformulare bleiben ausdrücklich gesteuert: Wähle **Erstellen** für eine neue Entität oder **Speichern** für eine bestehende. Das bloße Beginnen einer neuen Entität oder Wegnavigieren erstellt keinen Eintrag. Um einen Entitätsentwurf aufzugeben, wähle **Abbrechen** und bestätige danach **Änderungen verwerfen**; andere Entwürfe bleiben unberührt. Läuft bereits ein Schreibvorgang für die Entität, ist das Verwerfen nicht verfügbar, bis er abgeschlossen ist, denn dieser Schreibvorgang lässt sich nicht zurücknehmen.

War **Erstellen** erfolgreich, aber Chronacle findet eine neuere Fassung des zurückgegebenen Eintrags, erscheint **Erstellt, aber Eingriff erforderlich**. Wähle **Gespeicherten Eintrag behalten** oder **Meinen Entwurf behalten**. Keine der beiden Möglichkeiten startet einen weiteren Speichervorgang im Hintergrund. Behältst du einen noch ungespeicherten Entwurf, wähle anschließend **Speichern**.

Wurde das Ziel gelöscht oder ist es anderweitig nicht verfügbar, bleibt der fehlgeschlagene Entwurf als nicht verfügbar sichtbar. **Erneut versuchen** zielt weiterhin auf diesen Eintrag und leitet deinen Text niemals an eine andere Stelle um. Nutze **Änderungen verwerfen**, wenn du den Entwurf nicht mehr brauchst.

<h2 id="automatisches-speichern-wiederholen">Automatisches Speichern wiederholen</h2>

Sitzungstitel, Spieldatum und Notizen sowie **Tischnotizen** kompilierter Regeln fordern das Speichern an, wenn der Fokus zu einem anderen Bedienelement derselben Sitzung oder Regel wechselt. Wechselst du direkt zu einem anderen Eintrag oder einer anderen Ansicht, bleibt der Entwurf als **Ungespeicherte Änderungen** erhalten, ohne dass ein Speichervorgang beginnt. Scheitert eine Speicheranforderung:

1. Behalte den angezeigten Text; er wurde nicht durch die gespeicherte Fassung ersetzt.
2. Lies den Fehler direkt beim jeweiligen Editor.
3. Behebe das Inhalts- oder Verbindungsproblem, falls die Meldung eines nennt.
4. Wähle **Erneut versuchen**. Eine erfolgreiche Bestätigung entfernt den Fehler und zeigt **Gespeichert**.

Die Wiederholung behält ursprüngliche Kampagne und Ziel bei. Mehrere schnelle Änderungen werden ohne parallele Schreibvorgänge auf die neueste angeforderte Fassung zusammengeführt.

<h2 id="chronacle-sicher-schliessen">Chronacle sicher schließen</h2>

Das normale Schließen des Chronacle-Fensters wird blockiert, solange die App eine ungesendete Frage, eine ungespeicherte oder fehlgeschlagene Änderung aufbewahrt oder einen Speichervorgang ausführt. Im Dialog **Ungespeicherte Änderungen** ist zunächst die sichere Aktion **Abbrechen** ausgewählt:

- Wähle **Abbrechen** oder drücke Escape, um weiterzuarbeiten. Der Fokus kehrt an die vorige Stelle zurück.
- Läuft kein Schreibvorgang, wähle **Verwerfen und schließen** nur, wenn du alle aufbewahrten Entwürfe entfernen und die App schließen möchtest. Beim Schließen startet Chronacle keinen Speicherversuch in letzter Sekunde.
- Solange bereits geschrieben wird, ist **Verwerfen und schließen** nicht verfügbar. Sind danach alle Änderungen gespeichert, meldet der Dialog, dass du sicher schließen kannst, und bietet **Schließen** an. Bleibt ein Risiko bestehen, kannst du abbrechen oder ausdrücklich **Verwerfen und schließen** wählen.

<h2 id="grenze-der-wiederherstellung">Grenze der Wiederherstellung kennen</h2>

Chronacle bewahrt diese Entwürfe nur im Arbeitsspeicher des laufenden Programms auf. Nach einem Absturz oder Stromausfall sowie nach einem erzwungenen Beenden oder Neustart lassen sie sich nicht wiederherstellen. Private Kampagnenentwürfe werden weder in den Browserspeicher noch in einen anderen dauerhaften Entwurfsspeicher kopiert. Schließe wichtige Speichervorgänge ab und kläre den Dialog zum Schließen, bevor du die App beendest.
