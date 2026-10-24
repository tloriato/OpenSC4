# PR2: shipped buffer-resource inventory

Date: 2026-10-24. Analysis/spec-side evidence for [persistence.md §6.5](../persistence.md#65-shipped-data-inventory-pr2).

## Result and scope

No resource keys of type `0x62B9DA24` occur in the enumerated GOG installer corpus.
Therefore there are no buffer-resource forms to classify under that type here.
This is an inventory observation, not a runtime, pixel-decoding or whole-owner
verification result.

| Files | DBPF files | Indexed entries | Buffer-type entries | PNG-type entries |
|---|---:|---:|---:|---:|
| Root, locale and SKU packages | 22 | 183,205 | 0 | 2,294 |
| `Regions/` shipped cities | 417 | 49,096 | 0 | 0 |
| Identical city copies in `__support/userdocs/SimCity 4/Regions/` | 417 | 49,096 | 0 | 0 |
| `__support/userdocs/SimCity 4/SimCity 4.cfg` | 1 | 2 | 0 | 0 |
| **All package paths** | **857** | **281,399** | **0** | **2,294** |
| **Distinct package contents, by SHA-256** | **440** | **232,303** | **0** | **2,294** |

Buffer type is `0x62B9DA24`; PNG type is `0x856DDBAC`. The latter is a positive
control for type enumeration, not a claim that those pixels were decoded. All
containers have DBPF version 1.0 and index version 7.0. The 22 data packages include
eight root DATs, thirteen locale DATs and the SKU1 Living Mall package.

All 1,097 regular extracted files were inspected by signature, regardless of
extension. There were 240 non-DBPF files, no embedded-package candidates, and no
enumeration, read or parse errors. The DBPF configuration file demonstrates why
enumeration must not depend only on `.dat`/`.sc4` extensions.

## Provenance

The source is Luca's existing GOG offline installer:

- File: `setup_simcity_4_deluxe_edition_1.1.641_hotfix_(25621).exe`.
- Bytes: 1,105,410,240.
- SHA-256: `1966f23a6c7d4faba18d7e09c887c5678192cccedb349e75fa055c63966e6e4d`.
- Extracted `Apps/SimCity 4.exe`, SHA-256:
  `3bc5c7fe807fe5aa24a3abb06e40f13486670bc1150612c09baf4ecedafe1a4b`,
  matching the project's required GOG 1.1.641 executable.
- Extractor: innoextract 1.9, default all-language extraction, no language filter.
- Repository base: `7a010ed17e850437b114b04f56c893c41a26906e`.

The [metadata manifest](persistence-pr2-gog-25621.tsv) has one row per distinct
package content; `identical_copies` lists the mirrored paths (`-` means none). Its SHA-256 with LF
line endings is `755fce8ee0ee781c177d81b386a9ed7235f46dae8324b98527174b7cd2c7e812`.
No package, executable or resource payload is included in this contribution.

## Reproduction and cross-check

Extract the installer to a folder outside the repository, then run from the repo:

```powershell
innoextract --extract --output-dir 'C:\Corpus\GOG25621' 'C:\Installers\setup_simcity_4_deluxe_edition_1.1.641_hotfix_(25621).exe'
python tools/scan_buffer_resources.py C:\Corpus\GOG25621 --output C:\Reports\buffer-corpus.json
```

Quote installer paths when invoking the commands in PowerShell. The output folder
must already exist and must be outside the scanned corpus. Compare package hashes
and counts against the manifest; the JSON report includes each physical copy.
Deduplicate by `sha256` to recover the distinct-content totals above.

The scanner checks container/index versions, index size/count consistency, every
indexed extent and compression-directory record shape. It fails rather than
reporting a complete zero result for missing roots, unreadable files, unsupported
containers, links/reparse points or embedded-package candidates. Its index-7.1
inventory support is a tool extension, not a native compatibility claim
([data-formats.md §2](../data-formats.md#2-dbpf-packages)).

As an independent cross-check, the existing `tools/dbpf.py` `DBPF` reader enumerated
one file for each of the 440 distinct hashes. Its entry totals and counts of
`find(0x62B9DA24)` and `find(0x856DDBAC)` agreed for every file. This comparison did
not require decompressing any target payload.

## Limits

The result covers these shipped packages and cities only, including all mirrored
installer city files and locale packages. It does not establish absence in other
editions, downloads, user-created saves or custom plugins, or prove that no native
caller creates or consumes this resource type. PR3's caller-dependent padded-v1
question remains open.

For future positive matches the scanner classifies target headers after supported
outer DBPF decompression; it does not validate pixels or inner v2 QFS streams.
Unsupported framing, unknown markers or truncated headers make a scan incomplete.
No target payload needed decompression in this particular corpus.
