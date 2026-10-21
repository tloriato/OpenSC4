//! Day and month name tables of the language utility (language-utility.md §5).
//!
//! Entries are byte strings returned exactly as stored: several are not valid
//! UTF-8 (raw cp1252/cp1250/cp1251/cp874 bytes) and are kept that way
//! (strings-language.md §2). Original typos are kept too.
//!
//! An empty entry means "(none)" in the spec: the getter leaves `out` alone.

/// One table row: the language IDs sharing it, then the names for index 1..N.
pub(crate) struct NameRow {
    pub ids: &'static [u32],
    pub names: &'static [&'static [u8]],
}

const fn s(x: &'static str) -> &'static [u8] {
    x.as_bytes()
}

const ENGLISH_DAYS: [&[u8]; 7] = [
    b"Monday",
    b"Tuesday",
    b"Wednesday",
    b"Thursday",
    b"Friday",
    b"Saturday",
    b"Sunday",
];
const JA_DAYS: [&[u8]; 7] = [
    s("月曜日"),
    s("火曜日"),
    s("水曜日"),
    s("木曜日"),
    s("金曜日"),
    s("土曜日"),
    s("日曜日"),
];
const ZH_DAYS: [&[u8]; 7] = [
    s("星期一"),
    s("星期二"),
    s("星期三"),
    s("星期四"),
    s("星期五"),
    s("星期六"),
    s("星期天"),
];
const KO_DAYS: [&[u8]; 7] = [
    s("월요일"),
    s("화요일"),
    s("수요일"),
    s("목요일"),
    s("금요일"),
    s("토요일"),
    s("일요일"),
];

/// §5.1 `GetDayName`.
pub(crate) static DAY_NAMES: &[NameRow] = &[
    NameRow {
        ids: &[0x01, 0x02, 0x09, 0x0C, 0x0D, 0x10, 0x13],
        names: &ENGLISH_DAYS,
    },
    NameRow {
        ids: &[0x03],
        names: &[
            b"Lundi",
            b"Mardi",
            b"Mercredi",
            b"Jeudi",
            b"Vendredi",
            b"Samedi",
            b"Dimanche",
        ],
    },
    NameRow {
        ids: &[0x04],
        names: &[
            b"Montag",
            b"Dienstag",
            b"Mittwoch",
            b"Donnerstag",
            b"Freitag",
            b"Samstag",
            b"Sonntag",
        ],
    },
    NameRow {
        ids: &[0x05],
        names: &[
            s("Lunedì"),
            s("Martedì"),
            s("Mercoledì"),
            s("Giovedì"),
            s("Venerdì"),
            b"Sabato",
            b"Domenica",
        ],
    },
    NameRow {
        ids: &[0x06],
        names: &[
            b"Lunes",
            b"Martes",
            s("Miércoles"),
            b"Jueves",
            b"Viernes",
            s("Sábado"),
            b"Domingo",
        ],
    },
    NameRow {
        ids: &[0x07],
        names: &[
            b"Maandag",
            b"Dinsdag",
            b"Woensdag",
            b"Donderdag",
            b"Vrijdag",
            b"Zaterdag",
            b"Zondag",
        ],
    },
    NameRow {
        ids: &[0x08, 0x0A],
        names: &[
            b"Mandag",
            b"Tirsdag",
            b"Onsdag",
            b"Torsdag",
            b"Fredag",
            s("Lørdag"),
            s("Søndag"),
        ],
    },
    NameRow {
        ids: &[0x0B],
        names: &[
            b"Maanantai",
            b"Tiistai",
            b"Keskiviikko",
            b"Torstai",
            b"Perjantai",
            b"Lauantai",
            b"Sunnuntai",
        ],
    },
    NameRow {
        ids: &[0x0E, 0x23],
        names: &[
            b"Segunda-feira",
            b"Ter\xe7a-feira",
            b"Quarta-feira",
            b"Quinta-feira",
            b"Sexta-feira",
            b"S\xe1bado",
            b"Domingo",
        ],
    },
    NameRow {
        ids: &[0x0F],
        names: &JA_DAYS,
    },
    NameRow {
        ids: &[0x11, 0x12],
        names: &ZH_DAYS,
    },
    NameRow {
        ids: &[0x14],
        names: &KO_DAYS,
    },
];

/// §5.2 `GetAbbrDayName`.
pub(crate) static ABBR_DAY_NAMES: &[NameRow] = &[
    NameRow {
        ids: &[0x01, 0x02],
        names: &[b"Mon", b"Tue", b"Wed", b"Thur", b"Fri", b"Sat", b"Sun"],
    },
    NameRow {
        ids: &[0x03],
        names: &[b"lun", b"mar", b"mer", b"jeu", b"ven", b"sam", b"dim"],
    },
    NameRow {
        ids: &[0x04],
        names: &[b"Mo", b"Di", b"Mi", b"Do", b"Fr", b"Sa", b"So"],
    },
    NameRow {
        ids: &[0x05],
        names: &[b"Lun", b"Mar", b"Mer", b"Gio", b"Ven", b"Sab", b"Dom"],
    },
    NameRow {
        ids: &[0x06],
        names: &[b"L", b"M", b"X", b"J", b"V", b"S", b"D"],
    },
    NameRow {
        ids: &[0x07],
        names: &[b"Ma", b"Di", b"Wo", b"Do", b"Vr", b"Za", b"Zo"],
    },
    NameRow {
        ids: &[0x09],
        names: &[b"M\xe5\x06E", b"Tis", b"Ons", b"Tor", b"Fre", b"L\xf6r", b"S\xf6n"],
    },
    NameRow {
        ids: &[0x0B],
        names: &[b"ma", b"ti", b"ke", b"to", b"pe", b"la", b"su"],
    },
    NameRow {
        ids: &[0x0D],
        names: &[
            b"\xcf\xed",
            b"\xc4\xf2",
            b"\xd1\xf0",
            b"\xd7\xf2",
            b"\xcf\xf2",
            b"\xd1\xe1",
            b"\xc4\xf1",
        ],
    },
    NameRow {
        ids: &[0x0E, 0x23],
        names: &[b"Seg", b"Ter", b"Qua", b"Qui", b"Sex", b"Sab", b"Dom"],
    },
    NameRow {
        ids: &[0x0F],
        names: &JA_DAYS,
    },
    NameRow {
        ids: &[0x10],
        names: &[b"Pn", b"Wt", b"\xc2\x8cr", b"Cz", b"Pt", b"So", b"N"],
    },
    NameRow {
        ids: &[0x11, 0x12],
        names: &ZH_DAYS,
    },
    NameRow {
        ids: &[0x13],
        names: &[b"\xa8", b"\xcd", b"\xbe", b"\xbe\xc4", b"\xc9", b"\xca", b"\xcd\xd2"],
    },
    NameRow {
        ids: &[0x14],
        names: &KO_DAYS,
    },
];

const ZH_MONTHS: [&[u8]; 12] = [
    s("1月"),
    s("2月"),
    s("3月"),
    s("4月"),
    s("5月"),
    s("6月"),
    s("7月"),
    s("8月"),
    s("9月"),
    s("10月"),
    s("11月"),
    s("12月"),
];
const KO_MONTHS: [&[u8]; 12] = [
    s("1월"),
    s("이월"),
    s("3월"),
    s("4월"),
    s("오월"),
    s("유월"),
    s("칠월"),
    s("팔월"),
    s("9월"),
    s("10월"),
    s("11월"),
    s("십이월"),
];

/// §5.3 `GetMonthName`.
pub(crate) static MONTH_NAMES: &[NameRow] = &[
    NameRow {
        ids: &[0x01, 0x02, 0x0C, 0x0D, 0x13],
        names: &[
            b"January",
            b"February",
            b"March",
            b"April",
            b"May",
            b"June",
            b"July",
            b"August",
            b"September",
            b"October",
            b"November",
            b"December",
        ],
    },
    NameRow {
        ids: &[0x03],
        names: &[
            b"janvier",
            b"F\xe9vrier",
            b"mars",
            b"avril",
            b"mai",
            b"juin",
            b"juillet",
            b"Ao\xfbt",
            b"septembre",
            b"octobre",
            b"novembre",
            b"D\xe9cembre",
        ],
    },
    NameRow {
        ids: &[0x04],
        names: &[
            b"Januar",
            b"Februar",
            b"M\xe4rz",
            b"April",
            b"Mai",
            b"Juni",
            b"Juli",
            b"August",
            b"September",
            b"Oktober",
            b"November",
            b"Dezember",
        ],
    },
    NameRow {
        ids: &[0x05],
        names: &[
            b"Gennaio",
            b"Febbraio",
            b"Marzo",
            b"Aprile",
            b"Maggio",
            b"Giugno",
            b"Luglio",
            b"Agosto",
            b"Settembre",
            b"Ottobre",
            b"Novembre",
            b"Dicembre",
        ],
    },
    NameRow {
        ids: &[0x06],
        names: &[
            b"Enero",
            b"Febrero",
            b"Marzo",
            b"Abril",
            b"Mayo",
            b"Junio",
            b"Julio",
            b"Agosto",
            b"Septiembre",
            b"Octubre",
            b"Noviembre",
            b"Diciembre",
        ],
    },
    NameRow {
        ids: &[0x07],
        names: &[
            b"Januari",
            b"Februari",
            b"Maart",
            b"April",
            b"Mei",
            b"Juni",
            b"Juli",
            b"Augustus",
            b"September",
            b"Oktober",
            b"November",
            b"December",
        ],
    },
    NameRow {
        ids: &[0x08],
        names: &[
            b"januar",
            b"februar",
            b"marts",
            b"april",
            b"maj",
            b"juni",
            b"juli",
            b"august",
            b"september",
            b"oktober",
            b"november",
            b"december",
        ],
    },
    NameRow {
        ids: &[0x09],
        names: &[
            b"Januari",
            b"Februari",
            b"Mars",
            b"April",
            b"Maj",
            b"Juni",
            b"Juli",
            b"Augusti",
            b"September",
            b"Oktober",
            b"November",
            b"December",
        ],
    },
    NameRow {
        ids: &[0x0B],
        names: &[
            b"tamikuu",
            b"helmikuu",
            b"maaliskuu",
            b"huhtikuu",
            b"toukokuu",
            b"kes\xe4kuu",
            b"hein\xe4kuu",
            b"elokuu",
            b"syyskuu",
            b"lokakuu",
            b"marraskuu",
            b"joulukuu",
        ],
    },
    NameRow {
        ids: &[0x0E, 0x23],
        names: &[
            b"Janeiro",
            b"Fevereiro",
            b"Mar\xe7o",
            b"Abril",
            b"Maio",
            b"Junho",
            b"Julho",
            b"Agosto",
            b"Setembro",
            b"Outubro",
            b"Novembro",
            b"Dezembro",
        ],
    },
    NameRow {
        ids: &[0x0F],
        // February is "(none)" in the stored table.
        names: &[
            s("1 月"),
            b"",
            s("3 月"),
            s("4 月"),
            s("5 月"),
            s("6 月"),
            s("7 月"),
            s("8 月"),
            s("9 月"),
            s("10 月"),
            s("11 月"),
            s("12 月"),
        ],
    },
    NameRow {
        ids: &[0x10],
        names: &[
            b"Stycze\xf1",
            b"Luty",
            b"Marzec",
            b"Kwiecie\xf1",
            b"Maj",
            b"Czerwiec",
            b"Lipiec",
            b"Sierpie\xf1",
            b"Wrzesie\xf1",
            b"Pa\xc2\x9fdziernik",
            b"Listopad",
            b"Grudzie\xf1",
        ],
    },
    NameRow {
        ids: &[0x11, 0x12],
        names: &ZH_MONTHS,
    },
    NameRow {
        ids: &[0x14],
        names: &KO_MONTHS,
    },
];

/// §5.4 `GetAbbrMonthName`.
pub(crate) static ABBR_MONTH_NAMES: &[NameRow] = &[
    NameRow {
        ids: &[0x01, 0x02, 0x08, 0x0B, 0x0C, 0x0D, 0x13],
        names: &[
            b"Jan", b"Feb", b"Mar", b"Apr", b"May", b"Jun", b"Jul", b"Aug", b"Sep", b"Oct", b"Nov", b"Dec",
        ],
    },
    NameRow {
        ids: &[0x03],
        names: &[
            b"jan", b"F\xe9v", b"mar", b"Avr", b"mai", b"juin", b"juil", b"Ao\xfbt", b"sept", b"oct", b"nov", b"D\xe9c",
        ],
    },
    NameRow {
        ids: &[0x04],
        names: &[
            b"Jan", b"Feb", b"M\xe4r", b"Apr", b"Mai", b"Jun", b"Jul", b"Aug", b"Sep", b"Okt", b"Nov", b"Dez",
        ],
    },
    NameRow {
        ids: &[0x05],
        names: &[
            b"Gen", b"Feb", b"Mar", b"Apr", b"Mag", b"Giu", b"Lug", b"Ago", b"Set", b"Ott", b"Nov", b"Dic",
        ],
    },
    NameRow {
        ids: &[0x06],
        names: &[
            b"En", b"Feb", b"Mar", b"Abr", b"Mayo", b"Jun", b"Jul", b"Ag", b"Sep", b"Oct", b"Nov", b"Dic",
        ],
    },
    NameRow {
        ids: &[0x07],
        names: &[
            b"Jan", b"Feb", b"Mrt", b"Apr", b"Mei", b"Jun", b"Jul", b"Aug", b"Sep", b"Okt", b"Nov", b"Dec",
        ],
    },
    NameRow {
        ids: &[0x09],
        names: &[
            b"Jan", b"Feb", b"Mar", b"Apr", b"Maj", b"Jun", b"Jul", b"Aug", b"Sep", b"Okt", b"Nov", b"Dec",
        ],
    },
    NameRow {
        ids: &[0x0E, 0x23],
        names: &[
            b"Jan", b"Fev", b"Mar", b"Abr", b"Mai", b"Jun", b"Jul", b"Ago", b"Set", b"Out", b"Nov", b"Dez",
        ],
    },
    NameRow {
        ids: &[0x0F],
        names: &[
            s("1 月"),
            s("2 月"),
            s("3 月"),
            s("4 月"),
            s("5 月"),
            s("6 月"),
            s("7 月"),
            s("8 月"),
            s("9 月"),
            s("10 月"),
            s("11 月"),
            s("12 月"),
        ],
    },
    NameRow {
        ids: &[0x10],
        names: &[
            b"Sty", b"Lut", b"Mar", b"Kwi", b"Maj", b"Cze", b"Lip", b"Sie", b"Wrz", b"Pa\x9f", b"Lis", b"Gru",
        ],
    },
    NameRow {
        ids: &[0x11, 0x12],
        names: &ZH_MONTHS,
    },
    NameRow {
        ids: &[0x14],
        names: &KO_MONTHS,
    },
];

/// The entry for (language, index), or `None` when the pair has none.
/// Only the low byte of `index` is read (strings-language.md §3).
pub(crate) fn lookup(table: &[NameRow], language: u32, index: u32) -> Option<&'static [u8]> {
    let i = (index & 0xFF) as usize;
    if i == 0 {
        return None;
    }
    let row = table.iter().find(|r| r.ids.contains(&language))?;
    let name = *row.names.get(i - 1)?;
    if name.is_empty() {
        None
    } else {
        Some(name)
    }
}
