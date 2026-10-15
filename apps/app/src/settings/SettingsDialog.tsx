/**
 * Einstellungen als Overlay über jedem Schritt (F-101, docs/analyse/05 §2): Tabs Einfach, Erweitert,
 * Keystore (nur wenn aktiviert), FIDO2, Über. Bearbeitet wird eine **Arbeitskopie**; „Speichern“
 * sendet die vollständigen Einstellungen (`UiEvent::SaveSettings`, unbekannte Einträge bleiben
 * erhalten), „Abbrechen“/Escape verwirft (`CloseSettings`). Validierungsprobleme kommen vom Kern
 * (`SettingsView::problems`).
 */
import { useEffect, useMemo, useRef, useState } from 'react';
import {
  Button,
  Dialog,
  Heading,
  Label,
  Modal,
  ModalOverlay,
  Slider,
  SliderOutput,
  SliderThumb,
  SliderTrack,
  Tab,
  TabList,
  TabPanel,
  Tabs,
  type Key,
} from 'react-aria-components';
import { useApp } from '../app/context';
import { IconClose } from '../components/icons';
import { AccessButton, ConfirmDialog, Markup, Mnemonic } from '../components/ui';
import { withSection, type SettingsSection } from '../core/defaults';
import type { BackendChoice, Lang, NoteSetting, Settings, SettingsView, SignatureProfile } from '../core/types';
import { translator } from '../i18n/catalog';
import { CheckField, RadioField, SelectField, TextInputField, useProblem } from './fields';
import { applyTheme, readTheme, writeTheme, type Theme } from './theme';

/** Ereignis, mit dem `App` eine auf die Blockvorschau gezogene Bilddatei meldet. */
export const EMBLEM_DROP_EVENT = 'pdfover:emblem-drop';

type Update = <K extends SettingsSection>(section: K, patch: Partial<Settings[K]>) => void;

interface TabProps {
  draft: Settings;
  update: Update;
  view: SettingsView;
}

const PROFILES: SignatureProfile[] = ['SIGNATURBLOCK_SMALL', 'AMTSSIGNATURBLOCK', 'BASE_LOGO', 'INVISIBLE'];
const NOTE_IDS: Partial<Record<SignatureProfile, string>> = {
  SIGNATURBLOCK_SMALL: 'simple_config-Note_Default_Standard',
  AMTSSIGNATURBLOCK: 'simple_config-Note_Default_OfficialSignature',
};

/** Regler der Logo-Größe (PDF-Over: 10–200 mm, Anzeige in cm). */
const LOGO_MIN_CM = 1;
const LOGO_MAX_CM = 20;

/**
 * Logo-Größe in cm für den Regler. Der Regler arbeitet in cm, damit Screenreader den richtigen Wert
 * hören (`aria-valuetext` „2,3 cm“, nicht „23 cm“); gespeichert wird weiter in mm.
 */
export function logoCm(mm: number): number {
  const cm = Math.round(mm) / 10;
  return Number.isFinite(cm) ? Math.min(LOGO_MAX_CM, Math.max(LOGO_MIN_CM, cm)) : 2.3;
}

/** Keystore-Typ aus der Dateiendung wie PDF-Over (`KeystoreConfigurationComposite`). */
export function keystoreTypeFromFile(file: string): 'PKCS12' | 'JCEKS' | null {
  const m = /\.([^./\\]+)$/.exec(file.trim());
  const ext = m?.[1]?.toLowerCase();
  if (ext === 'p12' || ext === 'pkcs12' || ext === 'pfx') return 'PKCS12';
  if (ext === 'ks' || ext === 'jks') return 'JCEKS';
  return null;
}

/** Inhaltsvergleich unabhängig von der Schlüsselreihenfolge. */
function stableJson(value: unknown): string {
  return JSON.stringify(value, (_key, v: unknown) =>
    v && typeof v === 'object' && !Array.isArray(v)
      ? Object.fromEntries(Object.entries(v as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)))
      : v,
  );
}

