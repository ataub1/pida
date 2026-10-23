/**
 * Lizenzen in der App (Backlog Nr. 2): „Lizenzen anzeigen“ öffnet den Dialog über den Einstellungen mit EUPL-1.2,
 * Drittlizenzen und Rust-Standardbibliothek (HTML als Text, ohne Skripte); ohne eingebettete Hinweise ein Hinweis
 * statt eines Fehlers; axe.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { expectNoAxeViolations } from '../../test/a11y';
import { renderApp } from '../../test/render';
import { htmlToText } from './licenses';

async function openLicenses(options: Parameters<typeof renderApp>[0] = {}) {
  const r = renderApp(options);
  const settings = await screen.findByRole('button', { name: 'Einstellungen' });
  await waitFor(() => expect(settings).toBeEnabled(), { timeout: 3000 });
  await r.user.click(settings);
  const panel = await screen.findByRole('dialog', { name: 'Einstellungen' });
  await r.user.click(within(within(panel).getByRole('navigation', { name: /Bereiche/ })).getByRole('button', { name: 'Über PIDA' }));
  await r.user.click(within(panel).getByRole('link', { name: 'Lizenzen anzeigen' }));
  const dialog = await screen.findByRole('dialog', { name: 'Lizenzen' });
  return { ...r, dialog };
}

describe('Lizenzen in der App', () => {
  it('EUPL zuerst, dann Drittkomponenten und Rust-Standardbibliothek (eingebettet, kein Systemprogramm)', async () => {
    const { dialog, user, mock } = await openLicenses();
    expect(await within(dialog).findByTestId('licenses-eupl')).toHaveTextContent('EUROPEAN UNION PUBLIC LICENCE v. 1.2');
    expect(within(dialog).getByRole('tab', { name: 'PIDA (EUPL-1.2)' })).toHaveAttribute('aria-selected', 'true');
    await expectNoAxeViolations();
    await user.click(within(dialog).getByRole('tab', { name: 'Drittkomponenten' }));
    expect(await within(dialog).findByTestId('licenses-notices')).toHaveTextContent('Android-Bibliotheken');
    await user.click(within(dialog).getByRole('tab', { name: 'Rust-Standardbibliothek' }));
    const std = await within(dialog).findByTestId('licenses-rust_std');
    expect(std.textContent).toContain('Copyright notices for The Rust Standard Library\n');
    expect(std.textContent).toContain('• core');
    expect(std.textContent).not.toContain('__pwned');
    expect((window as unknown as { __pwned?: boolean }).__pwned).toBeUndefined();
    expect(mock.nativeCalls.filter((c) => c.startsWith('licenseText:'))).toEqual(['licenseText:eupl', 'licenseText:notices', 'licenseText:rust_std']);
    expect(mock.openedUrls).toEqual([]);
    await user.keyboard('{Escape}');
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Lizenzen' })).not.toBeInTheDocument());
    expect(screen.getByRole('dialog', { name: 'Einstellungen' })).toBeInTheDocument();
  });

  it('Entwicklungsbuild ohne Hinweise: EUPL da, Drittkomponenten mit Hinweis', async () => {
    const { dialog, user } = await openLicenses({ notices: false });
    expect(await within(dialog).findByTestId('licenses-eupl')).toBeInTheDocument();
    await user.click(within(dialog).getByRole('tab', { name: 'Drittkomponenten' }));
    expect(await within(dialog).findByRole('status')).toHaveTextContent('fehlen in diesem Entwicklungsbuild');
  });

  it('htmlToText: Blöcke als Zeilen, Listen mit Punkt, Skripte und Titel fallen weg', () => {
    expect(htmlToText('<title>T</title><h2>A</h2><p>b  c</p><ul><li>x</li></ul><pre>  1\n  2</pre><script>s()</script>')).toBe('A\n\nb c\n\n• x\n\n1\n2');
  });
});
