/**
 * Einstellungen als Seitenleiste über der App: Änderungen gelten sofort (kein Speichern/Abbrechen),
 * Suche über alle Bereiche. Bereiche: Signaturbild (Live-Editor), Signiermittel, Speichern,
 * Darstellung, Erweitert.
 */
import { useRef, useState, type ReactNode } from 'react';
import { Button as AriaButton, DropZone, FileTrigger, Input, SearchField, type DropItem } from 'react-aria-components';
import { SIGNER_NAME, useAppState, useDispatch, type SettingsSection } from '../../app/store';
import { useT, type MsgKey } from '../../i18n';
import { blockSize, defaultNote, effectiveNote, isoWithOffset, outputName } from '../../model/format';
import type { Lang, ProxyMode, SaveTarget, SignerKind, SigProfile, ThemePref } from '../../model/types';
import { Badge, Button, RadioCards, Segmented, SwitchField, TextAreaInput, TextInput } from '../../ui/controls';
import { DialogHeader, ModalShell } from '../../ui/Dialog';
import { useFitScale } from '../../ui/hooks';
import {
  IconCard,
  IconClose,
  IconFileKey,
  IconFolder,
  IconImage,
  IconMonitor,
  IconMoon,
  IconPhone,
  IconRefresh,
  IconSearch,
  IconSettings,
  IconSignature,
  IconSun,
} from '../../ui/Icons';
import { SignatureBlock } from '../sigblock/SignatureBlock';

interface RowDef {
  id: string;
  section: SettingsSection;
  label: MsgKey;
  extra: string[];
}

const SECTIONS: { id: SettingsSection; label: MsgKey; icon: ReactNode }[] = [
  { id: 'block', label: 'set.sec.block', icon: <IconSignature size={18} /> },
  { id: 'signer', label: 'set.sec.signer', icon: <IconPhone size={18} /> },
  { id: 'save', label: 'set.sec.save', icon: <IconFolder size={18} /> },
  { id: 'appearance', label: 'set.sec.appearance', icon: <IconSun size={18} /> },
  { id: 'advanced', label: 'set.sec.advanced', icon: <IconSettings size={18} /> },
];

const ROWS: RowDef[] = [
  { id: 'block.preview', section: 'block', label: 'set.block.preview', extra: ['signaturbild', 'signature block', 'vorschau', 'preview'] },
  { id: 'block.profile', section: 'block', label: 'set.block.profile', extra: ['amtssignatur', 'standard', 'official', 'profil'] },
  { id: 'block.lang', section: 'block', label: 'set.block.lang', extra: ['sprache', 'language', 'deutsch', 'english'] },
  { id: 'block.logo', section: 'block', label: 'set.block.logo', extra: ['logo', 'bildmarke', 'emblem', 'bild', 'image'] },
  { id: 'block.note', section: 'block', label: 'set.block.note', extra: ['hinweis', 'text', 'note', 'eidas'] },
  { id: 'signer.default', section: 'signer', label: 'set.signer.default', extra: ['id austria', 'keystore', 'bürgerkarte', 'standard'] },
  { id: 'signer.ida', section: 'signer', label: 'set.signer.ida.user', extra: ['telefon', 'handy', 'benutzer', 'phone', 'user', 'merken', 'konto', 'account'] },
  { id: 'signer.fido2', section: 'signer', label: 'set.signer.fido2', extra: ['fido2', 'sicherheitsschlüssel', 'security key', 'yubikey'] },
  { id: 'signer.keystore', section: 'signer', label: 'set.signer.ks.file', extra: ['keystore', 'pkcs#12', 'p12', 'alias', 'zertifikat'] },
  { id: 'signer.card', section: 'signer', label: 'set.signer.card.status', extra: ['bürgerkarte', 'karte', 'card', 'lesegerät'] },
  { id: 'save.target', section: 'save', label: 'set.save.target', extra: ['ordner', 'folder', 'speicherort', 'ausgabe', 'output'] },
  { id: 'save.pattern', section: 'save', label: 'set.save.pattern', extra: ['dateiname', 'name', 'muster', 'signiert', 'pattern', 'postfix'] },
  { id: 'app.theme', section: 'appearance', label: 'set.app.theme', extra: ['dunkel', 'hell', 'dark', 'light', 'theme', 'farbe'] },
  { id: 'app.lang', section: 'appearance', label: 'set.app.lang', extra: ['sprache', 'language', 'english', 'deutsch'] },
  { id: 'adv.proxy', section: 'advanced', label: 'set.adv.proxy', extra: ['netzwerk', 'network', 'host', 'port'] },
  { id: 'adv.updates', section: 'advanced', label: 'set.adv.updates', extra: ['update', 'version', 'aktualisierung'] },
  { id: 'adv.logs', section: 'advanced', label: 'set.adv.logs', extra: ['log', 'protokoll', 'debug', 'fehlersuche'] },
  { id: 'adv.placeholder', section: 'advanced', label: 'set.adv.placeholder', extra: ['qr', 'signaturfeld', 'platzhalter', 'field'] },
  { id: 'adv.privacy', section: 'advanced', label: 'set.adv.privacy', extra: ['datenschutz', 'privacy', 'pdf-over', 'import'] },
];