function langValue(v: Lang | null): 'system' | Lang {
  return v ?? 'system';
}

function SimpleTab({ draft, update, view }: TabProps) {
  const { t, bridge, toast, view: appView } = useApp();
  const sig = draft.signature;
  const visible = sig.profile !== 'INVISIBLE';
  const hasNote = sig.profile in NOTE_IDS;
  const blockLang: Lang = sig.block_language ?? appView.language;
  const noteId = NOTE_IDS[sig.profile];
  const defaultNote = noteId ? translator(blockLang).t(noteId) : '';
  const noteText = sig.note.mode === 'custom' ? sig.note.text : sig.note.mode === 'default' ? defaultNote : '';
  const setNote = (text: string) => {
    const note: NoteSetting = text === defaultNote ? { mode: 'default' } : text === '' ? { mode: 'none' } : { mode: 'custom', text };
    update('signature', { note });
  };
  const previewRev = useMemo(() => Date.now(), []);
  const pickEmblem = async () => {
    try {
      const files = await bridge.pickFiles({
        title: t.t('simple_config-ReplaceEmblem'),
        multiple: false,
        filters: [
          { name: t.t('common-ImageExtension_Description'), extensions: ['png', 'jpg', 'jpeg', 'gif'] },
          { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
        ],
        defaultPath: sig.emblem,
      });
      if (files && files[0]) update('signature', { emblem: files[0] });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };
  const usernameError = useProblem(view.problems, 'id_austria.username');
  const emblemError = useProblem(view.problems, 'signature.emblem');
  return (
    <div className="tab-grid">
      <section className="group">
        <h3 className="group-title">{t.t('simple_config-MobileBKU_Title')}</h3>
        <TextInputField
          label={t.t('simple_config-PhoneNumber')}
          value={draft.id_austria.username ?? ''}
          onChange={(v) => update('id_austria', { username: v.trim() === '' ? null : v })}
          placeholder={t.t('simple_config-ExampleNumber')}
          description={t.t('simple_config-ExampleNumber_ToolTip')}
          error={usernameError}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('simple_config-SigProfile_Title')}</h3>
        <RadioField
          label={t.t('simple_config-SigProfile_Title')}
          hideLabel
          value={sig.profile}
          onChange={(profile) => update('signature', { profile })}
          options={PROFILES.map((p) => ({ id: p, label: t.t(`simple_config-${p}`) }))}
        />
        {visible && sig.profile !== 'BASE_LOGO' ? (
          <SelectField
            label={t.t('simple_config-SigBlockLang_Title')}
            accessMsg="simple_config-SigBlockLang_Title"
            description={t.t('simple_config-SigBlockLang_ToolTip')}
            value={langValue(sig.block_language)}
            onChange={(v) => update('signature', { block_language: v === 'system' ? null : v })}
            options={[
              { id: 'system', label: t.t('ng-ui-lang-system') },
              { id: 'de', label: t.t('ng-ui-lang-de') },
              { id: 'en', label: t.t('ng-ui-lang-en') },
            ]}
          />
        ) : null}
      </section>

      {hasNote ? (
        <section className="group">
          <h3 className="group-title">
            <Mnemonic text={t.t('simple_config-Note_Title')} accessKey={null} />
          </h3>
          <TextInputField
            label={t.t('simple_config-Note')}
            value={noteText}
            onChange={setNote}
            description={t.t('simple_config-Note_Tooltip')}
            multiline
          />
          <div className="actions actions-start">
            <AccessButton msg="simple_config-Note_SetDefault" variant="quiet" isDisabled={sig.note.mode === 'default'} onPress={() => update('signature', { note: { mode: 'default' } })} />
          </div>
        </section>
      ) : null}

      {sig.profile === 'BASE_LOGO' ? (
        <section className="group">
          <Slider
            className="slider"
            minValue={LOGO_MIN_CM}
            maxValue={LOGO_MAX_CM}
            step={0.1}
            value={logoCm(sig.logo_only_size_mm)}
            onChange={(v) => update('signature', { logo_only_size_mm: Math.round((Array.isArray(v) ? (v[0] ?? 2.3) : v) * 10) })}
            formatOptions={{ style: 'unit', unit: 'centimeter', minimumFractionDigits: 1, maximumFractionDigits: 1 }}
          >
            <div className="slider-head">
              <Label className="field-label">{t.t('simple_config-LogoOnlyTargetSize_Title')}</Label>
              <SliderOutput className="slider-output" />
            </div>
            <SliderTrack className="slider-track">
              {({ state }) => (
                <>
                  <div className="slider-fill" style={{ width: `${state.getThumbPercent(0) * 100}%` }} />
                  <SliderThumb className="slider-thumb" aria-label={t.t('simple_config-LogoOnlyTargetSize_Title')} />
                </>
              )}
            </SliderTrack>
            <div className="slider-scale hint" aria-hidden="true">
              <span>1 cm</span>
              <span>20 cm</span>
            </div>
          </Slider>
        </section>
      ) : null}

      {visible ? (
        <section className="group">
          <h3 className="group-title">{t.t('simple_config-Preview_Title')}</h3>
          <div className="emblem-row">
            <div className="emblem-preview" data-emblem-drop="" aria-describedby="emblem-hint">
              <img src={bridge.blockPreviewUrl(2, previewRev)} alt={t.t('ng-ui-settings-preview-alt')} />
            </div>
            <div className="emblem-controls">
              <p className="hint">{sig.emblem ? sig.emblem : t.t('ng-ui-emblem-none')}</p>
              {emblemError ? <p className="message message-error">{emblemError}</p> : null}
              <div className="actions actions-start">
                <AccessButton msg="simple_config-ReplaceEmblem" onPress={() => void pickEmblem()} />
                <AccessButton msg="simple_config-ClearEmblem" variant="quiet" isDisabled={!sig.emblem} onPress={() => update('signature', { emblem: null })} />
              </div>
              <p id="emblem-hint" className="hint">
                {t.t('ng-ui-emblem-drop-hint')}
              </p>
            </div>
          </div>
          <p className="hint">{t.t('ng-ui-settings-preview-note')}</p>
        </section>
      ) : null}
    </div>
  );
}

function AdvancedTab({ draft, update, view }: TabProps) {
  const { t, bridge, toast } = useApp();
  const [theme, setTheme] = useState<Theme>(readTheme);
  const folderError = useProblem(view.problems, 'output.folder');
  const postfixError = useProblem(view.problems, 'output.postfix');
  const hostError = useProblem(view.problems, 'proxy.host');
  const portError = useProblem(view.problems, 'proxy.port');
  const manual = draft.proxy.mode === 'manual';
  const browseFolder = async () => {
    try {
      const dir = await bridge.pickDirectory({ title: t.t('advanced_config-OutputFolder-Dialog_Title'), defaultPath: draft.output.folder });
      if (dir) update('output', { folder: dir });
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };
  const backendOptions: { id: 'none' | BackendChoice; label: string }[] = [
    { id: 'none', label: t.t('BKU-NONE') },
    { id: 'id_austria', label: t.t('BKU-MOBILE') },
    { id: 'local_bku', label: t.t('BKU-LOCAL') },
  ];
  if (draft.keystore.enabled) backendOptions.push({ id: 'keystore', label: t.t('BKU-KS') });
  return (
    <div className="tab-grid">
      <section className="group">
        <h3 className="group-title">{t.t('ng-ui-settings-interface')}</h3>
        <SelectField
          label={t.t('advanced_config-LocaleSelection_Title')}
          accessMsg="advanced_config-LocaleSelection_Title"
          description={t.t('advanced_config-LocaleSelection_ToolTip')}
          value={langValue(draft.general.ui_language)}
          onChange={(v) => update('general', { ui_language: v === 'system' ? null : v })}
          options={[
            { id: 'system', label: t.t('ng-ui-lang-system') },
            { id: 'de', label: t.t('ng-ui-lang-de') },
            { id: 'en', label: t.t('ng-ui-lang-en') },
          ]}
        />
        <SelectField
          label={t.t('ng-ui-theme')}
          value={theme}
          onChange={(v) => {
            setTheme(v);
            writeTheme(v);
            applyTheme(v);
          }}
          options={[
            { id: 'system', label: t.t('ng-ui-theme-system') },
            { id: 'light', label: t.t('ng-ui-theme-light') },
            { id: 'dark', label: t.t('ng-ui-theme-dark') },
          ]}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('advanced_config-Signature_Title')}</h3>
        <CheckField
          label={t.t('advanced_config-AutoPosition')}
          description={t.t('advanced_config-AutoPosition_ToolTip')}
          isSelected={draft.signature.auto_position}
          onChange={(v) => update('signature', { auto_position: v })}
        />
        <CheckField
          label={t.t('advanced_config-PdfACompat')}
          description={t.t('advanced_config-PdfACompat_ToolTip')}
          isSelected={draft.signature.pdfa_force}
          onChange={(v) => update('signature', { pdfa_force: v })}
        />
        <SelectField
          label={t.t('ng-ui-cms-profile')}
          description={t.t('ng-ui-cms-profile-hint')}
          value={draft.signature.cms_profile}
          onChange={(v) => update('signature', { cms_profile: v })}
          options={[
            { id: 'pdfas_compat', label: t.t('ng-ui-cms-pdfas') },
            { id: 'pades_bb', label: t.t('ng-ui-cms-pades') },
          ]}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('advanced_config-Placeholder_Title')}</h3>
        <CheckField
          label={t.t('advanced_config-Placeholder_Enabled')}
          isSelected={draft.placeholder.search}
          onChange={(v) => update('placeholder', { search: v })}
        />
        <RadioField
          label={t.t('ng-ui-placeholder-kind')}
          value={draft.placeholder.kind}
          isDisabled={!draft.placeholder.search}
          onChange={(kind) => update('placeholder', { kind })}
          options={[
            { id: 'marker', label: t.t('advanced_config-UseMarker'), description: t.t('advanced_config-UseMarker_ToolTip') },
            { id: 'fields', label: t.t('advanced_config-UseSignatureFields') },
          ]}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('advanced_config-BKUSelection_Title')}</h3>
        <SelectField
          label={t.t('advanced_config-BKUSelection_Title')}
          accessMsg="advanced_config-BKUSelection_Title"
          description={t.t('advanced_config-BKUSelection_ToolTip')}
          value={draft.backend.default ?? 'none'}
          onChange={(v) => update('backend', { default: v === 'none' ? null : v })}
          options={backendOptions}
        />
        <CheckField
          label={t.t('advanced_config-KeystoreEnabled')}
          description={t.t('advanced_config-KeystoreEnabled_ToolTip')}
          isSelected={draft.keystore.enabled}
          onChange={(enabled) => {
            update('keystore', { enabled });
            if (!enabled && draft.backend.default === 'keystore') update('backend', { default: null });
          }}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('advanced_config-OutputFolder_Title')}</h3>
        <TextInputField
          label={t.t('advanced_config-OutputFolder')}
          value={draft.output.folder ?? ''}
          onChange={(v) => update('output', { folder: v.trim() === '' ? null : v })}
          description={t.t('advanced_config-OutputFolder_ToolTip')}
          error={folderError}
          trailing={<AccessButton msg="common-browse" onPress={() => void browseFolder()} />}
        />
        <TextInputField
          label={t.t('AdvancedConfigurationComposite-lblSaveFilePostFix-text')}
          value={draft.output.postfix}
          placeholder="_signed"
          onChange={(v) => update('output', { postfix: v })}
          description={t.t('ng-ui-postfix-hint')}
          error={postfixError}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('advanced_config-Proxy_Title')}</h3>
        <RadioField
          label={t.t('ng-ui-proxy-mode')}
          value={draft.proxy.mode}
          onChange={(mode) => update('proxy', { mode })}
          options={[
            { id: 'system', label: t.t('ng-ui-proxy-system') },
            { id: 'none', label: t.t('ng-ui-proxy-none') },
            { id: 'manual', label: t.t('ng-ui-proxy-manual') },
          ]}
        />
        <TextInputField
          label={t.t('advanced_config-ProxyHost')}
          value={draft.proxy.host ?? ''}
          placeholder={t.t('advanced_config-ProxyHost_Template')}
          onChange={(v) => update('proxy', { host: v.trim() === '' ? null : v.trim() })}
          isDisabled={!manual}
          error={hostError}
        />
        <TextInputField
          label={t.t('advanced_config-ProxyPort')}
          value={draft.proxy.port == null ? '' : String(draft.proxy.port)}
          placeholder={t.t('advanced_config-ProxyPort_Template')}
          onChange={(v) => {
            const digits = v.replace(/[^0-9]/g, '').slice(0, 5);
            update('proxy', { port: digits === '' ? null : Number(digits) });
          }}
          isDisabled={!manual}
          error={portError}
        />
        <TextInputField
          label={t.t('advanced_config-ProxyUser')}
          value={draft.proxy.user ?? ''}
          placeholder={t.t('advanced_config-ProxyUser_Template')}
          onChange={(v) => update('proxy', { user: v.trim() === '' ? null : v })}
          isDisabled={!manual}
          description={t.t('ng-ui-proxy-password-hint')}
        />
      </section>

      <section className="group">
        <h3 className="group-title">{t.t('ng-ui-privacy-title')}</h3>
        <CheckField
          label={t.t('ng-ui-certcache')}
          description={t.t('ng-ui-certcache-hint')}
          isSelected={draft.certificate_cache.enabled}
          onChange={(enabled) => update('certificate_cache', { enabled })}
        />
      </section>
    </div>
  );
}

