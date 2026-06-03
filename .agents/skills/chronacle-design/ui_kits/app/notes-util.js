/* Helpers for the notebook: derive a plausible on-disk .md filename for each entry. */
export function slugify(s) {
  return s.toLowerCase().replace(/['']/g, "").replace(/[^a-z0-9]+/g, "-").replace(/(^-|-$)/g, "");
}

export function noteFile(item, cat) {
  const slug = slugify(item.title);
  if (cat.id === "sessions") {
    const num = (item.lead.match(/Session\s+(\d+)/) || [])[1];
    const nn = num ? String(num).padStart(3, "0") + "-" : "";
    return { name: nn + slug + ".md", path: cat.folder + "/" + nn + slug + ".md" };
  }
  return { name: slug + ".md", path: cat.folder + "/" + slug + ".md" };
}
