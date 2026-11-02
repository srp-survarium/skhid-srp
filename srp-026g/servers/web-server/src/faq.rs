//! `/faq`, the 0.26g site's "what to do if": one line per case, the file or
//! button a player uses. Each fix is what the files of the players' ZIP do
//! (`client-patches/make_client_026g.py`: `Skhid-defender-setup.cmd`,
//! `Skhid-network-setup.cmd`, `VPN.txt`, `Skhid-without-updater.bat`) and what
//! the updater, the exe and the crash reporter say (`client/updater`,
//! `client/bugtrap`, `docs/notes/required-patches.md`).

use crate::Lang;

pub(crate) const PATH: &str = "/faq";

/// Where a log or a crash file goes: the channel and invite the updater's,
/// `Skhid-without-updater.bat`'s and the crash reporter's own words name.
const BUGREPORTS: &str =
    "#bugreports in Discord (<a href=\"https://discord.gg/Pxa4prXfwu\">discord.gg/Pxa4prXfwu</a>)";
const BUGREPORTS_UK: &str =
    "#bugreports у Discord (<a href=\"https://discord.gg/Pxa4prXfwu\">discord.gg/Pxa4prXfwu</a>)";

/// A case: what the player sees, and what to do; English, then Ukrainian.
type Case = [&'static str; 4];

const INSTALL: [Case; 8] = [
    [
        "Windows Defender deleted or blocked a game file.",
        "Run <code>Skhid-defender-setup.cmd</code> (beside <code>Skhid.bat</code>) and answer Yes: it excludes <code>game\\binaries\\x86</code> from Defender's scans and puts back the files it deleted. Defender stays on.",
        "Windows Defender видалив або заблокував файл гри.",
        "Запустіть <code>Skhid-defender-setup.cmd</code> (поруч зі <code>Skhid.bat</code>) і погодьтеся: він виключає <code>game\\binaries\\x86</code> з перевірок Defender і повертає видалені файли. Defender лишається ввімкненим.",
    ],
    [
        "Another antivirus deletes <code>updater_skhid.exe</code>.",
        "Start <code>Skhid-without-updater.bat</code> instead of <code>Skhid.bat</code>: it installs, updates and starts the game without the launcher (Windows 10 1803 or later).",
        "Інший антивірус видаляє <code>updater_skhid.exe</code>.",
        "Запускайте <code>Skhid-without-updater.bat</code> замість <code>Skhid.bat</code>: він встановлює, оновлює й запускає гру без лаунчера (Windows 10 1803 або новіша).",
    ],
    [
        "A download stops or fails (“Quota exceeded”, too slow).",
        "Start <code>Skhid.bat</code> again: it tries every download address in turn, and the game's download goes on from where it stopped.",
        "Завантаження зупиняється або не вдається («Quota exceeded», надто повільно).",
        "Запустіть <code>Skhid.bat</code> знову: він по черзі пробує всі адреси, а завантаження гри продовжується з місця зупинки.",
    ],
    [
        "Still nothing downloads.",
        "Run <code>Skhid-network-setup.cmd</code> (below), then <code>Skhid.bat</code> again. Still failing: send <code>game\\binaries\\x86\\updater_skhid.log</code> to {bugreports}; with <code>Skhid-without-updater.bat</code>, <code>skhid-download.log</code> beside it.",
        "Нічого не завантажується.",
        "Запустіть <code>Skhid-network-setup.cmd</code> (нижче), потім знову <code>Skhid.bat</code>. Не допомогло: надішліть <code>game\\binaries\\x86\\updater_skhid.log</code> у {bugreports}; для <code>Skhid-without-updater.bat</code> — <code>skhid-download.log</code> поруч із ним.",
    ],
    [
        "“Cannot write … Access is denied (error 5)”.",
        "Move the game out of Program Files, or start the launcher as administrator.",
        "«Cannot write … Access is denied (error 5)».",
        "Перенесіть гру з Program Files або запустіть лаунчер від імені адміністратора.",
    ],
    [
        "“Cannot open resources.db.patch_&lt;n&gt; (error 2). Please run the Skhid launcher again.”",
        "Start the game with <code>Skhid.bat</code>, not <code>play_skhid.bat</code>: the launcher downloads the missing patch, then press <b>PLAY</b>.",
        "«Cannot open resources.db.patch_&lt;n&gt; (error 2). Please run the Skhid launcher again.»",
        "Запускайте гру через <code>Skhid.bat</code>, а не <code>play_skhid.bat</code>: лаунчер завантажить відсутній патч, потім натисніть <b>PLAY</b>.",
    ],
    [
        "“You're using an outdated version of the game.” at sign-in.",
        "Close the game and start <code>Skhid.bat</code>: the launcher updates it.",
        "«Ви використовуєте застарілу версію гри.» під час входу.",
        "Закрийте гру й запустіть <code>Skhid.bat</code>: лаунчер її оновить.",
    ],
    [
        "The launcher says “Server maintenance...” and <b>PLAY</b> is off.",
        "The server is being updated. Leave the launcher open: it starts anew and updates once the server is back.",
        "Лаунчер повідомляє про технічні роботи, і <b>PLAY</b> вимкнено.",
        "Сервер оновлюється. Не закривайте лаунчер: коли сервер повернеться, він перезапуститься й оновиться сам.",
    ],
];

const CONNECTION: [Case; 2] = [
    [
        "The launcher says “skhid.link didn't answer”, “refused the connection”, or nothing connects.",
        "Run <code>Skhid-network-setup.cmd</code> (beside <code>Skhid.bat</code>) and answer Yes: it lets the game's three programs through Windows Firewall. The firewall stays on.",
        "Лаунчер пише, що skhid.link не відповідає чи відхилив з'єднання, або ніщо не підключається.",
        "Запустіть <code>Skhid-network-setup.cmd</code> (поруч зі <code>Skhid.bat</code>) і погодьтеся: він дозволяє трьом програмам гри виходити в мережу через брандмауер Windows. Брандмауер лишається ввімкненим.",
    ],
    [
        "Your provider blocks the server (common in Russia), or the game works only through a VPN.",
        "Put <code>launcher_skhid.exe</code>, <code>updater_skhid.exe</code> and <code>survarium_skhid.exe</code> (in <code>game\\binaries\\x86</code>) in your VPN's split tunnelling: <code>VPN.txt</code> beside <code>Skhid.bat</code> shows how for HAPP, v2rayN, NekoRay and AmneziaVPN.",
        "Провайдер блокує сервер (часто в Росії), або гра працює лише через VPN.",
        "Додайте <code>launcher_skhid.exe</code>, <code>updater_skhid.exe</code> і <code>survarium_skhid.exe</code> (у <code>game\\binaries\\x86</code>) до роздільного тунелювання VPN: як це зробити в HAPP, v2rayN, NekoRay і AmneziaVPN, описано у <code>VPN.txt</code> поруч зі <code>Skhid.bat</code>.",
    ],
];

const GAME: [Case; 6] = [
    [
        "The game crashed.",
        "The crash window names an <code>SRP-crash-….zip</code> file and its folder: press <b>Open Discord</b> and send that file to #bugreports, saying what you were doing. Nothing is sent by itself.",
        "Гра впала.",
        "Вікно збою показує файл <code>SRP-crash-….zip</code> і його теку: натисніть <b>Відкрити Discord</b> і надішліть цей файл у #bugreports, написавши, що ви робили. Сам він нікуди не надсилається.",
    ],
    [
        "The crash window says the game is in a folder with non-English letters.",
        "Move the game to a simple path like <code>C:\\Games\\Survarium</code> and start it from there.",
        "Вікно збою каже, що гра в теці з неанглійськими літерами.",
        "Перенесіть гру в простий шлях, наприклад <code>C:\\Games\\Survarium</code>, і запускайте звідти.",
    ],
    [
        "“Survarium cannot save its files” at start.",
        "Allow <code>survarium_skhid.exe</code> through Controlled folder access, as that window says.",
        "«Survarium не може зберігати свої файли» під час запуску.",
        "Дозвольте <code>survarium_skhid.exe</code> у контрольованому доступі до папок, як підказує це вікно.",
    ],
    [
        "“This replay was recorded by a game version that yours cannot play.”",
        "A newer replay: start the game with <code>Skhid.bat</code> to update it. An older one plays differently and cannot be watched in this version.",
        "«Цей повтор записала версія гри, яку ваша гра не може відтворити.»",
        "Новіший повтор: запустіть гру через <code>Skhid.bat</code>, щоб оновити її. Старіший відтворюється інакше, і в цій версії його не переглянути.",
    ],
    [
        "Linux: typing does not work after switching the keyboard layout.",
        "Select the English layout before starting the game (<code>LINUX.txt</code>).",
        "Linux: після зміни розкладки не працює введення тексту.",
        "Перед запуском гри виберіть англійську розкладку (<code>LINUX.txt</code>).",
    ],
    [
        "Nothing works.",
        "Run <code>Skhid-collect-logs.cmd</code> (beside <code>Skhid.bat</code>) and send the zip it puts on your Desktop to {bugreports}, saying what happened.",
        "Нічого не працює.",
        "Запустіть <code>Skhid-collect-logs.cmd</code> (поруч зі <code>Skhid.bat</code>) і надішліть zip, який він покладе на Робочий стіл, у {bugreports}, описавши, що сталося.",
    ],
];

/// The page's body in `lang`: a card per group, a line per case.
pub(crate) fn body(lang: Lang) -> String {
    let group = |title: String, cases: &[Case]| {
        let items: String = cases
            .iter()
            .map(|&[en_case, en_fix, uk_case, uk_fix]| {
                let (case, fix) = match lang {
                    Lang::En => (en_case, en_fix.replace("{bugreports}", BUGREPORTS)),
                    Lang::Uk => (uk_case, uk_fix.replace("{bugreports}", BUGREPORTS_UK)),
                };
                format!("<li><b>{case}</b> {fix}</li>")
            })
            .collect();
        format!("<section class=card><h2>{title}</h2><ul class=rules>{items}</ul></section>")
    };
    format!(
        "<section class=card><h1>{}</h1></section>{}{}{}",
        lang.tr("What to do if…", "Що робити, якщо…"),
        group(
            lang.tr("Installing and updating", "Встановлення й оновлення"),
            &INSTALL
        ),
        group(lang.tr("Connecting", "З'єднання"), &CONNECTION),
        group(lang.tr("In the game", "У грі"), &GAME),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_is_in_both_languages_with_the_file_to_use() {
        for lang in [Lang::En, Lang::Uk] {
            let page = body(lang);
            assert_eq!(page.matches("<li>").count(), 16, "{page}");
            assert!(!page.contains("{bugreports}"), "{page}");
            for file in [
                "Skhid-defender-setup.cmd",
                "Skhid-network-setup.cmd",
                "Skhid-without-updater.bat",
                "VPN.txt",
                "updater_skhid.log",
                "skhid-download.log",
                "Skhid-collect-logs.cmd",
                "href=\"https://discord.gg/Pxa4prXfwu\"",
            ] {
                assert!(page.contains(file), "{lang:?}: {file}");
            }
        }
    }
}
