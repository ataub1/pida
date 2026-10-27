/** „Was bedeutet das?“ an Fehlermeldungen: öffnet die Offline-Hilfe beim Artikel zum Code (nur wenn es den Artikel gibt). */
import { Button as AriaButton } from 'react-aria-components';
import { useApp } from '../../app/context';
import { IconHelp } from '../../ui/Icons';
import { errorId, hasErrorArticle } from './codes';
import { HELP_AVAILABLE } from './index';

export function HelpLink({ code }: { code: string }) {
  const { t, openHelp } = useApp();
  if (!HELP_AVAILABLE || !hasErrorArticle(t, code)) return null;
  return (
    <AriaButton className="link help-what" onPress={() => openHelp(`fehler:${errorId(code)}`)}>
      <IconHelp size={15} />
      {t.t('ng-ui-shell-help-what')}
    </AriaButton>
  );
}
