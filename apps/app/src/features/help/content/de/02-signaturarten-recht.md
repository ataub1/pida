# Elektronische Signaturen: Arten und Rechtslage

Allgemeine Information, keine Rechtsberatung.

## Rechtsgrundlagen

- **eIDAS-Verordnung:** Verordnung (EU) Nr. 910/2014 über elektronische Identifizierung und Vertrauensdienste.
  Sie gilt unmittelbar in allen EU-Mitgliedstaaten und regelt elektronische Signaturen, Siegel, Zeitstempel und
  Vertrauensdiensteanbieter.
- **eIDAS 2.0:** Verordnung (EU) 2024/1183 ändert die eIDAS-Verordnung. Kern ist die europäische Brieftasche für die
  digitale Identität (EUDI-Wallet), die jeder Mitgliedstaat bereitstellen muss. Die Wallet soll qualifizierte
  elektronische Signaturen ermöglichen, für nicht-berufliche Zwecke kostenlos.
- **Österreich:** Das Signatur- und Vertrauensdienstegesetz (SVG) ergänzt die eIDAS-Verordnung (z. B. Aufsicht,
  Ausnahmen von der Gleichstellung mit der Schriftform).

## Drei Arten elektronischer Signaturen (eIDAS)

- **Einfache elektronische Signatur:** Daten in elektronischer Form, die anderen Daten beigefügt sind und mit denen
  die unterzeichnende Person unterzeichnet (z. B. ein eingescannter Namenszug oder ein getippter Name). Kaum
  Beweiskraft.
- **Fortgeschrittene elektronische Signatur (AdES):** eindeutig der unterzeichnenden Person zugeordnet, ermöglicht
  ihre Identifizierung, mit Mitteln erstellt, die sie unter ihrer alleinigen Kontrolle verwenden kann, und so mit den
  Daten verbunden, dass eine nachträgliche Veränderung erkannt werden kann. Beispiel: Signatur mit einem Keystore
  (PKCS#12) und einem nicht-qualifizierten Zertifikat.
- **Qualifizierte elektronische Signatur (QES):** eine fortgeschrittene Signatur, die mit einer qualifizierten
  Signaturerstellungseinheit erstellt wurde und auf einem qualifizierten Zertifikat beruht. Beispiel: Signatur mit
  ID Austria (über A-Trust) oder mit einer qualifizierten Signaturkarte.

## Rechtswirkung

- Einer elektronischen Signatur darf die Rechtswirkung und die Zulässigkeit als Beweismittel nicht allein deshalb
  abgesprochen werden, weil sie elektronisch ist oder nicht die Anforderungen an eine qualifizierte Signatur erfüllt.
- **Eine qualifizierte elektronische Signatur hat die gleiche Rechtswirkung wie eine handschriftliche Unterschrift.**
- Eine qualifizierte Signatur auf Basis eines qualifizierten Zertifikats aus einem Mitgliedstaat wird in allen
  anderen Mitgliedstaaten als qualifizierte Signatur anerkannt.
- In Österreich erfüllt die qualifizierte Signatur grundsätzlich das Erfordernis der Schriftlichkeit (Unterschrift).
  Das SVG nennt Ausnahmen (z. B. bestimmte Rechtsgeschäfte des Familien- und Erbrechts, für die eine notarielle Form
  verlangt ist). Im Zweifel: rechtliche Beratung einholen.

## Weitere Vertrauensdienste

- **Elektronisches Siegel:** wie eine Signatur, aber für juristische Personen (z. B. Behörden, Unternehmen). Ein
  qualifiziertes Siegel begründet die Vermutung der Unversehrtheit und der Richtigkeit der Herkunft der Daten.
- **Elektronischer Zeitstempel:** bestätigt, dass Daten zu einem bestimmten Zeitpunkt vorlagen. Ein qualifizierter
  Zeitstempel begründet die Vermutung der Richtigkeit von Datum und Uhrzeit.
- **Vertrauensdiensteanbieter (VDA):** erbringen Vertrauensdienste; qualifizierte VDA werden beaufsichtigt und in den
  Vertrauenslisten (Trusted Lists) der Mitgliedstaaten geführt. In Österreich ist A-Trust ein qualifizierter VDA.
- **Qualifizierte Signaturerstellungseinheit (QSCD):** Hard- oder Software, die den Signaturschlüssel schützt (z. B.
  Signaturkarte oder ein Hardware-Sicherheitsmodul beim Anbieter, bei der Fernsignatur).

## Amtssignatur (Österreich)

Die Amtssignatur ist eine elektronische Signatur oder ein elektronisches Siegel einer Behörde, die nach dem
E-Government-Gesetz die Herkunft eines Dokuments von der Behörde erkennbar macht. Sie besteht aus einer Bildmarke und
einem Hinweis, wie die Echtheit geprüft werden kann. PIDA bietet das Profil „Amtssignatur“ für das Signaturbild an.
