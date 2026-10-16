/**
 * Feature-lokale Bausteine der Einstellungen (ADR 0004 §6.1: UI-B ändert `ui/**` nicht). Gleiche Klassen und
 * Struktur wie die Bausteine des Design-Systems (`ui/controls.tsx`), ergänzt um das, was dort fehlt:
 *
 * - `ChoiceCards`/`ChoiceSegments`: ganze Gruppe sperrbar (`isDisabled`, z. B. Platzhalterart bei
 *   ausgeschalteter Suche oder per Richtlinie) und Beschreibung der Gruppe – im Prototyp hat `RadioCards`
 *   dafür `isDisabled` (Commit fd7def3); zur Übernahme in `ui/controls.tsx` gemeldet.
 * - `SettingTextField`: Textfeld mit `onFocus`/`onBlur` (entprelltes Senden, ADR 0004 §3.11), mehrzeilig,
 *   mit Knopf am Ende.
 * - `LogoSizeSlider` (aus dem bisherigen Dialog), `AccountCard`, `PathRow` (aus dem Prototyp).
 * - `SettingSelect`: Auswahlliste (React Aria `Select`) mit zweizeiligen Einträgen im Stil der Menüs
 *   (`select-button`, `popover`, `menu-item`), z. B. die Schlüssel eines Keystores.
 */
import { useId, type ReactNode } from 'react';
import {
  Button as AriaButton,
  FieldError,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
  Slider,
  SliderOutput,
  SliderThumb,
  SliderTrack,
  Text,
  TextArea,
  TextField,
} from 'react-aria-components';
import type { ChoiceOption } from '../../ui/controls';
import { IconChevronDown } from '../../ui/Icons';

/** Auswahl als Karten; `isDisabled` sperrt die ganze Gruppe. */
export function ChoiceCards<V extends string>({
  label,
  value,
  onChange,
  options,
  columns = 1,
  isDisabled,
  description,
}: {
  label: string;
  value: V | null;
  onChange: (v: V) => void;
  options: ChoiceOption<V>[];
  columns?: number;
  isDisabled?: boolean;
  description?: string | null;
}) {
  return (
    <RadioGroup className="radio-cards-group" value={value} onChange={(v) => onChange(v as V)} isDisabled={isDisabled}>
      <Label className="field-label">{label}</Label>
      <div className="radio-cards" style={{ ['--cols' as string]: columns }}>
        {options.map((o) => (
          <Radio key={o.value} value={o.value} className="radio-card" isDisabled={o.isDisabled}>
            {o.icon ? <span className="radio-card-icon">{o.icon}</span> : null}
            <span className="radio-card-text">
              <span className="radio-card-label">{o.label}</span>
              {o.description ? <span className="radio-card-desc">{o.description}</span> : null}
            </span>
            <span className="radio-card-dot" aria-hidden="true" />
          </Radio>
        ))}
      </div>
      {description ? (
        <Text slot="description" className="field-hint">
          {description}
        </Text>
      ) : null}
    </RadioGroup>
  );
}

/** Segmentierte Auswahl (waagrecht); `isDisabled` sperrt die ganze Gruppe. */
export function ChoiceSegments<V extends string>({
  label,
  value,
  onChange,
  options,
  isDisabled,
  description,
}: {
  label: string;
  value: V;
  onChange: (v: V) => void;
  options: ChoiceOption<V>[];
  isDisabled?: boolean;
  description?: string | null;
}) {
  return (
    <RadioGroup className="segmented-group segmented-md" orientation="horizontal" value={value} onChange={(v) => onChange(v as V)} isDisabled={isDisabled}>
      <Label className="field-label">{label}</Label>
      <div className="segmented">
        {options.map((o) => (
          <Radio key={o.value} value={o.value} className="segment" isDisabled={o.isDisabled}>
            {o.icon}
            <span>{o.label}</span>
          </Radio>
        ))}
      </div>
      {description ? (
        <Text slot="description" className="field-hint">
          {description}
        </Text>
      ) : null}
    </RadioGroup>
  );
}

/** Textfeld einer Einstellung (Struktur wie `ui/controls.tsx` `TextInput`). */
export function SettingTextField({
  label,
  value,
  onChange,
  onFocus,
  onBlur,
  description,
  error,
  multiline,
  placeholder,
  inputMode,
  autoComplete,
  autoCapitalize,
  isDisabled,
  mono,
  trailing,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  onFocus?: () => void;
  onBlur?: () => void;
  description?: ReactNode;
  error?: string | null;
  multiline?: boolean;
  placeholder?: string;
  inputMode?: 'text' | 'numeric' | 'tel';
  autoComplete?: string;
  /** Bildschirmtastatur: `none` für Kennungen (kein Großbuchstabe am Anfang) */
  autoCapitalize?: 'none' | 'sentences';
  isDisabled?: boolean;
  mono?: boolean;
  trailing?: ReactNode;
}) {
  return (
    <TextField className="field" value={value} onChange={onChange} onFocus={onFocus} onBlur={onBlur} isInvalid={!!error} isDisabled={isDisabled} autoComplete={autoComplete}>
      <Label className="field-label">{label}</Label>
      <div className="setting-input-row">
        {multiline ? (
          <TextArea className="input textarea" rows={4} placeholder={placeholder} />
        ) : (
          <Input className={`input${mono ? ' mono' : ''}`} placeholder={placeholder} spellCheck={false} inputMode={inputMode} autoCapitalize={autoCapitalize} />
        )}
        {trailing}
      </div>
      {description && !error ? (
        <Text slot="description" className="field-hint">
          {description}
        </Text>
      ) : null}
      <div className="field-alert" role="alert">
        <FieldError className="field-error">{error}</FieldError>
      </div>
    </TextField>
  );
}