function KeystoreTab({ draft, update, view }: TabProps) {
  const { t, bridge, toast } = useApp();
  const ks = draft.keystore;
  const fileError = useProblem(view.problems, 'keystore.file');
  const aliasError = useProblem(view.problems, 'keystore.alias');
  const browse = async () => {
    try {
      const files = await bridge.pickFiles({
        title: t.t('keystore_config-KeystoreFile-Dialog_Title'),
        multiple: false,
        filters: [
          { name: t.t('common-KeystoreExtension_Description'), extensions: ['p12', 'pkcs12', 'pfx', 'ks', 'jks'] },
          { name: t.t('common-PKCS12Extension_Description'), extensions: ['p12', 'pkcs12', 'pfx'] },
          { name: t.t('common-AllExtension_Description'), extensions: ['*'] },
        ],
        defaultPath: ks.file,
      });
      const file = files?.[0];
      if (file) setFile(file);
    } catch {
      toast(t.t('ng-ui-dialog-failed'), 'error');
    }
  };
  // Wie PDF-Over: Der Typ folgt der Dateiendung (.p12/.pfx → PKCS12, .jks/.ks → Java-Keystore).
  const setFile = (file: string) => {
    const derived = keystoreTypeFromFile(file);
    update('keystore', derived ? { file, declared_type: derived } : { file });
  };
  const type = (ks.declared_type ?? 'PKCS12').toUpperCase();
  return (
    <div className="tab-grid">
      <section className="group">
        <h3 className="group-title">{t.t('keystore_config-Keystore_Title')}</h3>
        <TextInputField
          label={t.t('keystore_config-KeystoreFile')}
          value={ks.file ?? ''}
          onChange={(v) => (v.trim() === '' ? update('keystore', { file: null }) : setFile(v))}
          description={t.t('keystore_config-KeystoreFile_ToolTip')}
          error={fileError}
          trailing={<AccessButton msg="common-browse" onPress={() => void browse()} />}
        />
        <SelectField
          label={t.t('keystore_config-KeystoreType')}
          value={type === 'JKS' || type === 'JCEKS' ? type : 'PKCS12'}
          onChange={(v) => update('keystore', { declared_type: v })}
          options={[
            { id: 'PKCS12', label: t.t('keystore_config-KeystoreType_PKCS12') },
            { id: 'JKS', label: `${t.t('keystore_config-KeystoreType_JKS')} (JKS)` },
            { id: 'JCEKS', label: `${t.t('keystore_config-KeystoreType_JKS')} (JCEKS)` },
          ]}
          description={type === 'JKS' || type === 'JCEKS' ? t.t('ng-ui-notice-keystore-conversion') : undefined}
        />
        <TextInputField
          label={t.t('keystore_config-KeystoreAlias')}
          value={ks.alias ?? ''}
          onChange={(v) => update('keystore', { alias: v.trim() === '' ? null : v })}
          description={t.t('ng-ui-keystore-alias-hint')}
          error={aliasError}
        />
      </section>
      <section className="group">
        <h3 className="group-title">{t.t('keystore_config-SaveToWhere-Header')}</h3>
        <RadioField
          label={t.t('keystore_config-SaveToWhere-Header')}
          hideLabel
          value={ks.password_storage}
          onChange={(password_storage) => update('keystore', { password_storage })}
          options={[
            { id: 'none', label: t.t('keystore_config-SaveToWhere-None') },
            { id: 'memory', label: t.t('keystore_config-SaveToWhere-Memory') },
            {
              id: 'keychain',
              label: t.t('ng-ui-keystore-keychain'),
              disabled: !view.keychain_available,
              description: view.keychain_available ? undefined : t.t('ng-secrets-not-persistent'),
            },
          ]}
        />
      </section>
    </div>
  );
}