export function SettingsPanel() {
  const state = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const [query, setQuery] = useState('');
  const isOpen = state.overlay === 'settings';
  const close = () => {
    setQuery('');
    dispatch({ type: 'overlay', overlay: null });
  };
  const q = query.trim().toLowerCase();
  const matches = (r: RowDef) => {
    if (!q) return r.section === state.settingsSection;
    const hay = [t(r.label), ...r.extra, t(SECTIONS.find((s) => s.id === r.section)?.label ?? 'set.title')].join(' ').toLowerCase();
    return q.split(/\s+/).every((w) => hay.includes(w));
  };
  const hits = ROWS.filter(matches);
  // Beim Suchen nach Signaturbild-Einstellungen die Live-Vorschau mitzeigen (Änderungen gelten sofort)
  const visible =
    q && hits.some((r) => r.section === 'block') && !hits.some((r) => r.id === 'block.preview')
      ? [ROWS.find((r) => r.id === 'block.preview') as RowDef, ...hits]
      : hits;
  const sections = SECTIONS.filter((s) => visible.some((r) => r.section === s.id));

  return (
    <ModalShell isOpen={isOpen} onClose={close} variant="drawer" size="lg" className="settings-modal">
      <DialogHeader title={t('set.title')} subtitle={t('set.instant')} onClose={close} icon={<IconSettings size={20} />} />
      <div className="settings-search">
        <SearchField aria-label={t('set.search')} value={query} onChange={setQuery} className="search-field">
          <IconSearch size={17} className="search-icon" />
          <Input className="input search-input" placeholder={t('set.search')} />
          <AriaButton className="search-clear" aria-label={t('set.search.clear')}>
            <IconClose size={14} />
          </AriaButton>
        </SearchField>
      </div>
      <div className={`settings-layout${q ? ' is-searching' : ''}`}>
        {!q ? (
          <nav className="settings-nav" aria-label={t('set.sections')}>
            {SECTIONS.map((s) => (
              <AriaButton
                key={s.id}
                className={`settings-nav-item${state.settingsSection === s.id ? ' is-current' : ''}`}
                aria-current={state.settingsSection === s.id ? 'true' : undefined}
                onPress={() => dispatch({ type: 'overlay', overlay: 'settings', section: s.id })}
              >
                {s.icon}
                <span>{t(s.label)}</span>
              </AriaButton>
            ))}
          </nav>
        ) : null}
        <div className="settings-content">
          {visible.length === 0 ? <p className="muted settings-empty">{t('set.searchEmpty', { q: query })}</p> : null}
          {sections.map((s) => (
            <section key={s.id} className="settings-section" aria-labelledby={`set-h-${s.id}`}>
              <h3 className="settings-section-title" id={`set-h-${s.id}`}>
                {t(s.label)}
              </h3>
              {visible
                .filter((r) => r.section === s.id)
                .map((r) => (
                  <div key={r.id} className="settings-row" data-row={r.id}>
                    <SettingRow id={r.id} />
                  </div>
                ))}
            </section>
          ))}
        </div>
      </div>
    </ModalShell>
  );
}

