# Save-record layout questions

## SR33-2 — Traffic network map persistence (2026-10-28)

`0x6A0F82B2` accounts for 349,525,504 decompressed bytes in the 332 available golden end saves. save-records §3.2 lists the traffic network map, but no persistence layout was found in traffic-network §6 or automata/automata-support. Please specify the version gates, intersection/side records, object-reference lists and final occupant table. The standalone audit leaves this type explicitly unhandled until its owner layout is specified.

## SR33-3 — City object and auxiliary city record (2026-10-28)

The 547-save golden corpus contains 318,073 decompressed bytes for class `0x8990C372` and 14,222 bytes for `0x89D9B9EE / 0x69D9B9F1 / 0`. foundations §3.6 specifies the city's 53 component references and accepted versions 8..12, but does not specify the surrounding city fields or the reference version gates. save-records §2 lists the auxiliary record as not specified. Please supply both wire layouts, including legacy versions, so the standalone archive can decode them without inventing fields. Both remain explicitly unhandled.
