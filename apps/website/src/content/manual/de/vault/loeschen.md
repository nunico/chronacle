---
translationKey: vault.deleting
locale: de
slug: vault/loeschen
title: Datensätze und Tresordateien löschen
navTitle: Löschen
summary: Verstehe, welche Seite ein Löschen betrifft und was Chronacle absichtlich beibehält.
section: vault
order: 5
headings:
  - id: in-chronacle-loeschen
    text: In Chronacle löschen
    level: 2
  - id: im-ordner-loeschen
    text: Im Ordner löschen
    level: 2
  - id: beispiel
    text: Beispiel
    level: 2
  - id: arbeit-schuetzen
    text: Deine Arbeit schützen
    level: 2
---

<!-- German proofreading requested -->

Behandle Löschen als folgenschwer: Es gibt derzeit keine Wiederherstellungsansicht. Eine Löschung in der Oberfläche blendet den Datensatz überall in Chronacle aus. Eine vollständige Ordnerprüfung entfernt seine unveränderte verwaltete Datei, bewahrt aber eine Datei, die du seit der letzten gemeinsamen Fassung bearbeitet hast.

<h2 id="in-chronacle-loeschen">In Chronacle löschen</h2>

1. Sichere alle Notizen, die du noch brauchst.
2. Nutze die Entfernen-Aktion des Datensatzes und bestätige die Warnung.
3. Verschwindet die verwaltete Datei nicht zeitnah, wähle **Einstellungen → Markdown-Tresor → Jetzt synchronisieren**.

**Ergebnis:** Der Datensatz wird aus Listen, Suche, Verweisen und Kodex-Kompilierung ausgeblendet. Seine unveränderte verwaltete Datei und eine mögliche Konfliktbegleitdatei werden bereinigt. Weicht der Dateiinhalt von der letzten gemeinsamen Fassung ab, bleibt die Datei erhalten, damit dein späterer Text weder überschrieben noch gelöscht wird.

<h2 id="im-ordner-loeschen">Im Ordner löschen</h2>

Das Löschen einer zuvor exportierten verwalteten Datei signalisiert, dass der Chronacle-Datensatz ausgeblendet werden soll. Bevor Chronacle handelt, sucht es dieselbe `id` an anderen Stellen. So wird das vorübergehende Entfernen und Neuerstellen durch einen Editor nicht mit einer Löschung verwechselt und ein echtes Verschieben kann gefunden werden.

Bleibt keine Datei mit dieser `id` übrig, wird der Datensatz bei der nächsten vollständigen Prüfung weich gelöscht. „Weich gelöscht“ bedeutet: Chronacle behält den zugrunde liegenden Datensatz, schließt ihn aber überall aus, wo du ihn derzeit verwenden kannst. Eine Rückgängig-Aktion ist nicht umgesetzt.

<h2 id="beispiel">Beispiel</h2>

Du entfernst den aufgegebenen NSC Orren Pike in Chronacle. Bei der Abstimmung wird `campaigns/shadows-of-valdris/entities/npc/orren-pike.md` gelöscht, wenn die Datei noch Chronacles letztem Export entspricht. Hattest du dort später einen Epilog ergänzt, bleibt die Datei bestehen, obwohl Orren in Chronacle ausgeblendet ist.

<h2 id="arbeit-schuetzen">Deine Arbeit schützen</h2>

- Lege eine getrennte Sicherung an, bevor du viele Datensätze oder Dateien löschst.
- Lösche eine `.conflict.md`-Datei nicht zum Aufräumen; das ist ein Signal zur Konfliktlösung.
- Eine umbenannte Datei mit ihrer ursprünglichen `id` ist eine Verschiebung, keine Löschung.
- **Trennen** ist die sichere Wahl, wenn du nur den Ordner nicht mehr verwenden möchtest; die Dateien bleiben liegen.
