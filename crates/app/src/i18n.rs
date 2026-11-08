//! UI strings. One typed table per language: a missing translation is a compile error.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Ru,
    En,
}

impl Lang {
    pub const ALL: [Self; 2] = [Self::Ru, Self::En];

    /// Russian for a Russian Windows UI, English otherwise.
    pub fn from_windows_ui(primary_lang_id: u16) -> Self {
        const LANG_RUSSIAN: u16 = 0x19;
        if primary_lang_id & 0x3FF == LANG_RUSSIAN { Self::Ru } else { Self::En }
    }

    pub fn strings(self) -> &'static Strings {
        match self {
            Self::Ru => &RU,
            Self::En => &EN,
        }
    }
}

#[derive(Debug)]
pub struct Strings {
    pub connecting: &'static str,
    pub connecting_hint: &'static str,
    pub no_access: &'static str,
    pub no_access_title: &'static str,
    pub no_access_hint: &'static str,
    pub model_fallback: &'static str,
    pub model_unknown: &'static str,
    pub model_unknown_info: &'static str,
    pub footer: &'static str,
    pub read_failed: &'static str,

    pub telemetry: &'static str,
    pub telemetry_info: &'static str,
    pub seconds_ago: fn(u64) -> String,

    pub mode: &'static str,
    pub mode_info: &'static str,
    pub scenarios: [&'static str; 4],
    pub scenario_hints: [&'static str; 4],
    pub mode_unknown: &'static str,

    pub fans: &'static str,
    pub fans_info: &'static str,
    pub fan_readout: fn(u8, Option<u8>) -> String,
    pub fan_auto: &'static str,
    pub fan_custom: &'static str,
    pub unsaved: &'static str,
    pub reset: &'static str,
    pub apply: &'static str,
    pub cooler_boost_hint: &'static str,

    pub battery: &'static str,
    pub battery_info: &'static str,
    pub battery_readout: fn(u8) -> String,

    pub system: &'static str,
    pub backlight: &'static str,
    pub backlight_off: &'static str,
    pub backlight_info: &'static str,
    pub webcam: &'static str,
    pub webcam_info: &'static str,
    pub win_fn: &'static str,
    pub win_fn_info: &'static str,
    pub usb_share: &'static str,
    pub usb_share_info: &'static str,
    pub language: &'static str,

    pub app: &'static str,
    pub autostart: &'static str,
    pub autostart_info: &'static str,
    pub close_to_tray: &'static str,
    pub close_to_tray_info: &'static str,
    pub tray_open: &'static str,
    pub tray_exit: &'static str,
    pub tray_tooltip: fn(Option<u8>, Option<u8>) -> String,
    pub open_app: &'static str,

    pub restore: &'static str,
    pub restore_info: &'static str,
    pub per_source: &'static str,
    pub per_source_info: &'static str,
    pub profile_ac: &'static str,
    pub profile_battery: &'static str,

    pub touchpad: &'static str,
    pub touchpad_info: &'static str,
    pub fn_lock: &'static str,
    pub fn_lock_info: &'static str,
    pub on: &'static str,
    pub off: &'static str,

    pub ranges: [&'static str; 3],
    pub export_csv: &'static str,
    pub exported: fn(&str) -> String,

    pub check_updates: &'static str,
    pub check_updates_info: &'static str,
    pub update_available: fn(&str) -> String,

    pub err_scenario: &'static str,
    pub err_cooler_boost: &'static str,
    pub err_backlight: &'static str,
    pub err_webcam: &'static str,
    pub err_win_fn: &'static str,
    pub err_usb: &'static str,
    pub err_curve: &'static str,
    pub err_auto_fan: &'static str,
    pub err_battery: &'static str,
    pub err_autostart: &'static str,
    pub err_restore: &'static str,
    pub err_touchpad: &'static str,
    pub err_export: &'static str,
}

pub static RU: Strings = Strings {
    connecting: "Подключение к EC",
    connecting_hint: "Читаю MSI_ACPI…",
    no_access: "Нет доступа",
    no_access_title: "Нет доступа к прошивке ноутбука",
    no_access_hint: "Нужен ноутбук MSI с интерфейсом WMI 2.x и запуск от имени администратора.",
    model_fallback: "Ноутбук MSI",
    model_unknown: "Модель не распознана",
    model_unknown_info: "Этой прошивки EC нет в таблице моделей msi-ec, поэтому используются \
                         регистры Cyborg 15 A12VF (msi-ec CONF_G2_1). Часть настроек может не \
                         работать — запустите msi-probe и создайте issue о новой модели.",
    footer: "MSI Center Service может вернуть свой сценарий после перезагрузки и сна — включите \
             «Восстанавливать настройки».",
    read_failed: "Не удалось прочитать датчики",

    telemetry: "Телеметрия",
    telemetry_info: "Температура CPU и GPU за выбранный период (до 24 часов), данные обновляются раз \
                     в 1,5 с. Выше 90 °C линия краснеет. Ниже — обороты вентиляторов и их текущая \
                     мощность. «Экспорт CSV» сохраняет всю историю в таблицу.",
    seconds_ago: |s| match s {
        0..90 => format!("{s} с назад"),
        90..5400 => format!("{} мин назад", s / 60),
        _ => format!("{} ч {} мин назад", s / 3600, s % 3600 / 60),
    },

    mode: "Режим",
    mode_info: "Сценарии MSI Center. Меняют профиль мощности в прошивке ноутбука: \
                насколько высоко процессор и видеокарта могут поднимать частоты и потребление, \
                и как охотно разгоняются вентиляторы.",
    scenarios: ["Экстрим", "Баланс", "Тихий", "Батарея"],
    scenario_hints: [
        "Максимальные лимиты мощности. Быстрее всего, но горячее и громче.",
        "Обычные лимиты мощности — для повседневной работы.",
        "Обычные лимиты, но вентиляторы работают тише.",
        "Сниженные лимиты мощности для дольшей работы от батареи.",
    ],
    mode_unknown: "Прошивка в режиме, которого нет среди сценариев",

    fans: "Вентиляторы",
    fans_info: "Скорость вентилятора для каждого диапазона температур. Перетаскивайте точки вверх \
                и вниз, затем нажмите «Применить». Пороги температур и правую серую точку задаёт \
                прошивка; кривая не может подняться выше неё. «Авто» возвращает заводскую кривую.",
    fan_readout: |duty, temp| match temp {
        Some(t) => format!("сейчас {duty}% · {t}°"),
        None => format!("сейчас {duty}% · датчик спит"),
    },
    fan_auto: "Авто",
    fan_custom: "Своя кривая",
    unsaved: "Изменения не применены",
    reset: "Сброс",
    apply: "Применить",
    cooler_boost_hint: "Максимальные обороты поверх кривой",

    battery: "Батарея",
    battery_info: "Ноутбук перестанет заряжаться на выбранном уровне. Если он почти всегда \
                   подключён к сети, 60–80 % заметно продлевают жизнь батареи.",
    battery_readout: |v| if v >= 100 { "до полного".to_owned() } else { format!("до {v}%") },

    system: "Система",
    backlight: "Подсветка",
    backlight_off: "Выкл",
    backlight_info: "Яркость подсветки клавиатуры.",
    webcam: "Веб-камера",
    webcam_info: "Аппаратно отключает камеру: Windows и приложения перестают её видеть.",
    win_fn: "Поменять Win и Fn",
    win_fn_info: "Меняет местами клавиши Win и Fn — тот же переключатель, что в MSI Center.",
    usb_share: "USB-зарядка при выключении",
    usb_share_info: "Порты USB продолжают давать питание, когда ноутбук выключен или спит, \
                     — можно заряжать телефон.",
    language: "Язык",

    app: "Приложение",
    autostart: "Запускать вместе с Windows",
    autostart_info: "Приложение стартует свёрнутым в трей при входе в Windows. Используется \
                     Планировщик заданий: так оно запускается с правами администратора без \
                     запроса UAC.",
    close_to_tray: "Сворачивать в трей при закрытии",
    close_to_tray_info: "Крестик прячет окно в трей, датчики продолжают обновляться. \
                         Выйти — через меню значка в трее.",
    tray_open: "Открыть",
    tray_exit: "Выход",
    tray_tooltip: |cpu, gpu| format!("Open MSI Control\nCPU {} · GPU {}", temp(cpu), temp(gpu)),
    open_app: "Открыть приложение",

    restore: "Восстанавливать настройки",
    restore_info: "После включения, выхода из сна и смены питания приложение возвращает ваш \
                   режим, кривую вентиляторов, Cooler Boost и лимит заряда — даже если служба \
                   MSI Center их перезаписала.",
    per_source: "Отдельный профиль от батареи",
    per_source_info: "От сети и от батареи запоминаются разные режим, вентиляторы и Cooler Boost. \
                      При смене питания профиль переключается сам. Изменения применяются к \
                      профилю текущего источника.",
    profile_ac: "профиль: от сети",
    profile_battery: "профиль: от батареи",

    touchpad: "Тачпад",
    touchpad_info: "Включает и выключает сенсорную панель — как Ctrl+Win+F24 или клавиша на \
                    клавиатуре.",
    fn_lock: "Fn Lock",
    fn_lock_info: "Показывает, зафиксирована ли клавиша Fn. Переключается сочетанием Fn+Esc — \
                   программно прошивка его не меняет.",
    on: "вкл",
    off: "выкл",

    ranges: ["5 мин", "1 ч", "24 ч"],
    export_csv: "Экспорт CSV",
    exported: |path| format!("Сохранено: {path}"),

    check_updates: "Проверять обновления",
    check_updates_info: "Раз в сутки приложение смотрит последний релиз на GitHub и показывает \
                         ссылку, если вышла новая версия. Ничего не скачивается само.",
    update_available: |v| format!("Доступна версия {v}"),

    err_scenario: "Сценарий",
    err_cooler_boost: "Cooler Boost",
    err_backlight: "Подсветка",
    err_webcam: "Веб-камера",
    err_win_fn: "Win/Fn",
    err_usb: "USB-зарядка",
    err_curve: "Кривая вентиляторов",
    err_auto_fan: "Авто-режим вентиляторов",
    err_battery: "Лимит заряда",
    err_autostart: "Автозапуск",
    err_restore: "Восстановление профиля",
    err_touchpad: "Тачпад",
    err_export: "Экспорт",
};

pub static EN: Strings = Strings {
    connecting: "Connecting to EC",
    connecting_hint: "Reading MSI_ACPI…",
    no_access: "No access",
    no_access_title: "Cannot reach the laptop firmware",
    no_access_hint: "Requires an MSI laptop with WMI interface 2.x, running as Administrator.",
    model_fallback: "MSI laptop",
    model_unknown: "Model not recognised",
    model_unknown_info: "This EC firmware is not in the msi-ec model table, so the Cyborg 15 A12VF \
                         registers are used (msi-ec CONF_G2_1). Some settings may not work — run \
                         msi-probe and open a new-model issue.",
    footer: "MSI Center Service may restore its own scenario after reboot or sleep — turn on \
             Restore settings.",
    read_failed: "Failed to read sensors",

    telemetry: "Telemetry",
    telemetry_info: "CPU and GPU temperature over the selected range (up to 24 hours), refreshed \
                     every 1.5 s. The line turns red above 90 °C. Below: fan speed and current \
                     duty. Export CSV saves the whole history as a table.",
    seconds_ago: |s| match s {
        0..90 => format!("{s} s ago"),
        90..5400 => format!("{} min ago", s / 60),
        _ => format!("{} h {} min ago", s / 3600, s % 3600 / 60),
    },

    mode: "Mode",
    mode_info: "MSI Center scenarios. They switch the firmware power profile: how far the CPU \
                and GPU may push clocks and power draw, and how eagerly the fans spin up.",
    scenarios: ["Extreme", "Balanced", "Silent", "Battery"],
    scenario_hints: [
        "Highest power limits. Fastest, but hotter and louder.",
        "Standard power limits for everyday use.",
        "Standard limits with quieter fans.",
        "Reduced power limits for longer battery life.",
    ],
    mode_unknown: "Firmware is in a mode that matches no scenario",

    fans: "Fans",
    fans_info: "Fan speed for each temperature band. Drag the points up or down, then press \
                Apply. The temperature thresholds and the grey point on the right belong to the \
                firmware; the curve cannot go above it. Auto restores the factory curve.",
    fan_readout: |duty, temp| match temp {
        Some(t) => format!("now {duty}% · {t}°"),
        None => format!("now {duty}% · sensor asleep"),
    },
    fan_auto: "Auto",
    fan_custom: "Custom curve",
    unsaved: "Changes not applied",
    reset: "Reset",
    apply: "Apply",
    cooler_boost_hint: "Maximum fan speed, overrides the curve",

    battery: "Battery",
    battery_info: "The laptop stops charging at the chosen level. If it is plugged in most of \
                   the time, 60–80 % noticeably extends battery life.",
    battery_readout: |v| if v >= 100 { "to full".to_owned() } else { format!("to {v}%") },

    system: "System",
    backlight: "Backlight",
    backlight_off: "Off",
    backlight_info: "Keyboard backlight brightness.",
    webcam: "Webcam",
    webcam_info: "Disconnects the camera in hardware: Windows and apps no longer see it.",
    win_fn: "Swap Win and Fn",
    win_fn_info: "Swaps the Win and Fn keys — the same switch as in MSI Center.",
    usb_share: "USB charging when off",
    usb_share_info: "USB ports keep supplying power while the laptop is off or asleep, \
                     so you can charge a phone.",
    language: "Language",

    app: "App",
    autostart: "Start with Windows",
    autostart_info: "Starts minimised to the tray when you sign in. Uses Task Scheduler so it \
                     runs as Administrator without a UAC prompt.",
    close_to_tray: "Close to tray",
    close_to_tray_info: "The close button hides the window to the tray; sensors keep updating. \
                         Quit from the tray icon menu.",
    tray_open: "Open",
    tray_exit: "Quit",
    tray_tooltip: |cpu, gpu| format!("Open MSI Control\nCPU {} · GPU {}", temp(cpu), temp(gpu)),
    open_app: "Open app",

    restore: "Restore settings",
    restore_info: "After boot, wake from sleep and power-source changes the app re-applies your \
                   mode, fan curve, Cooler Boost and charge limit — even if MSI Center Service \
                   overwrote them.",
    per_source: "Separate battery profile",
    per_source_info: "Mode, fans and Cooler Boost are remembered separately on AC and on battery, \
                      and switch automatically with the power source. Changes go to the profile of \
                      the current source.",
    profile_ac: "profile: AC",
    profile_battery: "profile: battery",

    touchpad: "Touchpad",
    touchpad_info: "Turns the touchpad on or off — same as Ctrl+Win+F24 or the keyboard key.",
    fn_lock: "Fn Lock",
    fn_lock_info: "Shows whether the Fn key is locked. Toggle it with Fn+Esc — the firmware does \
                   not let software change it.",
    on: "on",
    off: "off",

    ranges: ["5 min", "1 h", "24 h"],
    export_csv: "Export CSV",
    exported: |path| format!("Saved: {path}"),

    check_updates: "Check for updates",
    check_updates_info: "Once a day the app looks at the latest GitHub release and shows a link if \
                         a newer version exists. Nothing is downloaded automatically.",
    update_available: |v| format!("Version {v} available"),

    err_scenario: "Scenario",
    err_cooler_boost: "Cooler Boost",
    err_backlight: "Backlight",
    err_webcam: "Webcam",
    err_win_fn: "Win/Fn",
    err_usb: "USB charging",
    err_curve: "Fan curve",
    err_auto_fan: "Auto fan mode",
    err_battery: "Charge limit",
    err_autostart: "Start with Windows",
    err_restore: "Restoring profile",
    err_touchpad: "Touchpad",
    err_export: "Export",
};

fn temp(t: Option<u8>) -> String {
    t.map_or("—".to_owned(), |v| format!("{v}°"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_russian_windows_ui_by_primary_language() {
        assert_eq!(Lang::from_windows_ui(0x0419), Lang::Ru); // ru-RU
        assert_eq!(Lang::from_windows_ui(0x0819), Lang::Ru); // ru-MD
        assert_eq!(Lang::from_windows_ui(0x0409), Lang::En); // en-US
        assert_eq!(Lang::from_windows_ui(0x0422), Lang::En); // uk-UA → English fallback
    }
}
