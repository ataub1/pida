/**
 * Bausteine des Design-Systems auf Basis von React Aria Components (aus dem Prototyp, ADR 0004 §4.2). Sie
 * legen nur Klassen und Struktur fest; Aussehen in `styles/components.css`, Tokens in `styles/tokens.css`.
 *
 * Angepasst für die App: Texte kommen aus den Fluent-Katalogen (`useApp().t`), Knöpfe mit Text aus dem
 * Katalog tragen dessen Zugriffstaste (`AccessButton`, `&`-Mnemonics aus PDF-Over, `data-accesskey`;
 * ausgelöst in `ui/AccessKeys.ts`), Tastenbeschriftungen folgen der Plattform (`useModKey`).
 */
import { useEffect, useId, useRef, type ReactNode } from 'react';
import {
  Button as AriaButton,
  Checkbox,
  Disclosure,
  DisclosurePanel,
  FieldError,
  Heading,
  Input,
  Label,
  ProgressBar,
  Radio,
  RadioGroup,
  Switch,
  Text,
  TextArea,
  TextField,
  Tooltip,
  TooltipTrigger,
  type ButtonProps as AriaButtonProps,
} from 'react-aria-components';
import { useApp } from '../app/context';
import type { Args } from '../i18n/catalog';
import { IconCheck, IconChevronRight } from './Icons';

export type Variant = 'primary' | 'secondary' | 'ghost' | 'quiet' | 'danger';
export type Size = 'sm' | 'md' | 'lg';

export function Button({
  variant = 'secondary',
  size = 'md',
  className,
  children,
  ...rest
}: Omit<AriaButtonProps, 'className' | 'children'> & { variant?: Variant; size?: Size; className?: string; children: ReactNode }) {
  return (
    <AriaButton className={`btn btn-${variant} btn-${size}${className ? ` ${className}` : ''}`} {...rest}>
      {children}
    </AriaButton>
  );
}

/** Symbolknopf mit sichtbarem Tooltip; `label` ist der zugängliche Name. */
export function IconButton({
  label,
  children,
  className,
  tooltip = true,
  variant = 'ghost',
  size = 'md',
  ...rest
}: Omit<AriaButtonProps, 'className' | 'children' | 'aria-label'> & {
  label: string;
  children: ReactNode;
  className?: string;
  tooltip?: boolean;
  variant?: Variant;
  size?: Size;
}) {
  const button = (
    <AriaButton aria-label={label} className={`icon-btn icon-btn-${variant} icon-btn-${size}${className ? ` ${className}` : ''}`} {...rest}>
      {children}
    </AriaButton>
  );
  if (!tooltip) return button;
  return (
    <TooltipTrigger delay={500} closeDelay={80}>
      {button}
      <Tooltip className="tooltip" offset={6}>
        {label}
      </Tooltip>
    </TooltipTrigger>
  );
}

export function Kbd({ children }: { children: ReactNode }) {
  return <kbd className="kbd">{children}</kbd>;
}

/**
 * Tastenkürzel als Folge von Tasten, z. B. `['⌘', 'K']`. Die Tastenkappen sind dekorativ; `label`
 * liefert bei Bedarf eine Textfassung für Screenreader (z. B. „Strg+K“).
 */
export function Keys({ keys, label }: { keys: string[]; label?: string; decorative?: boolean }) {
  return (
    <>
      <span className="keys" aria-hidden="true">
        {keys.map((k, i) => (
          <Kbd key={i}>{k}</Kbd>
        ))}
      </span>
      {label ? <span className="sr-only">{label}</span> : null}
    </>
  );
}

export type Tone = 'neutral' | 'success' | 'warning' | 'danger' | 'info' | 'accent';

export function Badge({ tone = 'neutral', children, icon }: { tone?: Tone; children: ReactNode; icon?: ReactNode }) {
  return (
    <span className={`badge badge-${tone}`}>
      {icon}
      {children}
    </span>
  );
}

export function Spinner({ size = 20, className }: { size?: number; className?: string }) {
  return (
    <svg className={`spinner${className ? ` ${className}` : ''}`} viewBox="0 0 24 24" width={size} height={size} aria-hidden="true" focusable="false">
      <circle cx="12" cy="12" r="9" fill="none" stroke="currentColor" strokeOpacity="0.18" strokeWidth="2.5" />
      <path d="M21 12a9 9 0 0 0-9-9" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" />
    </svg>
  );
}

export function Progress({ label, value, valueLabel, className }: { label: string; value: number; valueLabel?: string; className?: string }) {
  return (
    <ProgressBar className={`progress${className ? ` ${className}` : ''}`} value={value} valueLabel={valueLabel}>
      {({ percentage, valueText }) => (
        <>
          <div className="progress-head">
            <Label className="progress-label">{label}</Label>
            <span className="progress-value">{valueText}</span>
          </div>
          <div className="progress-track">
            <div className="progress-fill" style={{ width: `${percentage ?? 0}%` }} />
          </div>
        </>
      )}
    </ProgressBar>
  );
}

