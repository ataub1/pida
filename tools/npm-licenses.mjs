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
//   deny.toml. Alles andere – auch fehlende Lizenzangaben – bricht ab. `devOptional` zählt zur
//   Laufzeit: npm installiert solche Pakete auch ohne Dev-Abhängigkeiten (optionale Abhängigkeit
//   eines Laufzeitpakets).
// - Reine Build-Werkzeuge (`dev: true`, nicht ausgeliefert): zusätzlich übliche permissive
//   Lizenzen für Daten und Werkzeuge (CC0, CC-BY, 0BSD, BlueOak, Python-2.0 …). Copyleft (GPL,
//   AGPL, LGPL, SSPL, EUPL fremder Pakete …) bricht auch hier ab.
// - Was der Bundler in die App übernimmt (auch Framework-Laufzeit), gehört in `dependencies`,
//   nicht in `devDependencies` – sonst fehlt es in den Hinweisen.
// - Eigene Workspace-Pakete (Lockfile-Einträge außerhalb von `node_modules/`) sind kein
//   Drittcode und werden übersprungen.
// Ohne Abhängigkeiten außer Node ≥ 18.

import fs from "node:fs";
import os from "node:os";
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
  console.error("        node tools/npm-licenses.mjs --self-test");
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
      const exception = next(); // Ausnahme (z. B. LLVM-exception) erweitert nur Rechte
      if (exception === undefined || exception === "(" || exception === ")") {
        throw new Error("Ausnahme nach WITH fehlt");
      }
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
    // Wurzel, Workspace-Verweis (`link`) und Quellen eigener Workspace-Pakete (`packages/ui` …)
    if (key === "" || meta.link || !/(^|\/)node_modules\//.test(key)) continue;
    const name = meta.name || key.slice(key.lastIndexOf("node_modules/") + "node_modules/".length);
    let license = meta.license;
    if (license && typeof license === "object") license = license.type;
    if (Array.isArray(meta.licenses)) license = meta.licenses.map((l) => l.type || l).join(" OR ");
    // Nur `dev` ist reines Build-Werkzeug; `devOptional` kann ausgeliefert werden (s. o.).
    out.push({ key, name, version: meta.version || "?", license, dev: meta.dev === true });
  }
  return out.sort((a, b) => (a.name === b.name ? a.key.localeCompare(b.key) : a.name.localeCompare(b.name)));
}

function licenseFiles(dir) {
  try {
    return fs
      .readdirSync(dir, { withFileTypes: true })
      .filter((e) => e.isFile() && /^(licen[cs]e|copying|notice)([.-].*)?$/i.test(e.name))
      .map((e) => path.join(dir, e.name))
      .sort();
  } catch {
    return [];
  }
}

function checkPackages(pkgs) {
  const problems = [];
  for (const p of pkgs) {
    const allow = p.dev ? ALLOW_BUILD_ONLY : ALLOW_RUNTIME;
    if (!licenseOk(p.license, allow)) {
      problems.push(`${p.name}@${p.version} (${p.dev ? "Build" : "Laufzeit"}): ${p.license ?? "keine Lizenzangabe"}`);
    }
  }
  return problems;
}

