/* Patch 83: a DirectInput step that fails at the start stops the game with a
 * message instead of a crash (input_devices_026g.asm has the three sites;
 * client_patch.md section 3.59).
 *
 * The raw result of a step (its HRESULT and the interface or device it wrote)
 * is read once into an input_outcome: ready, or failed with the step, the
 * HRESULT and the advice for it. A failure shows one message box (Russian,
 * then English; the HRESULT in hex, so a report names the cause) and ends the
 * process as the game's own quit does (TerminateProcess of itself: no crash
 * report, no destructors of a half-made input world). */
#include "engine_026g.h"

typedef unsigned int u32;
typedef unsigned short u16;

/* input_devices_026g.asm's STEP_ values. */
enum input_step { STEP_DIRECT_INPUT, STEP_KEYBOARD, STEP_MOUSE };
enum { STEPS = 3 };

/* What the player is told to do. */
enum advice {
    REPAIR_DIRECTX,     /* Windows' DirectInput is damaged or unregistered */
    RESTART,            /* anything else: the device is busy or in a bad state */
};
enum { ADVICES = 2 };

struct input_failure { enum input_step step; u32 hresult; enum advice advice; };
enum input_outcome_kind { INPUT_READY, INPUT_FAILED };
struct input_outcome {
    enum input_outcome_kind kind;
    union { struct input_failure failed; } as;
};

static enum advice advice_of(u32 hresult)
{
    switch (hresult) {
    case 0x80040154u:   /* REGDB_E_CLASSNOTREG, DIERR_DEVICENOTREG */
    case 0x80040111u:   /* CLASS_E_CLASSNOTAVAILABLE */
    case 0x80004002u:   /* E_NOINTERFACE */
    case 0x8007047Eu:   /* DIERR_OLDDIRECTINPUTVERSION */
    case 0x80070481u:   /* DIERR_BETADIRECTINPUTVERSION */
        return REPAIR_DIRECTX;
    default:
        return RESTART;
    }
}

/* The boundary: a step's raw HRESULT and the pointer it wrote. FAILED(hr),
 * or a success with no interface (Windows' DLL answered nonsense), fails. */
static struct input_outcome outcome_of(enum input_step step, u32 hresult, const void *made)
{
    if ((hresult & 0x80000000u) == 0 && made != 0)
        return (struct input_outcome){ INPUT_READY, { .failed = { 0, 0, 0 } } };
    enum advice advice = made == 0 && (hresult & 0x80000000u) == 0 ? REPAIR_DIRECTX : advice_of(hresult);
    return (struct input_outcome){ INPUT_FAILED, { .failed = { step, hresult, advice } } };
}

struct language {
    const u16 *step[STEPS];     /* the first line, up to the HRESULT */
    const u16 *after;           /* after the HRESULT */
    const u16 *advice[ADVICES];
    const u16 *report;
};

static const struct language languages[] = {
    {
        { u"Survarium не может запуститься: Windows не запустила DirectInput (ошибка 0x",
          u"Survarium не может запуститься: DirectInput не открыл клавиатуру (ошибка 0x",
          u"Survarium не может запуститься: DirectInput не открыл мышь (ошибка 0x" },
        u").\r\n",
        { u"1. Запустите командную строку от имени администратора и выполните: sfc /scannow\r\n"
          u"2. Установите DirectX End-User Runtime с сайта Microsoft.\r\n"
          u"3. Перезагрузите компьютер и запустите игру снова.\r\n",
          u"1. Перезагрузите компьютер.\r\n"
          u"2. Проверьте в Диспетчере задач, что не запущен другой survarium_skhid.exe.\r\n"
          u"3. Отключите геймпады и необычные устройства ввода и запустите игру снова.\r\n" },
        u"Не помогло: пришлите этот текст (Ctrl+C) в Discord: discord.gg/Pxa4prXfwu\r\n",
    },
    {
        { u"Survarium cannot start: Windows could not start DirectInput (error 0x",
          u"Survarium cannot start: DirectInput could not open the keyboard (error 0x",
          u"Survarium cannot start: DirectInput could not open the mouse (error 0x" },
        u").\r\n",
        { u"1. Open Command Prompt as administrator and run: sfc /scannow\r\n"
          u"2. Install the DirectX End-User Runtime from Microsoft.\r\n"
          u"3. Restart the PC and start the game again.\r\n",
          u"1. Restart the PC.\r\n"
          u"2. In Task Manager, make sure no other survarium_skhid.exe is running.\r\n"
          u"3. Unplug game controllers and unusual input devices, then start the game again.\r\n" },
        u"Still failing? Send this text (Ctrl+C copies it) to discord.gg/Pxa4prXfwu",
    },
};
enum { LANGUAGES = sizeof languages / sizeof languages[0] };

/* Room for both languages' text (about 900 characters) and its end. */
enum { TEXT = 1536 };
struct text { u16 at[TEXT]; u32 size; };

static void put(struct text *text, const u16 *s)
{
    while (*s && text->size + 1 < TEXT)
        text->at[text->size++] = *s++;
    text->at[text->size] = 0;
}

static void put_hex(struct text *text, u32 value)
{
    u16 digits[9];
    for (u32 i = 0; i < 8; i++) {
        u32 nibble = (value >> (28 - 4 * i)) & 0xF;
        digits[i] = (u16)(nibble < 10 ? u'0' + nibble : u'A' + nibble - 10);
    }
    digits[8] = 0;
    put(text, digits);
}

static _Noreturn void stop(const struct input_failure *failure, void *window)
{
    struct text text;
    text.size = 0;
    for (u32 i = 0; i < LANGUAGES; i++) {
        const struct language *l = &languages[i];
        if (i)
            put(&text, u"\r\n");
        put(&text, l->step[failure->step]);
        put_hex(&text, failure->hresult);
        put(&text, l->after);
        put(&text, l->advice[failure->advice]);
        put(&text, l->report);
    }
    /* MB_OK | MB_ICONERROR | MB_SETFOREGROUND | MB_TOPMOST, owned by the
     * game's window (disabled while the box is up). */
    engine_message_box_w(window, text.at, u"Survarium", 0x10u | 0x10000u | 0x40000u);
    for (;;)
        engine_terminate_process(engine_get_current_process(), 1);
}

/* input_devices_026g.asm's entries: the step, its raw HRESULT, the pointer it
 * wrote, the game's window. Returns when the step succeeded. */
void input_devices_check(enum input_step step, u32 hresult, const void *made, void *window)
{
    struct input_outcome outcome = outcome_of(step, hresult, made);
    switch (outcome.kind) {
    case INPUT_READY:
        return;
    case INPUT_FAILED:
        stop(&outcome.as.failed, window);
    }
}
