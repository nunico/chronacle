import type { SiteFooterLabels } from '$lib/components/SiteFooter.svelte';
import type { SiteHeaderLabels } from '$lib/components/SiteHeader.svelte';
import type { Locale } from './types';

export interface FeatureCopy {
  icon: 'book-open' | 'notebook' | 'quote';
  title: string;
  body: string;
}

export interface WorkflowStepCopy {
  number: string;
  title: string;
  body: string;
}

export interface LandingCopy {
  metadata: {
    title: string;
    description: string;
  };
  header: SiteHeaderLabels;
  hero: {
    eyebrow: string;
    heading: string;
    headingLabel: string;
    body: string;
    download: string;
    manual: string;
    trust: string;
  };
  productExample: {
    label: string;
    windowLabel: string;
    questionLabel: string;
    question: string;
    assistant: string;
    answerLabel: string;
    verdict: string;
    answer: string;
    citationLabel: string;
    citation: string;
    excerpt: string;
    metadata: string;
  };
  features: {
    eyebrow: string;
    heading: string;
    body: string;
    items: [FeatureCopy, FeatureCopy, FeatureCopy];
  };
  workflow: {
    eyebrow: string;
    heading: string;
    items: [WorkflowStepCopy, WorkflowStepCopy, WorkflowStepCopy];
  };
  provider: {
    eyebrow: string;
    heading: string;
    body: string;
    localOption: string;
  };
  download: {
    eyebrow: string;
    heading: string;
    body: string;
    primary: string;
    secondary: string;
    note: string;
  };
  footer: SiteFooterLabels;
}

