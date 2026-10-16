// @vitest-environment node
/**
 * Eine App-Version überall (Integration Mobile, 16.10.2026): Cargo-Workspace, package.json, tauri.conf.json
 * sowie die versionierten Projektdateien für iOS (Info.plist, project.yml) und Android (Rückfallwerte in
 * build.gradle.kts, falls die von der CLI erzeugte tauri.properties fehlt). Vorher meldete die App 0.1.0,
 * iOS und Android zeigten 1.0.0 bzw. 1.0.
 */
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const app = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (file: string) => readFileSync(path.join(app, file), 'utf8');

const cargoVersion = /^\[workspace\.package\][^[]*?^version = "([^"]+)"/m.exec(read('../../Cargo.toml'))?.[1] ?? '';

/** Android-Rechnung von Tauri: major · 1000000 + minor · 1000 + patch */
function versionCode(version: string): number {
  const [major = 0, minor = 0, patch = 0] = version.split('.').map(Number);
  return major * 1_000_000 + minor * 1000 + patch;
}

describe('App-Version', () => {
  it('Cargo, package.json und tauri.conf.json stimmen überein', () => {
    expect(cargoVersion).toMatch(/^\d+\.\d+\.\d+$/);
    expect(JSON.parse(read('package.json')).version).toBe(cargoVersion);
    expect(JSON.parse(read('src-tauri/tauri.conf.json')).version).toBe(cargoVersion);
  });

  it('iOS: CFBundleShortVersionString und CFBundleVersion', () => {
    const plist = read('src-tauri/gen/apple/pida-app_iOS/Info.plist');
    expect(plist).toContain(`<key>CFBundleShortVersionString</key>\n\t<string>${cargoVersion}</string>`);
    expect(plist).toContain(`<key>CFBundleVersion</key>\n\t<string>${cargoVersion}</string>`);
    const yml = read('src-tauri/gen/apple/project.yml');
    expect(yml).toMatch(new RegExp(`CFBundleShortVersionString: ${cargoVersion.replace(/\./g, '\\.')}\\n`));
    expect(yml).toContain(`CFBundleVersion: "${cargoVersion}"`);
  });

  it('Android: versionName und versionCode', () => {
    const gradle = read('src-tauri/gen/android/app/build.gradle.kts');
    expect(gradle).toContain(`getProperty("tauri.android.versionName", "${cargoVersion}")`);
    expect(gradle).toContain(`getProperty("tauri.android.versionCode", "${versionCode(cargoVersion)}")`);
  });
});