/** Regler der Logo-Größe „Nur Bildmarke“ (PDF-Over: 1–20 cm; gespeichert in mm). */
export const LOGO_MIN_CM = 1;
export const LOGO_MAX_CM = 20;

/**
 * Logo-Größe in cm für den Regler. Der Regler arbeitet in cm, damit Screenreader den richtigen Wert hören
 * („2,3 cm“, nicht „23“); gespeichert wird in mm.
 */
export function logoCm(mm: number): number {
  const cm = Math.round(mm) / 10;
  return Number.isFinite(cm) ? Math.min(LOGO_MAX_CM, Math.max(LOGO_MIN_CM, cm)) : 2.3;
}

export function LogoSizeSlider({ label, valueCm, onChange, onChangeEnd, isDisabled }: { label: string; valueCm: number; onChange: (cm: number) => void; onChangeEnd: (cm: number) => void; isDisabled?: boolean }) {
  const first = (v: number | number[]) => (Array.isArray(v) ? (v[0] ?? 2.3) : v);
  return (
    <Slider
      className="setting-slider"
      minValue={LOGO_MIN_CM}
      maxValue={LOGO_MAX_CM}
      step={0.1}
      value={valueCm}
      isDisabled={isDisabled}
      onChange={(v) => onChange(first(v))}
      onChangeEnd={(v) => onChangeEnd(first(v))}
      formatOptions={{ style: 'unit', unit: 'centimeter', minimumFractionDigits: 1, maximumFractionDigits: 1 }}
    >
      <div className="setting-slider-head">
        <Label className="field-label">{label}</Label>
        <SliderOutput className="setting-slider-output" />
      </div>
      <SliderTrack className="setting-slider-track">
        {({ state }) => (
          <>
            <div className="setting-slider-fill" style={{ width: `${state.getThumbPercent(0) * 100}%` }} />
            <SliderThumb className="setting-slider-thumb" aria-label={label} />
          </>
        )}
      </SliderTrack>
      <div className="setting-slider-scale" aria-hidden="true">
        <span>{LOGO_MIN_CM} cm</span>
        <span>{LOGO_MAX_CM} cm</span>
      </div>
    </Slider>
  );
}

/** Karte eines Kontos bzw. Signiermittels (Prototyp `AccountCard`). */
export function AccountCard({ icon, title, children, badge }: { icon: ReactNode; title: string; children: ReactNode; badge?: ReactNode }) {
  const id = useId();
  return (
    <section className="account-card" aria-labelledby={id}>
      <div className="account-head">
        <span className="account-icon" aria-hidden="true">
          {icon}
        </span>
        <h4 className="account-title" id={id}>
          {title}
        </h4>
        {badge ? <span className="account-badge">{badge}</span> : null}
      </div>
      <div className="account-body">{children}</div>
    </section>
  );
}

/** Zeile „Beschriftung · Pfad · Knopf“ (Prototyp `path-row`). */
export function PathRow({ label, value, empty, action }: { label: string; value: string | null; empty: string; action?: ReactNode }) {
  return (
    <div className="path-row">
      <span className="path-label">{label}</span>
      <span className={`path-value${value ? ' mono' : ' is-empty'}`}>{value ?? empty}</span>
      {action}
    </div>
  );
}

/** Eintrag einer `SettingSelect`. */
export interface SelectOption {
  id: string;
  label: string;
  /** zweite, kleinere Zeile (z. B. Alias) */
  detail?: string;
  isDisabled?: boolean;
}

/** Auswahlliste einer Einstellung (React Aria `Select`); `value = null` zeigt den Platzhalter. */
export function SettingSelect({
  label,
  value,
  onChange,
  options,
  placeholder,
  description,
  error,
  isDisabled,
}: {
  label: string;
  value: string | null;
  onChange: (id: string) => void;
  options: SelectOption[];
  placeholder?: string;
  description?: ReactNode;
  error?: string | null;
  isDisabled?: boolean;
}) {
  return (
    <Select
      className="field setting-select"
      value={value}
      onChange={(key) => {
        if (key !== null) onChange(String(key));
      }}
      placeholder={placeholder}
      isInvalid={!!error}
      isDisabled={isDisabled}
    >
      <Label className="field-label">{label}</Label>
      <AriaButton className="select-button">
        <SelectValue className="select-value">{({ isPlaceholder, selectedText, defaultChildren }) => (isPlaceholder ? defaultChildren : selectedText)}</SelectValue>
        <IconChevronDown size={16} aria-hidden="true" />
      </AriaButton>
      {description && !error ? (
        <Text slot="description" className="field-hint">
          {description}
        </Text>
      ) : null}
      <div className="field-alert" role="alert">
        <FieldError className="field-error">{error}</FieldError>
      </div>
      <Popover className="popover" placement="bottom start" offset={4}>
        <ListBox className="menu" items={options}>
          {(o) => (
            <ListBoxItem id={o.id} textValue={o.label} className="menu-item" isDisabled={o.isDisabled}>
              <span className="menu-item-text">
                <span className="menu-item-label">{o.label}</span>
                {o.detail ? <span className="menu-item-desc mono-small">{o.detail}</span> : null}
              </span>
            </ListBoxItem>
          )}
        </ListBox>
      </Popover>
    </Select>
  );
}