function Fido2Tab({ draft, update, view }: TabProps) {
  const { t } = useApp();
  return (
    <div className="tab-grid">
      <section className="group">
        <h3 className="group-title">{t.t('config-FIDO2')}</h3>
        <p className="pre-line">
          <Markup text={t.t('config-fido2-AboutFIDO')} targets={[{ href: t.t('config-fido2-InfoURL') }]} />
        </p>
      </section>
      <section className="group">
        <h3 className="group-title">{t.t('config-fido2-Group')}</h3>
        {view.fido2_available ? (
          <CheckField
            label={t.t('config-fido2-ByDefault')}
            isSelected={draft.id_austria.fido2_default}
            onChange={(fido2_default) => update('id_austria', { fido2_default })}
          />
        ) : (
          <p className="message message-warning">{t.t('config-fido2-UnsupportedPlatform')}</p>
        )}
      </section>
    </div>
  );
}

/** Über (docs/analyse/05 §2.6): Version, Lizenz, Datenschutz, Update-Prüfung, Logs. */
function AboutTab({ draft, update }: TabProps) {
  const { t, bridge, toast } = useApp();
  const [info, setInfo] = useState<{ version: string; platform: string } | null>(null);
  useEffect(() => {
    let alive = true;
    bridge
      .appInfo()
      .then((i) => alive && setInfo(i))
      .catch(() => undefined);
    return () => {
      alive = false;
    };
  }, [bridge]);
  return (
    <div className="tab-grid">
      <section className="group">
        <h3 className="group-title">PDF-Over NG</h3>
        <p>{t.t('ng-ui-about-version', { version: info?.version ?? '…' })}</p>
        <p className="pre-line">
          <Markup text={t.t('config-AboutText')} />
        </p>
        <p className="hint">{t.t('ng-ui-about-thirdparty')}</p>
      </section>
      <section className="group">
        <h3 className="group-title">{t.t('config-DataProtection')}</h3>
        <p>
          <Markup text={t.t('config-DataProtectionStatement')} targets={[{ href: t.t('config-DataProtectionURL') }]} />
        </p>
      </section>
      <section className="group">
        <h3 className="group-title">{t.t('advanced_config-UpdateCheck_Title')}</h3>
        <CheckField
          label={t.t('advanced_config-UpdateCheck')}
          description={t.t('advanced_config-UpdateCheck_ToolTip')}
          isSelected={draft.general.update_check}
          onChange={(update_check) => update('general', { update_check })}
        />
      </section>
      <section className="group">
        <div className="actions actions-start">
          <AccessButton msg="config-ShowLogDirectory" onPress={() => bridge.openLogDir().catch(() => toast(t.t('ng-ui-open-failed'), 'error'))} />
        </div>
      </section>
    </div>
  );
}

