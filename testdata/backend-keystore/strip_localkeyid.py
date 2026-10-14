#!/usr/bin/env python3
"""NUR TEST – PDF-Over NG, Paket B (Review-Zusatz).

Entfernt die localKeyID-Attribute aus einem *unverschlüsselten* PKCS#12 (``openssl pkcs12 -export
-keypbe NONE -certpbe NONE``) und berechnet den MAC (HMAC-SHA-256, PKCS#12-KDF nach RFC 7292 B.2,
2048 Iterationen, fester Salt) neu. So entstehen Dateien, die Java (``PKCS12KeyStore``, also PDF-Over)
über den friendlyName bzw. die Ein-Schlüssel-Regel lädt, ``p12-keystore`` im Modus ``Strict`` aber nicht
verknüpft.

Aufruf: strip_localkeyid.py <ein.p12> <aus.p12> <passwort>
Benötigt asn1crypto (z. B. aus der .venv mit pyHanko). Keine Netzwerkzugriffe.
"""
import hashlib
import hmac
import sys

from asn1crypto import algos, cms, core, pkcs12


def pkcs12_kdf(password: str, salt: bytes, id_byte: int, iterations: int, n: int) -> bytes:
    """PKCS#12-Schlüsselableitung (RFC 7292 Anhang B.2) mit SHA-256."""
    u, v = 32, 64
    d = bytes([id_byte]) * v
    pw = password.encode("utf-16-be") + b"\x00\x00"

    def fill(data: bytes) -> bytes:
        if not data:
            return b""
        length = v * ((len(data) + v - 1) // v)
        return (data * (length // len(data) + 1))[:length]

    i = bytearray(fill(salt) + fill(pw))
    out = b""
    while len(out) < n:
        a = d + bytes(i)
        for _ in range(iterations):
            a = hashlib.sha256(a).digest()
        out += a
        b = int.from_bytes((a * (v // u + 1))[:v], "big")
        for j in range(0, len(i), v):
            block = (int.from_bytes(i[j : j + v], "big") + b + 1) % (1 << (8 * v))
            i[j : j + v] = block.to_bytes(v, "big")
    return out[:n]


def strip(src: str, dst: str, password: str) -> None:
    pfx = pkcs12.Pfx.load(open(src, "rb").read())
    auth_safe = pfx["auth_safe"]
    assert auth_safe["content_type"].native == "data", "authSafe muss data sein"
    safes = pkcs12.AuthenticatedSafe.load(auth_safe["content"].native)
    new_safes = []
    for ci in safes:
        assert ci["content_type"].native == "data", "nur unverschlüsselte SafeContents (-keypbe/-certpbe NONE)"
        bags = pkcs12.SafeContents.load(ci["content"].native)
        new_bags = []
        for bag in bags:
            attrs = [a for a in bag["bag_attributes"] if a["type"].native != "local_key_id"]
            new_bag = pkcs12.SafeBag()
            new_bag["bag_id"] = bag["bag_id"].dotted
            new_bag["bag_value"] = pkcs12.SafeBag.load(bag.dump())["bag_value"]
            if attrs:
                new_bag["bag_attributes"] = pkcs12.Attributes(attrs)
            new_bags.append(new_bag)
        new_safes.append(cms.ContentInfo({"content_type": "data", "content": pkcs12.SafeContents(new_bags).dump()}))
    content = pkcs12.AuthenticatedSafe(new_safes).dump()
    salt = hashlib.sha256(b"PDF-Over NG NUR TEST " + dst.encode()).digest()[:8]
    key = pkcs12_kdf(password, salt, 3, 2048, 32)
    mac = hmac.new(key, content, hashlib.sha256).digest()
    out = pkcs12.Pfx(
        {
            "version": "v3",
            "auth_safe": {"content_type": "data", "content": content},
            "mac_data": {
                "mac": {"digest_algorithm": algos.DigestAlgorithm({"algorithm": "sha256", "parameters": core.Null()}), "digest": mac},
                "mac_salt": salt,
                "iterations": 2048,
            },
        }
    )
    open(dst, "wb").write(out.dump(force=True))


if __name__ == "__main__":
    if len(sys.argv) != 4:
        sys.exit(__doc__)
    strip(sys.argv[1], sys.argv[2], sys.argv[3])
