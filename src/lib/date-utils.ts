let dateTimeFormatter: Intl.DateTimeFormat

function getDateTimeFormatter(): Intl.DateTimeFormat {
  if (!dateTimeFormatter) {
    dateTimeFormatter = new Intl.DateTimeFormat();
  }
  return dateTimeFormatter;
}

export function formatDate(dateStr: string): string {
  if (!dateStr) return '';
  // Append T12:00:00 to treat as local noon, avoiding UTC midnight off-by-one
  // when YYYY-MM-DD strings are parsed as UTC and rendered in western timezones
  const d = new Date(dateStr.includes('T') ? dateStr : dateStr + 'T12:00:00');
  if (isNaN(d.getTime())) return dateStr;

  return getDateTimeFormatter().format(d);
}
