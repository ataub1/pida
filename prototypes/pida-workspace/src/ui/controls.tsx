/**
 * Kleine Bausteine des Design-Systems auf Basis von React Aria Components. Sie legen nur Klassen und
 * Struktur fest; Aussehen in `styles/components.css`, Tokens in `styles/tokens.css`.
 */
import { createContext, useContext, useId, type ReactNode } from 'react';
import {
  Button as AriaButton,
  Checkbox,
  Disclosure,
  DisclosurePanel,
  Heading,
  FieldError,
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
import { useT } from '../i18n';
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

export function Badge({ tone = 'neutral', children, icon }: { tone?: 'neutral' | 'success' | 'warning' | 'danger' | 'info' | 'accent'; children: ReactNode; icon?: ReactNode }) {
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
  spellCheck?: 'true' | 'false';
}) {
  const { placeholder, ...fieldProps } = rest;
  return (
    <TextField className={`field${className ? ` ${className}` : ''}`} isInvalid={!!error} {...fieldProps}>
      <Label className="field-label">{label}</Label>
      <Input className={`input${inputClassName ? ` ${inputClassName}` : ''}`} placeholder={placeholder} />
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

export function CheckboxField({ label, isSelected, onChange }: { label: string; isSelected: boolean; onChange: (v: boolean) => void }) {
  return (
    <Checkbox className="checkbox" isSelected={isSelected} onChange={onChange}>
      <span className="checkbox-box" aria-hidden="true">
        <IconCheck size={14} />
      </span>
      <span>{label}</span>
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
  value: V;
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

/** Plattform (Tastenbeschriftung). */
export const PlatformContext = createContext<{ isMac: boolean }>({ isMac: false });

export function useModKey(): { mod: string; isMac: boolean; enter: string; shift: string } {
  const { isMac } = useContext(PlatformContext);
  const t = useT();
  const de = t.lang === 'de';
  return {
    mod: isMac ? '⌘' : de ? 'Strg' : 'Ctrl',
    isMac,
    enter: isMac ? '↵' : de ? 'Eingabe' : 'Enter',
    shift: isMac ? '⇧' : de ? 'Umschalt' : 'Shift',
  };
}

/** Aufklappbare Details (z. B. technische Fehlerdetails, Zertifikat). */
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
