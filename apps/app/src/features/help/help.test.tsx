/**
 * Offline-Hilfe: Öffnen über die Kopfzeile und F1, Suche während des Tippens, Artikel mit Zurück, Themen;
 * „Was bedeutet das?“ an einer Fehlermeldung öffnet den Artikel zum Code. Dazu Suche und Artikel ohne Oberfläche.
 */
import { screen, waitFor, within } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { defaultErrorContext } from '../../core/defaults';
import { translator } from '../../i18n/catalog';
import { renderApp } from '../../test/render';
import { viewOf } from '../../test/views';
import { allArticles } from './articles';
import { HelpIndex, stem } from './search';

describe('Suche und Artikel', () => {
  const t = translator('de');
  const index = new HelpIndex(allArticles(t));

  it('Wissensbasis und Fehlercodes sind Artikel mit stabiler Kennung', () => {
    const ids = index.articles.map((a) => a.id);
    expect(ids).toContain('pida:was-ist-pida');
    expect(ids.some((id) => id.startsWith('fehler:'))).toBe(true);
    expect(new Set(ids).size).toBe(ids.length);
  });

  it('findet Alltagswörter (Synonym, Grundform) und den Artikel zum Code', () => {
    expect(index.search('Handysignatur')[0]?.article.topic).toBe('id-austria');
    expect(stem('signaturen')).toBe('signatur');
    const code = index.articles.find((a) => a.topic === 'fehler')?.code ?? '';
    expect(index.search(code)[0]?.article.code).toBe(code);
  });
});

describe('Englisch', () => {
  const en = translator('en');
  const index = new HelpIndex(allArticles(en), 'en');

  it('gleiche Kennungen wie Deutsch, englische Texte und Suche', () => {
    const de = allArticles(translator('de')).filter((a) => a.topic !== 'fehler').map((a) => a.id);
    const ids = index.articles.filter((a) => a.topic !== 'fehler').map((a) => a.id);
    expect(ids).toEqual(de);
    expect(index.articles.find((a) => a.id === 'pida:was-ist-pida')?.title).toBe('What is PIDA?');
    expect(index.search('reference value')[0]?.article.id).toBe('id-austria:wie-signiert-man-mit-id-austria-in-pida');
    expect(index.search('Adobe validity unknown')[0]?.article.id).toBe('pdf:signatur-prufen');
    const proxy = index.articles.find((a) => a.id === 'fehler:backend-proxy-auth');
    expect(proxy?.body).toContain('Code: `backend-proxy-auth`');
    expect(proxy?.body).not.toMatch(/Was Sie tun|Technischen Details/);
  });
});

describe('Hilfe-Panel', () => {
  it('Kopfzeile → Suche → Artikel → Zurück; F1 öffnet ebenfalls', async () => {
    const { user } = renderApp();
    await user.click(await screen.findByRole('button', { name: 'Hilfe' }));
    const panel = await screen.findByRole('dialog', { name: 'Hilfe' });
    await user.type(within(panel).getByRole('searchbox', { name: 'Hilfe durchsuchen' }), 'Was ist PIDA');
    await user.click(await within(panel).findByRole('button', { name: /^Was ist PIDA\?/ }));
    expect(await within(panel).findByRole('heading', { level: 3, name: 'Was ist PIDA?' })).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'Zurück' }));
    expect(await within(panel).findByRole('heading', { name: 'Themen' })).toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'Schließen' }));
    await waitFor(() => expect(screen.queryByRole('dialog', { name: 'Hilfe' })).not.toBeInTheDocument());
    await user.keyboard('{F1}');
    expect(await screen.findByRole('dialog', { name: 'Hilfe' })).toBeInTheDocument();
  });

  it('„Was bedeutet das?“ an der Fehlermeldung öffnet den Artikel zum Code; ohne Artikel kein Link', async () => {
    const error = (code: string) => viewOf({ step: 'error', code, params: {}, retry: true, context: { ...defaultErrorContext(), origin: 'load' } });
    const { user, unmount } = renderApp({ view: error('backend.proxy.auth') });
    await user.click(await screen.findByRole('button', { name: 'Was bedeutet das?' }));
    const panel = await screen.findByRole('dialog', { name: 'Hilfe' });
    expect(await within(panel).findByText('Code:', { exact: false })).toHaveTextContent('backend-proxy-auth');
    unmount();
    renderApp({ view: error('error-MayNotBeAPDF') });
    expect(await screen.findByRole('button', { name: 'Erneut versuchen' })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Was bedeutet das?' })).not.toBeInTheDocument();
  });
});
