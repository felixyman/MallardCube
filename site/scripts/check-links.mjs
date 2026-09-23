// Verify every internal link and asset reference in the built site resolves
// to something that exists in `dist/`.
//
// The docs are served from a sub-path (`base` in astro.config.mjs), and
// Starlight does not rewrite markdown links for it: a relative link written as
// `./deployment/` from `/installation/` resolves in the browser to
// `/installation/deployment/` — a 404. Writing cross-page links as `../slug/`
// works, but nothing enforced it until this check (added after the deployed
// site shipped with broken links).
//
// The base is read from the root page's canonical URL, so moving the site to a
// custom domain needs no change here.

import { readFileSync, existsSync, readdirSync, statSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join, relative, sep } from 'node:path';

const distDir = join(dirname(fileURLToPath(import.meta.url)), '..', 'dist');

if (!existsSync(distDir)) {
  console.error('check-links: dist/ not found — build the site first');
  process.exit(1);
}

const rootHtml = readFileSync(join(distDir, 'index.html'), 'utf8');
const canonical = rootHtml.match(/<link rel="canonical" href="([^"]+)"/);
if (!canonical) {
  console.error('check-links: no canonical URL on the root page');
  process.exit(1);
}
const base = new URL(canonical[1]).pathname; // e.g. "/MallardCube/"

/** Every built HTML page, as [file path, its URL path]. */
function* pages(dir) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) {
      yield* pages(full);
    } else if (entry.endsWith('.html')) {
      const rel = relative(distDir, full).split(sep).join('/');
      // /installation/index.html -> /installation/
      const urlPath = rel.endsWith('index.html')
        ? rel.slice(0, -'index.html'.length)
        : rel;
      yield [full, urlPath];
    }
  }
}

const problems = [];
let checked = 0;

for (const [file, pageUrl] of pages(distDir)) {
  const html = readFileSync(file, 'utf8');
  for (const match of html.matchAll(/(?:href|src)="([^"]+)"/g)) {
    const href = match[1];
    if (/^(https?:|mailto:|javascript:|data:|#|\/\/)/.test(href)) continue;
    checked++;
    const resolved = new URL(href, `https://example.invalid${base}${pageUrl}`);
    const pathname = resolved.pathname;
    if (!pathname.startsWith(base)) {
      // An absolute link that forgets the base, e.g. "/deployment/".
      problems.push(`${pageUrl}  href="${href}"  -> ${pathname} (missing base "${base}")`);
      continue;
    }
    const rest = pathname.slice(base.length);
    const candidates = [
      join(distDir, rest),
      join(distDir, rest, 'index.html'),
      join(distDir, `${rest}.html`),
    ];
    if (!candidates.some((c) => existsSync(c))) {
      problems.push(`${pageUrl}  href="${href}"  -> ${pathname} (no such page or file)`);
    }
  }
}

if (problems.length > 0) {
  console.error(`check-links: ${problems.length} broken link(s) of ${checked} checked:`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
console.log(`check-links: ${checked} internal links OK`);
