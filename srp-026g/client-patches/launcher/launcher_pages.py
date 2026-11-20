"""The pages the Skhid launcher shows from its own folder
(`game/binaries/x86/launcher_skhid/`), in the place of the game's
`game/launcher/index-*.html`:

    index-blank-<code>.html   while the server's page is being fetched
    index-<code>.html         when it could not be fetched

for the launcher's five languages (en, ru, ua, de, pl), each in its own
language (the server's page has English and Ukrainian). When the server's
page cannot be loaded, the page says what to try: the firewall
(`Skhid-network-setup.cmd`, beside `Skhid.bat`) and, in a download with a
relay, the launcher's own "Relay" row at its bottom left (launcher patch
16), for a provider that blocks the server. They are made for the launcher's embedded browser (Internet Explorer's
engine, 899 by 335 pixels): fixed sizes, old CSS, no script, and embedded
original background artwork. The server and SRP Discord links open in the
player's browser.

    python launcher_pages.py <folder> [--site=https://skhid.link] [--relay]
        writes the ten files into <folder> (to look at them); --relay: as
        in a download with a relay
"""

import base64
import html
from pathlib import Path
import os
import sys

CODES = ("en", "ru", "ua", "de", "pl")

# One skin for the web server and both local states. Each image is below
# IE8's 32 KiB data-URI limit; no network or relative path is needed.
HERE = Path(__file__).resolve().parent
SKIN = HERE.parents[1] / "servers" / "web-server" / "launcher-skin"
DISCORD_LINK = (SKIN / "discord-link.html").read_text(encoding="utf-8").strip()
STYLE = (SKIN / "style.css").read_text(encoding="utf-8")
for slot, name, mime in (("BACKGROUND", "back.jpg", "image/jpeg"),
                         ("SHADOW", "shadow.png", "image/png")):
    image = base64.b64encode((SKIN / name).read_bytes()).decode("ascii")
    STYLE = STYLE.replace(f"@{slot}@", f"data:{mime};base64,{image}")

TEXT = {
    "en": {
        "lang": "en",
        "title": "Skhid &middot; Survarium <i>0.26g</i> community server",
        "loading": "Loading the server's page&hellip;",
        "offline": "The server's page could not be loaded.",
        "hint": "If the line below shows an error: allow the game in your firewall (Skhid-network-setup.cmd).",
        "relay": "If your provider blocks the server: set Relay, at the bottom left, to auto.",
        "page": "The server's page",
        "legal_title": "Legal notice",
        "legal": "Unofficial community server. Not affiliated with or endorsed by Vostok Games. "
                 "Survarium is a trademark of its owners.",
        "launcher": "Launcher",
        "about": "This launcher keeps the Skhid files of the game up to date and starts the game "
                 "in the language chosen above.",
    },
    "ua": {
        "lang": "uk",
        "title": "Схід &middot; ком'юніті сервер Survarium <i>0.26g</i>",
        "loading": "Завантаження сторінки сервера&hellip;",
        "offline": "Не вдалося завантажити сторінку сервера.",
        "hint": "Якщо в рядку нижче помилка: дозвольте гру в брандмауері (Skhid-network-setup.cmd).",
        "relay": "Якщо провайдер блокує сервер: поставте «Релей» ліворуч унизу на авто.",
        "page": "Сторінка сервера",
        "legal_title": "Правова інформація",
        "legal": "Неофіційний ком'юніті сервер. Не пов'язаний із Vostok Games і не схвалений ними. "
                 "Survarium є торговельною маркою її власників.",
        "launcher": "Лаунчер",
        "about": "Цей лаунчер оновлює файли Схід для гри і запускає гру мовою, обраною вгорі.",
    },
    "ru": {
        "lang": "ru",
        "title": "Skhid &middot; комьюнити-сервер Survarium <i>0.26g</i>",
        "loading": "Загрузка страницы сервера&hellip;",
        "offline": "Не удалось загрузить страницу сервера.",
        "hint": "Если в строке ниже ошибка: разрешите игру в брандмауэре (Skhid-network-setup.cmd).",
        "relay": "Если провайдер блокирует сервер: поставьте «Релей» слева внизу на авто.",
        "page": "Страница сервера",
        "legal_title": "Правовая информация",
        "legal": "Неофициальный комьюнити-сервер. Не связан с Vostok Games и не одобрен ими. "
                 "Survarium является торговой маркой её владельцев.",
        "launcher": "Лаунчер",
        "about": "Этот лаунчер обновляет файлы Skhid для игры и запускает игру на языке, выбранном вверху.",
    },
    "de": {
        "lang": "de",
        "title": "Skhid &middot; Community-Server für Survarium <i>0.26g</i>",
        "loading": "Die Seite des Servers wird geladen&hellip;",
        "offline": "Die Seite des Servers konnte nicht geladen werden.",
        "hint": "Zeigt die Zeile unten einen Fehler: Erlaube das Spiel in deiner Firewall (Skhid-network-setup.cmd).",
        "relay": "Sperrt dein Anbieter den Server: Stelle „Relay“ unten links auf Auto.",
        "page": "Die Seite des Servers",
        "legal_title": "Rechtlicher Hinweis",
        "legal": "Inoffizieller Community-Server. Nicht mit Vostok Games verbunden oder von ihnen unterstützt. "
                 "Survarium ist eine Marke ihrer Inhaber.",
        "launcher": "Launcher",
        "about": "Dieser Launcher hält die Skhid-Dateien des Spiels aktuell und startet das Spiel in der oben "
                 "gewählten Sprache.",
    },
    "pl": {
        "lang": "pl",
        "title": "Skhid &middot; serwer społeczności Survarium <i>0.26g</i>",
        "loading": "Ładowanie strony serwera&hellip;",
        "offline": "Nie udało się załadować strony serwera.",
        "hint": "Jeśli w wierszu poniżej jest błąd: zezwól na grę w zaporze (Skhid-network-setup.cmd).",
        "relay": "Jeśli dostawca blokuje serwer: ustaw „Relay” na dole po lewej na auto.",
        "page": "Strona serwera",
        "legal_title": "Informacja prawna",
        "legal": "Nieoficjalny serwer społeczności. Niezwiązany z Vostok Games ani przez nie niepopierany. "
                 "Survarium jest znakiem towarowym jego właścicieli.",
        "launcher": "Launcher",
        "about": "Ten launcher aktualizuje pliki Skhid gry i uruchamia grę w języku wybranym powyżej.",
    },
}