function SettingRow({ id }: { id: string }) {
  switch (id) {
    case 'block.preview':
      return <BlockPreview />;
    case 'block.profile':
      return <ProfileRow />;
    case 'block.lang':
      return <BlockLangRow />;
    case 'block.logo':
      return <LogoRow />;
    case 'block.note':
      return <NoteRow />;
    case 'signer.default':
      return <DefaultSignerRow />;
    case 'signer.ida':
      return <IdaRow />;
    case 'signer.fido2':
      return <Fido2Row />;
    case 'signer.keystore':
      return <KeystoreRow />;
    case 'signer.card':
      return <CardRow />;
    case 'save.target':
      return <SaveTargetRow />;
    case 'save.pattern':
      return <PatternRow />;
    case 'app.theme':
      return <ThemeRow />;
    case 'app.lang':
      return <UiLangRow />;
    case 'adv.proxy':
      return <ProxyRow />;
    case 'adv.updates':
      return <UpdatesRow />;
    case 'adv.logs':
      return <LogsRow />;
    case 'adv.placeholder':
      return <PlaceholderRow />;
    case 'adv.privacy':
      return <PrivacyRow />;
    default:
      return null;
  }
}

/* ------------------------------------------------------------------ Signaturbild */

function BlockPreview() {
  const { settings } = useAppState();
  const t = useT();
  const ref = useRef<HTMLDivElement>(null);
  const size = blockSize(settings.sigBlock);
  const scale = useFitScale(ref, size.w, { min: 0.6, max: 1.8, pad: 32, initial: settings.sigBlock.profile === 'amt' ? 1 : 1.6 });
  const [time] = useState(() => isoWithOffset(new Date()));
  return (
    <figure className="block-preview" ref={ref}>
      <div className="block-preview-stage" role="img" aria-label={`${t('set.block.previewAlt')}: ${t(settings.sigBlock.profile === 'amt' ? 'set.block.profile.amt' : 'set.block.profile.standard')}, ${settings.sigBlock.lang === 'de' ? t('lang.de') : t('lang.en')}`}>
        <SignatureBlock sig={settings.sigBlock} signer={SIGNER_NAME} time={time} scale={scale} />
      </div>
      <figcaption className="block-preview-caption">{t('set.block.preview')}</figcaption>
    </figure>
  );
}

function ProfileRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <RadioCards<SigProfile>
      label={t('set.block.profile')}
      value={settings.sigBlock.profile}
      onChange={(v) => dispatch({ type: 'sigBlock', patch: { profile: v } })}
      columns={2}
      options={[
        { value: 'standard', label: t('set.block.profile.standard'), description: t('set.block.profile.standard.desc') },
        { value: 'amt', label: t('set.block.profile.amt'), description: t('set.block.profile.amt.desc') },
      ]}
    />
  );
}

function BlockLangRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <Segmented<Lang>
      label={t('set.block.lang')}
      value={settings.sigBlock.lang}
      onChange={(v) => dispatch({ type: 'sigBlock', patch: { lang: v } })}
      options={[
        { value: 'de', label: t('lang.de') },
        { value: 'en', label: t('lang.en') },
      ]}
    />
  );
}

function readImage(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const r = new FileReader();
    r.onload = () => resolve(String(r.result));
    r.onerror = () => reject(r.error);
    r.readAsDataURL(file);
  });
}

const IMAGE_TYPES = ['image/png', 'image/jpeg', 'image/gif', 'image/svg+xml'];

function LogoRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const [error, setError] = useState<string | null>(null);
  const take = async (file: File | null | undefined) => {
    if (!file) return;
    if (!IMAGE_TYPES.includes(file.type)) {
      setError(t('set.block.logo.invalid'));
      return;
    }
    setError(null);
    const url = await readImage(file);
    dispatch({ type: 'sigBlock', patch: { logo: url, logoName: file.name } });
  };
  return (
    <div className="field">
      <span className="field-label" id="logo-label">
        {t('set.block.logo')}
      </span>
      <DropZone
        className="logo-drop"
        aria-label={t('set.block.logo.dropLabel')}
        getDropOperation={() => 'copy'}
        onDrop={async (e) => {
          const item = e.items.find((i: DropItem) => i.kind === 'file') as Extract<DropItem, { kind: 'file' }> | undefined;
          if (item) await take(await item.getFile());
        }}
      >
        {({ isDropTarget }) => (
          <div className={`logo-drop-inner${isDropTarget ? ' is-target' : ''}`}>
            <span className="logo-thumb" aria-hidden="true">
              {settings.sigBlock.logo ? <img src={settings.sigBlock.logo} alt="" /> : <IconImage size={22} />}
            </span>
            <span className="logo-text">
              <span className="logo-name">{settings.sigBlock.logoName ? t('set.block.logo.current', { name: settings.sigBlock.logoName }) : t('set.block.logo.default')}</span>
              <span className="field-hint">
                {t('set.block.logo.drop')} · {t('set.block.logo.hint')}
              </span>
            </span>
            <span className="logo-actions">
              <FileTrigger acceptedFileTypes={IMAGE_TYPES} onSelect={(files) => void take(files?.[0])}>
                <Button variant="secondary" size="sm">
                  {t('set.block.logo.choose')}
                </Button>
              </FileTrigger>
              {settings.sigBlock.logo ? (
                <Button variant="ghost" size="sm" onPress={() => dispatch({ type: 'sigBlock', patch: { logo: null, logoName: null } })}>
                  {t('common.remove')}
                </Button>
              ) : null}
            </span>
          </div>
        )}
      </DropZone>
      {error ? (
        <p className="field-error" role="alert">
          {error}
        </p>
      ) : null}
    </div>
  );
}

function NoteRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const sig = settings.sigBlock;
  const value = effectiveNote(sig);
  const isDefault = sig.note === null;
  return (
    <div className="note-row">
      <TextAreaInput
        label={t('set.block.note')}
        value={value}
        onChange={(v) => dispatch({ type: 'sigBlock', patch: { note: v === defaultNote(sig.profile, sig.lang) ? null : v } })}
        description={t('set.block.note.hint')}
        rows={4}
      />
      {!isDefault ? (
        <Button variant="ghost" size="sm" onPress={() => dispatch({ type: 'sigBlock', patch: { note: null } })}>
          <IconRefresh size={15} />
          {t('set.block.note.reset')}
        </Button>
      ) : null}
    </div>
  );
}

/* ------------------------------------------------------------------ Signiermittel */

function DefaultSignerRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <RadioCards<SignerKind | 'ask'>
      label={t('set.signer.default')}
      value={settings.defaultSigner}
      onChange={(v) => dispatch({ type: 'settings', patch: { defaultSigner: v } })}
      columns={2}
      options={[
        { value: 'idaustria', label: t('signer.idaustria'), description: t('signer.idaustria.desc'), icon: <IconPhone size={18} /> },
        { value: 'keystore', label: t('signer.keystore'), description: t('signer.keystore.desc'), icon: <IconFileKey size={18} /> },
        { value: 'card', label: t('signer.card'), description: t('signer.card.desc'), icon: <IconCard size={18} /> },
        { value: 'ask', label: t('signer.ask'), description: t('signer.ask.desc') },
      ]}
    />
  );
}

