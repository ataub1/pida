#!/usr/bin/env node
// PDF-Over NG – Lizenzprüfung und Drittlizenz-Hinweise der npm-Abhängigkeiten (Plan §2.5, §10.2).
//
//   node tools/npm-licenses.mjs apps/app/package-lock.json                 # prüfen (CI)
//   node tools/npm-licenses.mjs apps/app/package-lock.json --notices out.md   # zusätzlich Hinweise schreiben
//
// Quelle ist das Lockfile (lockfileVersion 2/3, Feld `license` je Paket); Lizenztexte für die
// Hinweise kommen aus node_modules/<paket>/LICENSE* bzw. NOTICE*, wenn installiert (`npm ci`).
//
// Regeln:
// - Laufzeitabhängigkeiten (ohne `dev: true`, landen im App-Bundle): dieselbe Allowlist wie
//   deny.toml. Alles andere – auch fehlende Lizenzangaben – bricht ab.
// - Reine Build-Werkzeuge (`dev: true`, nicht ausgeliefert): zusätzlich übliche permissive
//   Lizenzen für Daten und Werkzeuge (CC0, CC-BY, 0BSD, BlueOak, Python-2.0 …). Copyleft (GPL,
//   AGPL, LGPL, SSPL, EUPL fremder Pakete …) bricht auch hier ab.
// Ohne Abhängigkeiten außer Node ≥ 18.

import fs from "node:fs";
import path from "node:path";

const ALLOW_RUNTIME = new Set([
  "MIT", "Apache-2.0", "BSD-2-Clause", "BSD-3-Clause", "ISC", "Zlib", "MPL-2.0",
  "Unicode-3.0", "BSL-1.0", "OFL-1.1", "Bitstream-Vera",
]);
const ALLOW_BUILD_ONLY = new Set([
  ...ALLOW_RUNTIME,
  "0BSD", "MIT-0", "CC0-1.0", "CC-BY-3.0", "CC-BY-4.0", "BlueOak-1.0.0", "Python-2.0",
  "Unlicense", "Unicode-DFS-2016", "WTFPL", "Artistic-2.0",
]);

function usage(code) {
  console.error("Aufruf: node tools/npm-licenses.mjs <package-lock.json> [--notices <datei.md>]");
  process.exit(code);
}

// --- SPDX-Ausdrücke (OR, AND, WITH, Klammern) ---------------------------------------------------
function tokenize(expr) {
  return expr.replace(/[()]/g, (m) => ` ${m} `).trim().split(/\s+/).filter(Boolean);
}

function parseExpr(tokens) {
  let pos = 0;
  const peek = () => tokens[pos];
  const next = () => tokens[pos++];
  function primary() {
    const t = next();
    if (t === undefined) throw new Error("unerwartetes Ende");
    if (t === "(") {
      const e = orExpr();
      if (next() !== ")") throw new Error("')' fehlt");
      return e;
    }
    let id = t.replace(/\+$/, ""); // "GPL-2.0+" → Basis-ID (bleibt verboten)
    if (peek() && peek().toUpperCase() === "WITH") {
      next();
      next(); // Ausnahme (z. B. LLVM-exception) erweitert nur Rechte
    }
    return { id };
  }
  function andExpr() {
    let left = primary();
    while (peek() && peek().toUpperCase() === "AND") {
      next();
      left = { and: [left, primary()] };
    }
    return left;
  }
  function orExpr() {
    let left = andExpr();
    while (peek() && peek().toUpperCase() === "OR") {
      next();
      left = { or: [left, andExpr()] };
    }
    return left;
  }
  const e = orExpr();
  if (pos !== tokens.length) throw new Error(`überzählige Zeichen ab '${tokens[pos]}'`);
  return e;
}

function allowed(node, allow) {
  if (node.id) return allow.has(node.id);
  if (node.and) return node.and.every((n) => allowed(n, allow));
  return node.or.some((n) => allowed(n, allow));
}