export function TextInput({
  label,
  description,
  error,
  className,
  inputClassName,
  placeholder,
  spellCheck,
  ...rest
}: {
  label: string;
  description?: ReactNode;
  error?: string | null;
  className?: string;
  inputClassName?: string;
  value?: string;
  defaultValue?: string;
  onChange?: (v: string) => void;
  type?: 'text' | 'password' | 'search' | 'tel' | 'email';
  autoFocus?: boolean;
  autoComplete?: string;
  inputMode?: 'text' | 'numeric' | 'tel' | 'search' | 'email' | 'decimal';
  name?: string;
  isDisabled?: boolean;
  isReadOnly?: boolean;
  maxLength?: number;
  placeholder?: string;
  spellCheck?: boolean;
}) {
  return (
    <TextField className={`field${className ? ` ${className}` : ''}`} isInvalid={!!error} {...rest}>
      <Label className="field-label">{label}</Label>
      <Input className={`input${inputClassName ? ` ${inputClassName}` : ''}`} placeholder={placeholder} spellCheck={spellCheck} />
      {description && !error ? (
        <Text slot="description" className="field-hint">
          {description}
        </Text>
      ) : null}
      {/* Dauerhafte Live-Region: Fehler nach „Absenden“ werden angesagt, auch wenn der Fokus auf dem
          Knopf bleibt (WCAG 4.1.3). Die Fehlermeldung bleibt zugleich per aria-describedby am Feld. */}
      <div className="field-alert" role="alert">
        <FieldError className="field-error">{error}</FieldError>
      </div>
    </TextField>
  );
}

export function TextAreaInput({
  label,
  description,
  value,
  onChange,
  rows = 4,
}: {
  label: string;
  description?: ReactNode;
  value: string;
  onChange: (v: string) => void;
  rows?: number;
}) {
  return (
    <TextField className="field" value={value} onChange={onChange}>
      <Label className="field-label">{label}</Label>
      <TextArea className="input textarea" rows={rows} />
      {description ? (
        <Text slot="description" className="field-hint">
          {description}
        </Text>
      ) : null}
    </TextField>
  );
}

export function SwitchField({
  label,
  hint,
  isSelected,
  onChange,
  isDisabled,
}: {
  label: string;
  hint?: string;
  isSelected: boolean;
  onChange: (v: boolean) => void;
  isDisabled?: boolean;
}) {
  const hintId = useId();
  return (
    <div className={`switch-field${isDisabled ? ' is-disabled' : ''}`}>
      <Switch className="switch" isSelected={isSelected} onChange={onChange} isDisabled={isDisabled} aria-describedby={hint ? hintId : undefined}>
        <span className="switch-label">{label}</span>
        <span className="switch-track" aria-hidden="true">
          <span className="switch-thumb" />
        </span>
      </Switch>
      {hint ? (
        <p className="switch-hint" id={hintId}>
          {hint}
        </p>
      ) : null}
    </div>
  );
}

export function CheckboxField({
  label,
  description,
  isSelected,
  onChange,
  isDisabled,
}: {
  label: string;
  description?: string;
  isSelected: boolean;
  onChange: (v: boolean) => void;
  isDisabled?: boolean;
}) {
  return (
    <Checkbox className="checkbox" isSelected={isSelected} onChange={onChange} isDisabled={isDisabled}>
      <span className="checkbox-box" aria-hidden="true">
        <IconCheck size={14} />
      </span>
      <span>
        {label}
        {description ? (
          <Text slot="description" className="hint block">
            {description}
          </Text>
        ) : null}
      </span>
    </Checkbox>
  );
}

export interface ChoiceOption<V extends string> {
  value: V;
  label: string;
  description?: string;
  icon?: ReactNode;
  isDisabled?: boolean;
}

/** Auswahl als Karten (Radio-Gruppe). */
export function RadioCards<V extends string>({
  label,
  value,
  onChange,
  options,
  columns = 1,
  hideLabel,
}: {
  label: string;
  value: V | null;
  onChange: (v: V) => void;
  options: ChoiceOption<V>[];
  columns?: number;
  hideLabel?: boolean;
}) {
  return (
    <RadioGroup className="radio-cards-group" value={value} onChange={(v) => onChange(v as V)} aria-label={hideLabel ? label : undefined}>
      {hideLabel ? null : <Label className="field-label">{label}</Label>}
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
    </RadioGroup>
  );
}

