// Generates the deterministic lore PDF fixture used by the enrichment E2E.
//
// No dependencies — emits a minimal, valid single-page PDF with a text stream
// so the real `pdf_extractor` produces chunks containing both the seed entity
// ("Commander Varn") and the related entity ("The Iron Fist"). Re-run with
// `node tests/e2e/ui/fixtures/make-pdf.mjs` if the lore text changes.
import { writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

// Each line becomes its own `Tj` show-text op. Keep lines free of unescaped
// parentheses/backslashes (none here) to avoid PDF string escaping.
const LINES = [
  'The Iron Fist is a militant guild that controls the eastern docks',
  'of the city of Varrowmoor. Commander Varn leads the Iron Fist with',
  'ruthless discipline, extorting every ship that makes berth there.',
  'Varn rose from the slums and now commands three hundred sworn blades.',
];

const text = LINES.map((l, i) => {
  const dy = i === 0 ? '' : '0 -18 Td\n';
  return `${dy}(${l}) Tj`;
}).join('\n');

const content = `BT
/F1 12 Tf
72 720 Td
${text}
ET`;

// Build the object table, tracking byte offsets for the xref.
const objects = [
  '<< /Type /Catalog /Pages 2 0 R >>',
  '<< /Type /Pages /Kids [3 0 R] /Count 1 >>',
  '<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>',
  `<< /Length ${content.length} >>\nstream\n${content}\nendstream`,
  '<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>',
];

let pdf = '%PDF-1.4\n';
const offsets = [];
objects.forEach((body, i) => {
  offsets.push(pdf.length);
  pdf += `${i + 1} 0 obj\n${body}\nendobj\n`;
});

const xrefStart = pdf.length;
pdf += `xref\n0 ${objects.length + 1}\n`;
pdf += '0000000000 65535 f \n';
for (const off of offsets) {
  pdf += `${String(off).padStart(10, '0')} 00000 n \n`;
}
pdf += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xrefStart}\n%%EOF`;

const out = fileURLToPath(new URL('./lore-iron-fist.pdf', import.meta.url));
writeFileSync(out, pdf, 'latin1');
console.log(`wrote ${out} (${pdf.length} bytes)`);