// --- Selbsttest (CI-Job `deny`): Parser und Einordnung des Lockfiles ------------------------------
function selfTest() {
  const failures = [];
  const expect = (what, actual, expected) => {
    if (actual !== expected) failures.push(`${what}: erwartet ${expected}, erhalten ${actual}`);
  };
  const runtimeCases = [
    ["MIT", true],
    ["(MIT OR Apache-2.0)", true],
    ["MIT/Apache-2.0", true],
    ["Apache-2.0 WITH LLVM-exception", true],
    ["MIT AND BSD-3-Clause", true],
    ["MIT OR GPL-3.0-only", true],
    ["MIT AND GPL-3.0-only", false],
    ["GPL-2.0+", false],
    ["LGPL-2.1-or-later", false],
    ["CC-BY-4.0", false],
    ["Apache-2.0 WITH", false],
    ["(MIT", false],
    ["MIT OR", false],
    ["SEE LICENSE IN LICENSE.txt", false],
    ["", false],
    [undefined, false],
  ];
  for (const [expr, ok] of runtimeCases) expect(`Laufzeit ${JSON.stringify(expr)}`, licenseOk(expr, ALLOW_RUNTIME), ok);
  expect("Build CC-BY-4.0", licenseOk("CC-BY-4.0", ALLOW_BUILD_ONLY), true);
  expect("Build AGPL-3.0-only", licenseOk("AGPL-3.0-only", ALLOW_BUILD_ONLY), false);

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "npm-licenses-"));
  try {
    const lock = {
      lockfileVersion: 3,
      packages: {
        "": { name: "app", license: "EUPL-1.2" },
        "packages/ui": { name: "ui", version: "0.1.0", license: "EUPL-1.2" },
        "node_modules/ui": { resolved: "packages/ui", link: true },
        "node_modules/runtime-ok": { version: "1.0.0", license: "MIT" },
        "node_modules/build-cc": { version: "1.0.0", license: "CC-BY-4.0", dev: true },
        "node_modules/optional-cc": { version: "1.0.0", license: "CC-BY-4.0", devOptional: true },
        "node_modules/a/node_modules/nested-gpl": { version: "2.0.0", license: "GPL-3.0-only" },
        "node_modules/legacy": { version: "0.1.0", licenses: [{ type: "MIT" }, { type: "Apache-2.0" }] },
      },
    };
    const lockPath = path.join(dir, "package-lock.json");
    fs.writeFileSync(lockPath, JSON.stringify(lock));
    const pkgs = readPackages(lockPath);
    expect("eigene Workspace-Pakete übersprungen", pkgs.some((p) => p.name === "ui" || p.name === "app"), false);
    expect("devOptional zählt zur Laufzeit", pkgs.find((p) => p.name === "optional-cc")?.dev, false);
    expect("dev ist Build-Werkzeug", pkgs.find((p) => p.name === "build-cc")?.dev, true);
    expect("Name verschachtelter Pakete", pkgs.some((p) => p.name === "nested-gpl"), true);
    const problems = checkPackages(pkgs).map((p) => p.split("@")[0]).sort().join(",");
    expect("Befunde", problems, "nested-gpl,optional-cc");
  } finally {
    fs.rmSync(dir, { recursive: true, force: true });
  }
  if (failures.length > 0) {
    console.error("npm-licenses: Selbsttest fehlgeschlagen:");
    for (const f of failures) console.error(`  - ${f}`);
    process.exit(1);
  }
  console.error("npm-licenses: Selbsttest ok");
}

function main() {
  const args = process.argv.slice(2);
  if (args.includes("--self-test")) {
    selfTest();
    return;
  }
  let lockPath = null;
  let noticesOut = null;
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--notices") {
      noticesOut = args[++i];
      if (!noticesOut) usage(2);
    } else if (args[i].startsWith("--") || lockPath) {
      usage(2);
    } else {
      lockPath = args[i];
    }
  }
  if (!lockPath) usage(2);

  const pkgs = readPackages(lockPath);
  const problems = checkPackages(pkgs);
  const runtime = pkgs.filter((p) => !p.dev);
  console.error(`npm-licenses: ${pkgs.length} Pakete, davon ${runtime.length} zur Laufzeit`);

  if (noticesOut) {
    const root = path.dirname(path.resolve(lockPath));
    const lines = ["## npm-Abhängigkeiten der Oberfläche (npm dependencies of the UI)", ""];
    lines.push(`Erzeugt aus \`${path.basename(lockPath)}\` (nur ausgelieferte Pakete, ohne Build-Werkzeuge).`, "");
    let withoutText = 0;
    for (const p of runtime) {
      lines.push(`### ${p.name} ${p.version} – ${p.license ?? "?"}`, "");
      // Nur innerhalb des App-Verzeichnisses lesen (Schlüssel stammen aus dem Lockfile).
      const dir = path.resolve(root, p.key);
      const inside = !path.relative(root, dir).startsWith("..") && !path.isAbsolute(path.relative(root, dir));
      const files = inside ? licenseFiles(dir) : [];
      if (files.length === 0) {
        withoutText += 1;
        lines.push("_(Lizenztext nicht installiert – `npm ci` ausführen)_", "");
      }
      for (const f of files) {
        lines.push("~~~~text", fs.readFileSync(f, "utf8").replace(/\s+$/, ""), "~~~~", "");
      }
    }
    fs.writeFileSync(noticesOut, lines.join("\n") + "\n");
    console.error(`npm-licenses: Hinweise → ${noticesOut}`);
    if (withoutText > 0) {
      console.error(`npm-licenses: warnung: ${withoutText} Laufzeitpaket(e) ohne Lizenzdatei (npm ci ausgeführt?)`);
    }
  }

  if (problems.length > 0) {
    console.error("npm-licenses: nicht zulässige oder fehlende Lizenzen (Plan §2.5):");
    for (const p of problems) console.error(`  - ${p}`);
    process.exit(1);
  }
  console.error("npm-licenses: ok");
}

main();
