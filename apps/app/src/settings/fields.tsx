/**
 * Formularbausteine der Einstellungen (React Aria Components): Auswahl, Kontrollkästchen, Textfeld,
 * Feldfehler aus `SettingsView::problems`.
 */
import type { ReactNode } from 'react';
import {
  Button,
  Checkbox,
  Input,
  Label,
  ListBox,
  ListBoxItem,
  Popover,
  Radio,
  RadioGroup,
  Select,
  SelectValue,
  Text,
  TextArea,
  TextField,
} from 'react-aria-components';
import { useApp } from '../app/context';
import { Mnemonic } from '../components/ui';

/** Fehlermeldung eines Feldes (`problems[key]` ist eine i18n-ID). */
export function useProblem(problems: Record<string, string>, key: string): string | null {
  const { t } = useApp();
  const id = problems[key];
  return id ? t.code(id, {}, 'ng-ui-problem-invalid') : null;
}

/** Option einer Auswahl. */
export interface Option<T extends string> {
  id: T;
  label: string;
}

/** Auswahlliste (RAC `Select`). */
export function SelectField<T extends string>({
  label,
  accessMsg,
  value,
  options,
  onChange,
  description,
  isDisabled,
}: {
  label: string;
  accessMsg?: string;
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
  description?: string;
  isDisabled?: boolean;
}) {
  const { t } = useApp();
  const key = accessMsg ? t.accessKey(accessMsg) : null;
  return (
    <Select
      className="field select"
      value={value}
      onChange={(k) => {
        if (k != null) onChange(String(k) as T);
      }}
      isDisabled={isDisabled}
    >
      <Label className="field-label" data-accesskey={key ?? undefined}>
        <Mnemonic text={label} accessKey={key} />
      </Label>
      <Button className="select-button">
        <SelectValue className="select-value" />
        <span className="select-caret" aria-hidden="true">
          ▾
        </span>
      </Button>
      {description ? (
        <Text slot="description" className="hint block">
          {description}
        </Text>
      ) : null}
      <Popover className="popover">
        <ListBox className="listbox">
          {options.map((o) => (
            <ListBoxItem key={o.id} id={o.id} className="listbox-item" textValue={o.label}>
              {o.label}
            </ListBoxItem>
          ))}
        </ListBox>
      </Popover>
    </Select>
  );
}

/** Kontrollkästchen mit Beschreibung. */
export function CheckField({
  label,
  isSelected,
  onChange,
  description,
  isDisabled,
}: {
  label: string;
  isSelected: boolean;
  onChange: (v: boolean) => void;
  description?: string;
  isDisabled?: boolean;
}) {
  return (
    <Checkbox className="checkbox" isSelected={isSelected} onChange={onChange} isDisabled={isDisabled}>
      <span className="checkbox-box" aria-hidden="true" />
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

/** Textfeld (ein- oder mehrzeilig) mit Fehler. */
export function TextInputField({
  label,
  value,
  onChange,
  placeholder,
  description,
  error,
  isDisabled,
  multiline,
  trailing,
  type,
}: {
  label: string;
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
  description?: string;
  error?: string | null;
  isDisabled?: boolean;
  multiline?: boolean;
  trailing?: ReactNode;
  type?: 'text' | 'password';
}) {
  return (
    <TextField
      className="field"
      value={value}
      onChange={onChange}
      isDisabled={isDisabled}
      isInvalid={!!error}
      type={type ?? 'text'}
    >
      <Label className="field-label">{label}</Label>
      <div className="field-row">
        {multiline ? (
          <TextArea className="input textarea" placeholder={placeholder} rows={4} />
        ) : (
          <Input className="input" placeholder={placeholder} spellCheck={false} />
        )}
        {trailing}
      </div>
      {description ? (
        <Text slot="description" className="hint block">
          {description}
        </Text>
      ) : null}
      {error ? (
        <Text slot="errorMessage" className="message message-error">
          {error}
        </Text>
      ) : null}
    </TextField>
  );
}

/** Optionsgruppe. */
export function RadioField<T extends string>({
  label,
  value,
  options,
  onChange,
  isDisabled,
  description,
  hideLabel,
}: {
  label: string;
  value: T;
  options: (Option<T> & { disabled?: boolean; description?: string })[];
  onChange: (v: T) => void;
  isDisabled?: boolean;
  description?: string;
  /** Beschriftung nur für Screenreader (wenn eine gleichlautende Gruppenüberschrift sichtbar ist) */
  hideLabel?: boolean;
}) {
  return (
    <RadioGroup className="radio-group" value={value} onChange={(v) => onChange(v as T)} isDisabled={isDisabled}>
      <Label className={hideLabel ? 'visually-hidden' : 'field-label'}>{label}</Label>
      {description ? (
        <Text slot="description" className="hint block">
          {description}
        </Text>
      ) : null}
      {options.map((o) => (
        <Radio key={o.id} value={o.id} className="radio" isDisabled={o.disabled}>
          <span className="radio-mark" aria-hidden="true" />
          <span>
            {o.label}
            {o.description ? <span className="hint block">{o.description}</span> : null}
          </span>
        </Radio>
      ))}
    </RadioGroup>
  );
}
