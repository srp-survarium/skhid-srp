"""Inspect real Win32 crash dialogs under Wine; no game or real browser launch.

Usage: python3 tests/check_localization.py BUILD [SCREENSHOT_DIRECTORY]
Build must contain ui_probe.exe and BugTrap.dll. Use an isolated WINEPREFIX.
"""
from pathlib import Path
import argparse
import os
import re
import shutil
import subprocess
import tempfile
import zipfile

# Independent expected UI strings, including Cyrillic and Polish diacritics.
LANGUAGES = {
    "english": ("0409", "The game has crashed.", "Open Discord", "Close", "File:", "Folder:", "Windows error", "Could not open your browser.", "Copy", "Copied"),
    "russian": ("0419", "В игре произошёл сбой.", "Открыть Discord", "Закрыть", "Файл:", "Папка:", "Ошибка Windows", "Не удалось открыть браузер.", "Копировать", "Скопировано"),
    "polish": ("0415", "Gra uległa awarii.", "Otwórz Discord", "Zamknij", "Plik:", "Folder:", "Błąd systemu Windows", "Nie udało się otworzyć przeglądarki.", "Kopiuj", "Skopiowano"),
    "ukrainian": ("0422", "У грі стався збій.", "Відкрити Discord", "Закрити", "Файл:", "Папка:", "Помилка Windows", "Не вдалося відкрити браузер.", "Копіювати", "Скопійовано"),
    "german": ("0807", "Das Spiel ist abgestürzt.", "Discord öffnen", "Schließen", "Datei:", "Ordner:", "Windows-Fehler", "Der Browser konnte nicht geöffnet werden.", "Kopieren", "Kopiert"),
}


# The line for a game folder with non-ASCII letters, as it starts.
WARNING = {
    "english": "The game is in a folder with non-English letters (",
    "russian": "Игра находится в папке с нелатинскими буквами (",
    "polish": "Gra jest w folderze z nieangielskimi literami (",
    "ukrainian": "Гра в папці з нелатинськими літерами (",
    "german": "Das Spiel liegt in einem Ordner mit nicht-englischen Buchstaben (",
}