function AccountCard({ icon, title, children }: { icon: ReactNode; title: string; children: ReactNode }) {
  return (
    <div className="account-card">
      <div className="account-head">
        <span className="account-icon" aria-hidden="true">
          {icon}
        </span>
        <h4 className="account-title">{title}</h4>
      </div>
      <div className="account-body">{children}</div>
    </div>
  );
}

function IdaRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <AccountCard icon={<IconPhone size={18} />} title={t('signer.idaustria')}>
      <TextInput label={t('set.signer.ida.user')} value={settings.idaUser} onChange={(v) => dispatch({ type: 'settings', patch: { idaUser: v } })} inputMode="tel" autoComplete="username" />
      <SwitchField label={t('set.signer.ida.remember')} isSelected={settings.rememberUser} onChange={(v) => dispatch({ type: 'settings', patch: { rememberUser: v } })} />
    </AccountCard>
  );
}

function Fido2Row() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <SwitchField
      label={t('set.signer.fido2')}
      hint={t('set.signer.fido2.hint')}
      isSelected={settings.fido2Default}
      onChange={(v) => dispatch({ type: 'settings', patch: { fido2Default: v } })}
    />
  );
}

function KeystoreRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <AccountCard icon={<IconFileKey size={18} />} title={t('signer.keystore')}>
      <div className="path-row">
        <span className="path-label">{t('set.signer.ks.file')}</span>
        <span className="path-value mono">~/Zertifikate/{settings.keystoreFile}</span>
        <Button variant="ghost" size="sm" onPress={() => dispatch({ type: 'toast', toast: { kind: 'info', title: t('set.signer.ks.demo') } })}>
          {t('common.change')}
        </Button>
      </div>
      <TextInput label={t('set.signer.ks.alias')} value={settings.keystoreAlias} onChange={(v) => dispatch({ type: 'settings', patch: { keystoreAlias: v } })} />
    </AccountCard>
  );
}

function CardRow() {
  const t = useT();
  return (
    <AccountCard icon={<IconCard size={18} />} title={t('signer.card')}>
      <div className="path-row">
        <span className="path-label">{t('set.signer.card.status')}</span>
        <Badge tone="success">{t('set.signer.card.found')}</Badge>
      </div>
    </AccountCard>
  );
}

/* ------------------------------------------------------------------ Speichern */

function SaveTargetRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <div className="stack-3">
      <RadioCards<SaveTarget>
        label={t('set.save.target')}
        value={settings.saveTarget}
        onChange={(v) => dispatch({ type: 'settings', patch: { saveTarget: v } })}
        options={[
          { value: 'original', label: t('set.save.original'), description: '~/Dokumente/Verträge' },
          { value: 'folder', label: t('set.save.folder'), description: settings.saveFolder },
          { value: 'ask', label: t('set.save.ask') },
        ]}
      />
      {settings.saveTarget === 'folder' ? (
        <div className="path-row">
          <span className="path-label">{t('set.save.folderPath')}</span>
          <span className="path-value mono">{settings.saveFolder}</span>
          <Button
            variant="ghost"
            size="sm"
            onPress={() => dispatch({ type: 'settings', patch: { saveFolder: settings.saveFolder === '~/Dokumente/Signiert' ? '~/Schreibtisch/Signiert' : '~/Dokumente/Signiert' } })}
          >
            {t('common.change')}
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function PatternRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <div className="stack-2">
      <TextInput
        label={t('set.save.pattern')}
        value={settings.namePattern}
        onChange={(v) => dispatch({ type: 'settings', patch: { namePattern: v } })}
        description={t('set.save.pattern.hint')}
        inputClassName="mono"
        spellCheck="false"
      />
      <p className="pattern-preview" aria-live="polite">
        <span className="pattern-label">{t('set.save.preview')}:</span> <span className="mono">Vertrag.pdf</span> → <strong className="mono">{outputName('Vertrag.pdf', settings.namePattern)}</strong>
      </p>
    </div>
  );
}