def page(code, site, blank, relay=False):
    text = TEXT[code]
    host = site.split("//", 1)[1].split("/", 1)[0]
    if blank:
        left = f"<p class=big>{text['loading']}</p>"
    else:
        # The "Relay" row is there only in a download with a relay.
        left = f"<p>{text['offline']}</p><p>{text['hint']}</p>" + (f"<p>{text['relay']}</p>" if relay else "")
    link = f'<a href="{html.escape(site, quote=True)}/" target="_blank">{html.escape(host)}</a>'
    return (
        "<!DOCTYPE html><html lang={lang}><head>"
        '<meta http-equiv="X-UA-Compatible" content="IE=edge">'
        '<meta http-equiv="content-type" content="text/html; charset=UTF-8">'
        '<title>Skhid</title><style type="text/css">{style}</style></head><body>'
        "<div id=container>"
        "<div id=left><h1>{title}</h1>{left}</div>"
        "<div id=right>"
        "<div class=\"tile first\"><p>{link}</p><div class=cap>{page}</div></div>"
        "<div class=\"tile second\"><p class=small>{about}</p><div class=cap>{launcher}</div></div>"
        "<div class=\"tile third\"><p class=small>{legal}</p><div class=cap>{discord}{legal_title}</div></div>"
        "</div></div></body></html>\r\n"
    ).format(style=STYLE, left=left, link=link, discord=DISCORD_LINK, **{k: v for k, v in text.items()
                                                   if k not in ("loading", "offline", "hint", "relay")}).encode("utf-8")


def pages(site, relay=False):
    """{file name: bytes} of the ten pages for the server page `site`;
    `relay`: of a download with a relay."""
    out = {}
    for code in CODES:
        out[f"index-blank-{code}.html"] = page(code, site, True)
        out[f"index-{code}.html"] = page(code, site, False, relay)
    return out


if __name__ == "__main__":
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    site = next((a[7:] for a in sys.argv[1:] if a.startswith("--site=")), "https://skhid.link")
    if len(args) != 1:
        sys.exit(__doc__)
    os.makedirs(args[0], exist_ok=True)
    for name, data in pages(site.rstrip("/"), "--relay" in sys.argv[1:]).items():
        with open(os.path.join(args[0], name), "wb") as out:
            out.write(data)
        print(f"wrote {os.path.join(args[0], name)} ({len(data)} bytes)")