/** Einstellungen-Overlay. */
export function SettingsDialog({ settingsView }: { settingsView: SettingsView }) {
  const { t, dispatch } = useApp();
  const [draft, setDraft] = useState<Settings>(() => structuredClone(settingsView.settings));
  const [tab, setTab] = useState<Key>('simple');
  const [confirmReset, setConfirmReset] = useState(false);
  const update: Update = (section, patch) => setDraft((d) => withSection(d, section, patch));

  // Arbeitskopie nachführen, wenn der Kern andere Einstellungen meldet, während der Dialog offen ist
  // („Einstellungen zurücksetzen“, Normalisierung beim Speichern) – sonst würde ein späteres „Speichern“
  // den neuen Stand mit der alten Arbeitskopie überschreiben. Eigene Eingaben bleiben erhalten, solange
  // der Kern nur denselben Stand bzw. das Echo der gesendeten Einstellungen liefert.
  const incoming = settingsView.settings;
  const lastIncoming = useRef<string | null>(null);
  const lastSent = useRef<string | null>(null);
  const resetRequested = useRef(false);
  useEffect(() => {
    const json = stableJson(incoming);
    const changed = lastIncoming.current !== null && json !== lastIncoming.current;
    lastIncoming.current = json;
    if (resetRequested.current) {
      resetRequested.current = false;
      setDraft(structuredClone(incoming));
      return;
    }
    if (!changed || json === lastSent.current) return;
    setDraft(structuredClone(incoming));
  }, [incoming]);
  const problemEntries = Object.entries(settingsView.problems);
  const summaryRef = useRef<HTMLDivElement>(null);

  // Keystore-Tab entfernen → nicht ausgewählt lassen
  useEffect(() => {
    if (!draft.keystore.enabled && tab === 'keystore') setTab('advanced');
  }, [draft.keystore.enabled, tab]);

  useEffect(() => {
    if (problemEntries.length > 0) summaryRef.current?.focus();
  }, [settingsView.problems, problemEntries.length]);

  // Auf die Blockvorschau gezogene Bilddatei → Bildmarke + Profil „Nur Bildmarke“ (wie PDF-Over)
  useEffect(() => {
    const onDrop = (e: Event) => {
      const path = (e as CustomEvent<string>).detail;
      if (typeof path === 'string') {
        setDraft((d) => withSection(d, 'signature', { emblem: path, profile: 'BASE_LOGO' }));
      }
    };
    window.addEventListener(EMBLEM_DROP_EVENT, onDrop);
    return () => window.removeEventListener(EMBLEM_DROP_EVENT, onDrop);
  }, []);

  const save = () => {
    const next: Settings = withSection(draft, 'output', { postfix: draft.output.postfix.trim() === '' ? '_signed' : draft.output.postfix });
    lastSent.current = stableJson(next);
    void dispatch({ type: 'save_settings', settings: next });
  };
  const cancel = () => void dispatch({ type: 'close_settings' });

  const tabs: { id: string; msg: string; args?: Record<string, string> }[] = [
    { id: 'simple', msg: 'config-Simple' },
    { id: 'advanced', msg: 'config-Advanced' },
    ...(draft.keystore.enabled ? [{ id: 'keystore', msg: 'config-Keystore' }] : []),
    { id: 'fido2', msg: 'config-FIDO2' },
    { id: 'about', msg: 'config-About', args: { arg1: 'PDF-Over NG' } },
  ];
  const props: TabProps = { draft, update, view: settingsView };

  return (
    <ModalOverlay className="modal-overlay" isOpen isDismissable={false} onOpenChange={(open) => !open && cancel()}>
      <Modal className="modal settings-modal">
        <Dialog className="dialog settings-dialog" data-modal-layer="">
          <div className="settings-head">
            <Heading slot="title" className="dialog-title">
              {t.t('main-configuration')}
            </Heading>
            <Button className="icon-button" aria-label={t.t('ng-ui-close')} onPress={cancel}>
              <IconClose size={18} />
            </Button>
          </div>
          {problemEntries.length > 0 ? (
            <div className="message message-error problems" role="alert" tabIndex={-1} ref={summaryRef}>
              <p>{t.t('error-InvalidSettings')}</p>
              <ul>
                {problemEntries.map(([field, id]) => (
                  <li key={field}>
                    <code>{field}</code>: {t.code(id, {}, 'ng-ui-problem-invalid')}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          <Tabs className="tabs" selectedKey={tab} onSelectionChange={setTab}>
            <TabList className="tab-list" aria-label={t.t('main-configuration')}>
              {tabs.map((x) => {
                const key = t.accessKey(x.msg);
                return (
                  <Tab key={x.id} id={x.id} className="tab">
                    <span data-accesskey={key ?? undefined}>
                      <Mnemonic text={t.t(x.msg, x.args)} accessKey={key} />
                    </span>
                  </Tab>
                );
              })}
            </TabList>
            <TabPanel id="simple" className="tab-panel">
              <SimpleTab {...props} />
            </TabPanel>
            <TabPanel id="advanced" className="tab-panel">
              <AdvancedTab {...props} />
            </TabPanel>
            {draft.keystore.enabled ? (
              <TabPanel id="keystore" className="tab-panel">
                <KeystoreTab {...props} />
              </TabPanel>
            ) : null}
            <TabPanel id="fido2" className="tab-panel">
              <Fido2Tab {...props} />
            </TabPanel>
            <TabPanel id="about" className="tab-panel">
              <AboutTab {...props} />
            </TabPanel>
          </Tabs>
          <div className="dialog-actions settings-actions">
            <AccessButton msg="ng-ui-settings-reset" variant="danger" onPress={() => setConfirmReset(true)} className="push-left" />
            <AccessButton msg="common-Cancel" variant="quiet" onPress={cancel} />
            <AccessButton msg="common-Save" variant="primary" onPress={save} />
          </div>
          {confirmReset ? (
            <ConfirmDialog
              title={t.t('ng-ui-settings-reset')}
              tone="warning"
              onCancel={() => setConfirmReset(false)}
              actions={
                <>
                  <AccessButton msg="common-Cancel" variant="quiet" onPress={() => setConfirmReset(false)} autoFocus />
                  <AccessButton
                    msg="ng-ui-settings-reset-confirm-button"
                    variant="danger"
                    onPress={() => {
                      setConfirmReset(false);
                      resetRequested.current = true;
                      void dispatch({ type: 'reset_settings' });
                    }}
                  />
                </>
              }
            >
              <p>{t.t('ng-ui-settings-reset-confirm')}</p>
            </ConfirmDialog>
          ) : null}
        </Dialog>
      </Modal>
    </ModalOverlay>
  );
}