export const landingCopy = {
  en: {
    metadata: {
      title: 'Chronacle — cited answers from your books',
      description:
        'Load source PDFs, keep campaign notes, and ask questions with citations you can inspect.',
    },
    header: {
      home: 'Chronacle home',
      manual: 'Manual',
      source: 'Source',
      download: 'Download Chronacle',
      language: 'Language',
      english: 'English',
      german: 'Deutsch',
      navigation: 'Primary navigation',
    },
    hero: {
      eyebrow: 'Local-first desktop reference',
      heading: 'Ask your books. Check the answer.',
      headingLabel: 'Chronacle — Ask your books. Check the answer.',
      body: 'Load source PDFs, keep notes for your campaigns and settings, then ask a question. Chronacle answers with citations you can open and inspect.',
      download: 'Download Chronacle',
      manual: 'Read the manual',
      trust: 'Open source · macOS, Windows, and Linux',
    },
    productExample: {
      label: 'An illustrative answer',
      windowLabel: 'Illustrative Chronacle product window',
      questionLabel: 'Question',
      question: 'How long does the signal beacon stay active after a failed check?',
      assistant: 'Chronacle',
      answerLabel: 'Answer',
      verdict: 'The beacon stays active for ten minutes.',
      answer:
        'A failed check prevents a stable lock, but the emergency signal continues until its reserve cycle ends.',
      citationLabel: 'Source',
      citation: 'Field Guide · Signal beacons · p. 48',
      excerpt: '“An emergency signal remains active for ten minutes after an unsuccessful lock.”',
      metadata: 'Illustrative example · 1 source excerpt',
    },
    features: {
      eyebrow: 'What it keeps together',
      heading: 'A practical reference for the table',
      body: 'Your source material, working notes, and cited answers stay in one focused desktop app.',
      items: [
        {
          icon: 'book-open',
          title: 'Searchable source PDFs',
          body: 'Load the books and documents you use. Chronacle builds a local search index for quick retrieval.',
        },
        {
          icon: 'notebook',
          title: 'Campaign and setting notes',
          body: 'Keep the people, places, decisions, and loose threads you need beside your source material.',
        },
        {
          icon: 'quote',
          title: 'Answers with a source',
          body: 'Each answer points back to the relevant passage, so you can check the wording yourself.',
        },
      ],
    },
    workflow: {
      eyebrow: 'How it works',
      heading: 'From document to checked answer',
      items: [
        {
          number: '01',
          title: 'Load your sources',
          body: 'Add the PDFs you already use and let Chronacle prepare them for search.',
        },
        {
          number: '02',
          title: 'Ask as you would at the table',
          body: 'Write a plain question about a rule, a place, a person, or your notes.',
        },
        {
          number: '03',
          title: 'Inspect the citation',
          body: 'Read the concise answer, then open the cited excerpt when the exact wording matters.',
        },
      ],
    },
    provider: {
      eyebrow: 'Storage and AI providers',
      heading: 'Your library lives here.',
      body: 'Your source files, search index, and notes stay local on this computer. A compatible online AI provider normally receives your question and the relevant source excerpts needed to answer it.',
      localOption: 'Supported local models are available as a secondary option.',
    },
    download: {
      eyebrow: 'Desktop app',
      heading: 'Download Chronacle',
      body: 'Get the current release from GitHub, or read the manual before you begin.',
      primary: 'Download Chronacle',
      secondary: 'Read the manual',
      note: 'Available for macOS, Windows, and Linux.',
    },
    footer: {
      home: 'Chronacle home',
      tagline: 'Check sources instead of guessing answers.',
      navigation: 'Footer navigation',
      manual: 'Manual',
      source: 'Source',
      license: 'License',
      copyright: 'Chronacle · AGPL-3.0 with Branding Exception',
    },
  },
  de: {
    metadata: {
      title: 'Chronacle — belegte Antworten aus deinen Büchern',
      description:
        'Lade Quellen als PDF, führe Kampagnennotizen und stell Fragen mit prüfbaren Fundstellen.',
    },
    header: {
      home: 'Chronacle Startseite',
      manual: 'Handbuch',
      source: 'Quellcode',
      download: 'Chronacle herunterladen',
      language: 'Sprache',
      english: 'English',
      german: 'Deutsch',
      navigation: 'Hauptnavigation',
    },
    hero: {
      eyebrow: 'Lokales Nachschlagewerk für den Desktop',
      heading: 'Frag deine Bücher. Prüf die Antwort.',
      headingLabel: 'Chronacle — Frag deine Bücher. Prüf die Antwort.',
      body: 'Lade deine Quellen als PDF, führe Notizen zu Kampagnen und Settings und stell dann deine Frage. Chronacle antwortet mit Fundstellen, die du öffnen und prüfen kannst.',
      download: 'Chronacle herunterladen',
      manual: 'Handbuch lesen',
      trust: 'Open Source · macOS, Windows und Linux',
    },
    productExample: {
      label: 'Eine beispielhafte Antwort',
      windowLabel: 'Beispielhafte Chronacle Produktansicht',
      questionLabel: 'Frage',
      question: 'Wie lange bleibt das Signal nach einer misslungenen Prüfung aktiv?',
      assistant: 'Chronacle',
      answerLabel: 'Antwort',
      verdict: 'Das Signal bleibt zehn Minuten aktiv.',
      answer:
        'Die misslungene Prüfung verhindert eine stabile Verbindung. Das Notsignal läuft aber bis zum Ende des Reservezyklus weiter.',
      citationLabel: 'Quelle',
      citation: 'Feldhandbuch · Signalgeber · S. 48',
      excerpt: '„Ein Notsignal bleibt nach einer erfolglosen Verbindung zehn Minuten aktiv.“',
      metadata: 'Beispiel · 1 Quellenausschnitt',
    },
    features: {
      eyebrow: 'Alles an einem Ort',
      heading: 'Ein praktisches Nachschlagewerk für den Tisch',
      body: 'Deine Quellen, Arbeitsnotizen und belegten Antworten bleiben in einer konzentrierten Desktop-App zusammen.',
      items: [
        {
          icon: 'book-open',
          title: 'Durchsuchbare Quellen-PDFs',
          body: 'Lade die Bücher und Dokumente, die du nutzt. Chronacle erstellt daraus einen lokalen Suchindex.',
        },
        {
          icon: 'notebook',
          title: 'Notizen zu Kampagne und Setting',
          body: 'Halte Personen, Orte, Entscheidungen und offene Fäden direkt neben deinen Quellen fest.',
        },
        {
          icon: 'quote',
          title: 'Antworten mit Fundstelle',
          body: 'Jede Antwort verweist auf die passende Passage, damit du den Wortlaut selbst prüfen kannst.',
        },
      ],
    },
    workflow: {
      eyebrow: 'So funktioniert es',
      heading: 'Vom Dokument zur geprüften Antwort',
      items: [
        {
          number: '01',
          title: 'Lade deine Quellen',
          body: 'Füge deine vorhandenen PDFs hinzu und lass Chronacle sie für die Suche vorbereiten.',
        },
        {
          number: '02',
          title: 'Frag, wie du am Tisch fragst',
          body: 'Schreib eine normale Frage zu einer Regel, einem Ort, einer Person oder deinen Notizen.',
        },
        {
          number: '03',
          title: 'Prüf die Fundstelle',
          body: 'Lies die kurze Antwort und öffne den Ausschnitt, wenn der genaue Wortlaut zählt.',
        },
      ],
    },
    provider: {
      eyebrow: 'Speicherort und KI-Anbieter',
      heading: 'Deine Bibliothek bleibt auf diesem Rechner.',
      body: 'Deine Quelldateien, der Suchindex und deine Notizen bleiben lokal auf diesem Rechner. Ein kompatibler Online-KI-Anbieter erhält normalerweise deine Frage und die relevanten Quellenausschnitte, die für die Antwort nötig sind.',
      localOption: 'Unterstützte lokale Modelle gibt es als zweite Option.',
    },
    download: {
      eyebrow: 'Desktop-App',
      heading: 'Chronacle herunterladen',
      body: 'Hol dir die aktuelle Version auf GitHub oder lies zuerst das Handbuch.',
      primary: 'Chronacle herunterladen',
      secondary: 'Handbuch lesen',
      note: 'Für macOS, Windows und Linux verfügbar.',
    },
    footer: {
      home: 'Chronacle Startseite',
      tagline: 'Quellen prüfen statt Antworten erraten.',
      navigation: 'Navigation im Seitenfuß',
      manual: 'Handbuch',
      source: 'Quellcode',
      license: 'Lizenz',
      copyright: 'Chronacle · AGPL-3.0 mit Branding-Ausnahme',
    },
  },
} satisfies Record<Locale, LandingCopy>;