/* ------------------------------------------------------------------ Darstellung */

function ThemeRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <Segmented<ThemePref>
      label={t('set.app.theme')}
      value={settings.theme}
      onChange={(v) => dispatch({ type: 'settings', patch: { theme: v } })}
      options={[
        { value: 'system', label: t('theme.system'), icon: <IconMonitor size={16} /> },
        { value: 'light', label: t('theme.light'), icon: <IconSun size={16} /> },
        { value: 'dark', label: t('theme.dark'), icon: <IconMoon size={16} /> },
      ]}
    />
  );
}

function UiLangRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <Segmented<Lang>
      label={t('set.app.lang')}
      value={settings.uiLang}
      onChange={(v) => dispatch({ type: 'settings', patch: { uiLang: v } })}
      options={[
        { value: 'de', label: t('lang.de') },
        { value: 'en', label: t('lang.en') },
      ]}
    />
  );
}

/* ------------------------------------------------------------------ Erweitert */

function ProxyRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  const port = Number(settings.proxyPort);
  const portErr = settings.proxyPort && (!Number.isInteger(port) || port < 1 || port > 65535) ? t('set.adv.proxy.portErr') : null;
  return (
    <div className="stack-3">
      <Segmented<ProxyMode>
        label={t('set.adv.proxy')}
        value={settings.proxy}
        onChange={(v) => dispatch({ type: 'settings', patch: { proxy: v } })}
        options={[
          { value: 'system', label: t('set.adv.proxy.system') },
          { value: 'none', label: t('set.adv.proxy.none') },
          { value: 'manual', label: t('set.adv.proxy.manual') },
        ]}
      />
      {settings.proxy === 'manual' ? (
        <div className="grid-2">
          <TextInput label={t('set.adv.proxy.host')} value={settings.proxyHost} onChange={(v) => dispatch({ type: 'settings', patch: { proxyHost: v } })} placeholder="proxy.example.at" />
          <TextInput label={t('set.adv.proxy.port')} value={settings.proxyPort} onChange={(v) => dispatch({ type: 'settings', patch: { proxyPort: v } })} inputMode="numeric" error={portErr} placeholder="8080" />
        </div>
      ) : null}
    </div>
  );
}

function UpdatesRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <SwitchField label={t('set.adv.updates')} hint={t('set.adv.updates.status')} isSelected={settings.autoUpdate} onChange={(v) => dispatch({ type: 'settings', patch: { autoUpdate: v } })} />
  );
}

function LogsRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return (
    <div className="stack-2">
      <SwitchField label={t('set.adv.logs.debug')} isSelected={settings.debugLog} onChange={(v) => dispatch({ type: 'settings', patch: { debugLog: v } })} />
      <Button variant="secondary" size="sm" onPress={() => dispatch({ type: 'toast', toast: { kind: 'info', title: t('set.adv.logs.demo') } })}>
        <IconFolder size={15} />
        {t('set.adv.logs.open')}
      </Button>
    </div>
  );
}

function PlaceholderRow() {
  const { settings } = useAppState();
  const dispatch = useDispatch();
  const t = useT();
  return <SwitchField label={t('set.adv.placeholder')} isSelected={settings.placeholderSearch} onChange={(v) => dispatch({ type: 'settings', patch: { placeholderSearch: v } })} />;
}

function PrivacyRow() {
  const dispatch = useDispatch();
  const t = useT();
  return (
    <div className="button-row">
      <Button
        variant="secondary"
        size="sm"
        onPress={() => {
          dispatch({ type: 'resetPrivacy' });
          dispatch({ type: 'toast', toast: { kind: 'info', title: t('set.adv.privacy.done') } });
        }}
      >
        {t('set.adv.privacy')}
      </Button>
      <Button variant="secondary" size="sm" onPress={() => dispatch({ type: 'overlay', overlay: 'onboarding' })}>
        {t('set.adv.import')}
      </Button>
    </div>
  );
}
