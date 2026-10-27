# Electronic signatures: types and legal situation

Translation of content/de/02-signaturarten-recht.md. General information, not legal advice.

## Legal basis {#rechtsgrundlagen}

- **eIDAS Regulation:** Regulation (EU) No 910/2014 on electronic identification and trust services. It applies
  directly in all EU member states and governs electronic signatures, seals, timestamps and trust service providers.
- **eIDAS 2.0:** Regulation (EU) 2024/1183 amends the eIDAS Regulation. Its core is the European Digital Identity
  Wallet (EUDI Wallet), which every member state must provide. The wallet is to enable qualified electronic
  signatures, free of charge for non-professional purposes.
- **Austria:** the Signature and Trust Services Act (Signatur- und Vertrauensdienstegesetz, SVG) supplements the
  eIDAS Regulation (e.g. supervision, exceptions to the equivalence with the written form).

## Three types of electronic signature (eIDAS) {#drei-arten-elektronischer-signaturen-eidas}

- **Simple electronic signature:** data in electronic form attached to other data and used by the signatory to sign
  (e.g. a scanned signature or a typed name). Little evidential value.
- **Advanced electronic signature (AdES):** uniquely linked to the signatory, capable of identifying them, created
  using means that they can use under their sole control, and linked to the data in such a way that any subsequent
  change is detectable. Example: a signature with a keystore (PKCS#12) and a non-qualified certificate.
- **Qualified electronic signature (QES):** an advanced signature created by a qualified signature creation device and
  based on a qualified certificate. Example: a signature with ID Austria (via A-Trust) or with a qualified signature
  card.

## Legal effect {#rechtswirkung}

- An electronic signature must not be denied legal effect and admissibility as evidence solely because it is
  electronic or does not meet the requirements for a qualified signature.
- **A qualified electronic signature has the same legal effect as a handwritten signature.**
- A qualified signature based on a qualified certificate issued in one member state is recognised as a qualified
  signature in all other member states.
- In Austria, a qualified signature generally satisfies the requirement of written form (signature). The SVG lists
  exceptions (e.g. certain legal transactions in family and inheritance law that require a notarial form). If in
  doubt, seek legal advice.

## Other trust services {#weitere-vertrauensdienste}

- **Electronic seal:** like a signature, but for legal persons (e.g. authorities, companies). A qualified seal
  carries the presumption of integrity and of the correctness of the origin of the data.
- **Electronic timestamp:** confirms that data existed at a certain point in time. A qualified timestamp carries the
  presumption of the accuracy of date and time.
- **Trust service providers (TSP):** provide trust services; qualified TSPs are supervised and listed in the member
  states' trusted lists. In Austria, A-Trust is a qualified TSP.
- **Qualified signature creation device (QSCD):** hardware or software that protects the signature key (e.g. a
  signature card or a hardware security module at the provider, for remote signing).

## Official signature (Austria) {#amtssignatur-osterreich}

The official signature (Amtssignatur) is an electronic signature or electronic seal of an authority that, under the
Austrian E-Government Act, makes the origin of a document from that authority recognisable. It consists of an emblem
and a note on how its authenticity can be verified. PIDA offers the profile “Official signature” for the signature
block.