function licenseOk(expr, allow) {
  if (!expr || typeof expr !== "string") return false;
  // Alte Schreibweisen ("MIT/Apache-2.0", "(MIT OR Apache-2.0)") tolerieren.
  const normalized = expr.replace(/\s*\/\s*/g, " OR ");
  try {
    return allowed(parseExpr(tokenize(normalized)), allow);
  } catch {
    return false;
  }
}

// --- Lockfile ------------------------------------------------------------------------------------
function readPackages(lockPath) {
  const lock = JSON.parse(fs.readFileSync(lockPath, "utf8"));
  if (!lock.packages) {
    throw new Error(`${lockPath}: lockfileVersion ${lock.lockfileVersion} ohne 'packages' – bitte mit npm ≥ 7 erzeugen`);
  }
  const out = [];
  for (const [key, meta] of Object.entries(lock.packages)) {
    if (key === "" || meta.link) continue; // Wurzel bzw. Workspace-Verweis
    const name = meta.name || key.slice(key.lastIndexOf("node_modules/") + "node_modules/".length);
    let license = meta.license;
    if (license && typeof license === "object") license = license.type;
    if (Array.isArray(meta.licenses)) license = meta.licenses.map((l) => l.type || l).join(" OR ");
    out.push({ key, name, version: meta.version || "?", license, dev: Boolean(meta.dev || meta.devOptional) });
  }
  return out.sort((a, b) => (a.name === b.name ? a.key.localeCompare(b.key) : a.name.localeCompare(b.name)));
}

function licenseFiles(dir) {
  try {
    return fs
      .readdirSync(dir)
      .filter((f) => /^(licen[cs]e|copying|notice)([.-].*)?$/i.test(f))
      .sort()
      .map((f) => path.join(dir, f));
  } catch {
    return [];
  }
}

function main() {
  const args = process.argv.slice(2);
  const lockPath = args.find((a) => !a.startsWith("--"));
  if (!lockPath) usage(2);
  const ni = args.indexOf("--notices");
  const noticesOut = ni >= 0 ? args[ni + 1] : null;
  if (ni >= 0 && !noticesOut) usage(2);

  const pkgs = readPackages(lockPath);
  const problems = [];
  for (const p of pkgs) {
    const allow = p.dev ? ALLOW_BUILD_ONLY : ALLOW_RUNTIME;
    if (!licenseOk(p.license, allow)) {
      problems.push(`${p.name}@${p.version} (${p.dev ? "Build" : "Laufzeit"}): ${p.license ?? "keine Lizenzangabe"}`);
    }
  }
  const runtime = pkgs.filter((p) => !p.dev);
  console.error(`npm-licenses: ${pkgs.length} Pakete, davon ${runtime.length} zur Laufzeit`);

  if (noticesOut) {
    const root = path.dirname(path.resolve(lockPath));
    const lines = ["## npm-Abhängigkeiten der Oberfläche (npm dependencies of the UI)", ""];
    lines.push(`Erzeugt aus \`${path.basename(lockPath)}\` (nur ausgelieferte Pakete, ohne Build-Werkzeuge).`, "");
    for (const p of runtime) {
      lines.push(`### ${p.name} ${p.version} – ${p.license ?? "?"}`, "");
      const files = licenseFiles(path.join(root, p.key));
      if (files.length === 0) lines.push("_(Lizenztext nicht installiert – `npm ci` ausführen)_", "");
      for (const f of files) {
        lines.push("~~~~text", fs.readFileSync(f, "utf8").replace(/\s+$/, ""), "~~~~", "");
      }
    }
    fs.writeFileSync(noticesOut, lines.join("\n") + "\n");
    console.error(`npm-licenses: Hinweise → ${noticesOut}`);
  }

  if (problems.length > 0) {
    console.error("npm-licenses: nicht zulässige oder fehlende Lizenzen (Plan §2.5):");
    for (const p of problems) console.error(`  - ${p}`);
    process.exit(1);
  }
  console.error("npm-licenses: ok");
}

main();