def check_details(controls, clip):
    """The short details in the dialog and the code block "Copy" copies.
    The probe's synthetic crash is an access violation where BugTrap.dll
    captured its context (Wine reports the caller, in ui_probe.exe)."""
    details = controls[1040]
    assert details.startswith("0xC0000005 ACCESS_VIOLATION (read of 0x00000000)"), details
    assert re.search(r" at (BugTrap\.dll|ui_probe\.exe)\+0x[0-9A-F]+$", details), details
    assert clip.startswith("```|") and clip.endswith("```"), clip
    for line in ("Exception: 0xC0000005 ACCESS_VIOLATION", "|Address:   ",
                 "|Thread:    ", "|Registers: EAX=", "|Game:      ui_probe.exe, patch revision unknown, sha256 ",
                 "|Reporter:  ", "|Windows:   ", "|Dump:      "):
        assert line in clip, (line, clip)
    assert "Game log:" not in clip, clip


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("build", type=Path)
    parser.add_argument("screenshots", type=Path, nargs="?")
    parser.add_argument("--states", nargs="+", default=["success", "raw", "partial", "failure", "blocked", "unicode", "browser-failure", "dialog-failure"])
    options = parser.parse_args()
    build = options.build.resolve()
    screenshots = options.screenshots.resolve() if options.screenshots else None
    if screenshots:
        screenshots.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="srp-ui-test-") as temporary:
        base = Path(temporary)
        # "unicode": the game in a folder of Cyrillic and Polish letters.
        unicode = base / "Игра-Żółw"
        unicode.mkdir()
        for folder in (base, unicode):
            for filename in ("BugTrap.dll", "ui_probe.exe"):
                shutil.copy2(build / filename, folder / filename)
        cases = [(language, state, LANGUAGES[language][0], ["-localization=english"])
                 for language in LANGUAGES
                 for state in options.states]
        # No game option: Windows' UI language, including German regional variant.
        cases += [(language, "success", text[0], []) for language, text in LANGUAGES.items()]
        cases += [("english", "success", "040c", []),
                  ("english", "success", "0409", ["-localization=russian"])]
        for language, state, windows, args in cases:
            root = unicode if state == "unicode" else base
            before = set(root.glob("SRP-crash-*"))
            env = os.environ.copy()
            if screenshots and state in ("success", "blocked", "failure", "unicode") and args:
                env["SRP_UI_SCREENSHOT"] = "Z:" + str(screenshots / f"{language}-{state}.bmp")
            result = subprocess.run(["wine", str(root / "ui_probe.exe"), state, windows, *args],
                                    cwd=root, env=env, capture_output=True, timeout=45)
            output = result.stdout.decode("utf-8")
            assert result.returncode == 0 and "PASS localized" in output, (
                language, state, result.returncode, output, result.stderr.decode(errors="replace"))
            controls = {int(line.split(" ", 2)[1]): line.split(" ", 2)[2]
                        for line in output.splitlines() if line.startswith("TEXT ")}
            clips = [line[5:] for line in output.splitlines() if line.startswith("CLIP ")]
            _, intro, button, close, file_label, folder_label, error, browser_error, copy, copied = LANGUAGES[language]
            new = set(root.glob("SRP-crash-*")) - before
            texts = [p for p in new if p.suffix == ".txt"]
            if state == "dialog-failure":
                [report] = new - set(texts)
                assert report.name in controls[-1] and report.suffix == ".zip", controls
                assert "discord.gg/QUmJg3gBhg" in controls[-1] and "#bugreports" in controls[-1], controls
                assert re.search(r"ACCESS_VIOLATION.*\s+at \S+\.(dll|exe)\+0x", controls[-1]), controls
                assert button not in controls[-1], "fallback must not refer to a missing button"
                for p in new:
                    p.unlink()
                print(f"PASS {language}: dialog creation failure", flush=True)
                continue
            assert controls[1030] == intro and controls[1013] == button and controls[2] == close, controls
            assert controls[1041] == copy and controls[-2] == copied, controls
            if state == "unicode":
                assert controls[1042].startswith(WARNING[language]) and "Игра-Żółw" in controls[1042], controls
                assert "C:\\Games\\Survarium" in controls[1042], controls
                assert "|Paths:     non-ASCII letters in the game folder" in clips[0], clips
            else:
                assert 1042 not in controls and "Paths:" not in clips[0], (controls, clips)
            assert "#bugreports" in controls[1036] and button in controls[1036] and copy in controls[1036], controls
            [clip] = clips
            check_details(controls, clip)
            if state in ("partial", "failure", "blocked"):
                assert error in controls[1031], controls
            if state == "failure":
                # Neither a dump nor a text report could be written: shown only.
                assert not new and all(i not in controls for i in (1034, 1035, 1037, 1038)), controls
                assert "|Dump:      not saved, the file could not be written, error 5" in clip, clip
                assert "#bugreports" not in controls[1031], controls
            elif state == "blocked":
                # The capture was refused: the antivirus message and the text report.
                assert "0x8007001F" in controls[1031] and "#bugreports" in controls[1031], controls
                [report] = new
                assert report.suffix == ".txt" and controls[1034] == report.name, controls
                text = report.read_bytes().decode("utf-8")
                assert text.startswith("Skhid crash report") and "ACCESS_VIOLATION" in text, text
                assert "Dump:      not saved, the capture failed, error 0x8007001F\r\n" in text, text
                report.unlink()
            else:
                assert controls[1037] == file_label and controls[1038] == folder_label, controls
                [text] = texts
                [report] = new - {text}
                assert controls[1034] == report.name and str(root).replace("/", "\\") in controls[1035], controls
                assert text.stem == report.stem, (text, report)
                content = text.read_bytes()
                assert content.startswith(b"Skhid crash report") and b"\r\nDump:      saved" in content
                if state in ("raw", "partial"):
                    assert report.suffix == ".dmp" and report.read_bytes()[:4] == b"MDMP"
                else:
                    assert report.suffix == ".zip"
                    with zipfile.ZipFile(report) as archive:
                        assert archive.testzip() is None
                        assert archive.read("crash.txt") == content
                report.unlink()
                text.unlink()
            if state == "browser-failure":
                assert controls[-1].startswith(browser_error), controls
                assert "discord.gg/QUmJg3gBhg" in controls[-1] and "#bugreports" in controls[-1]
            print(f"PASS {language}: {state}, {'automatic despite game option' if args else 'automatic without game option'}", flush=True)


if __name__ == "__main__":
    main()