/** Segmentierte Auswahl (Radio-Gruppe, waagrecht). */
export function Segmented<V extends string>({
  label,
  value,
  onChange,
  options,
  hideLabel,
  size = 'md',
}: {
  label: string;
  value: V;
  onChange: (v: V) => void;
  options: ChoiceOption<V>[];
  hideLabel?: boolean;
  size?: Size;
}) {
  return (
    <RadioGroup className={`segmented-group segmented-${size}`} orientation="horizontal" value={value} onChange={(v) => onChange(v as V)} aria-label={hideLabel ? label : undefined}>
      {hideLabel ? null : <Label className="field-label">{label}</Label>}
      <div className="segmented">
        {options.map((o) => (
          <Radio key={o.value} value={o.value} className="segment" isDisabled={o.isDisabled} aria-label={o.icon && !o.label ? o.value : undefined}>
            {o.icon}
            {o.label ? <span>{o.label}</span> : null}
          </Radio>
        ))}
      </div>
    </RadioGroup>
  );
}

/** Tastenbeschriftung der Plattform (⌘ bzw. Strg/Ctrl). */
export function useModKey(): { mod: string; isMac: boolean; enter: string; shift: string } {
  const { isMac, t } = useApp();
  const de = t.lang === 'de';
  return {
    mod: isMac ? '⌘' : de ? 'Strg' : 'Ctrl',
    isMac,
    enter: isMac ? '↵' : de ? 'Eingabe' : 'Enter',
    shift: isMac ? '⇧' : de ? 'Umschalt' : 'Shift',
  };
}

/** Aufklappbare Details (z. B. technische Fehlerdetails). */
export function Details({ label, children, level = 3, defaultExpanded }: { label: string; children: ReactNode; level?: number; defaultExpanded?: boolean }) {
  return (
    <Disclosure className="details" defaultExpanded={defaultExpanded}>
      <Heading level={level} className="details-heading">
        <AriaButton slot="trigger" className="details-trigger">
          <IconChevronRight size={16} className="details-caret" />
          {label}
        </AriaButton>
      </Heading>
      <DisclosurePanel className="details-panel">{children}</DisclosurePanel>
    </Disclosure>
  );
}

/* ---------------------------------------------------------------- Zugriffstasten (aus der bisherigen App) */

/** Tastenkürzel einer Zugriffstaste für `aria-keyshortcuts`. */
export function keyShortcut(key: string, isMac: boolean): string {
  return `${isMac ? 'Control+Alt' : 'Alt'}+${key.toUpperCase()}`;
}

/** Beschriftung mit Zugriffstaste (erstes passendes Zeichen; unterstrichen nur bei gedrückter Alt-Taste). */
export function Mnemonic({ text, accessKey }: { text: string; accessKey: string | null }) {
  if (!accessKey) return <>{text}</>;
  const i = text.toLowerCase().indexOf(accessKey.toLowerCase());
  if (i < 0) return <>{text}</>;
  return (
    <>
      {text.slice(0, i)}
      <span className="mnemonic">{text.slice(i, i + 1)}</span>
      {text.slice(i + 1)}
    </>
  );
}

/** Varianten der Knöpfe der bisherigen App (`secondary` = Rahmen, `quiet` = ohne Rahmen). */
export type ButtonVariant = Variant;

/** Knopf mit Text aus dem Katalog und dessen Zugriffstaste (`.accesskey`). */
export function AccessButton({
  msg,
  args,
  label,
  icon,
  variant = 'secondary',
  size = 'md',
  className,
  children,
  ...rest
}: Omit<AriaButtonProps, 'children' | 'className'> & {
  msg: string;
  args?: Args;
  label?: string;
  icon?: ReactNode;
  variant?: ButtonVariant;
  size?: Size;
  className?: string;
  /** zusätzlicher Inhalt hinter der Beschriftung (z. B. Tastenkürzel) */
  children?: ReactNode;
}) {
  const { t, isMac } = useApp();
  const text = label ?? t.t(msg, args);
  const key = t.accessKey(msg);
  const ref = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (key) el.setAttribute('aria-keyshortcuts', keyShortcut(key, isMac));
    else el.removeAttribute('aria-keyshortcuts');
  }, [key, isMac]);
  return (
    <AriaButton ref={ref} className={`btn btn-${variant} btn-${size}${className ? ` ${className}` : ''}`} data-accesskey={key ?? undefined} {...rest}>
      {icon}
      <span className="btn-label">
        <Mnemonic text={text} accessKey={key} />
      </span>
      {children}
    </AriaButton>
  );
}

/** Kopf eines Schritts der bisherigen Oberfläche (h1, fokussierbar; Rückfall-Ansichten). */
export function ScreenHeading({ children, subtitle }: { children: ReactNode; subtitle?: ReactNode }) {
  return (
    <header className="screen-header">
      <h1 id="screen-heading" tabIndex={-1} className="screen-title">
        {children}
      </h1>
      {subtitle ? <p className="screen-subtitle">{subtitle}</p> : null}
    </header>
  );
}
