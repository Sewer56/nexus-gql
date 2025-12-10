# batch-downloader-cli

[![CI](https://github.com/Sewer56/nexus-gql/actions/workflows/rust.yml/badge.svg)](https://github.com/Sewer56/nexus-gql/actions)

Command-line interface for batch downloading and analyzing mods from Nexus Mods using the GraphQL API.

> **Note**: This is experimental, mostly LLM-generated code for one-time use. Not recommended for production use.

## Features

- **Mod Size Analysis**: Calculate total size of mods in a category
- **File Type Breakdown**: Detailed statistics for all file types found within archives
- **File Extension Filtering**: Filter mods by file extensions within archives
- **Archive Content Inspection**: Deep inspection of mod archive contents using the legacy REST API
- **Concurrent Processing**: Configurable concurrency for faster analysis
- **Authentication**: Support for authenticated requests via API key
- **Download and Extraction**: Download and extract mod files with filtering support

## Installation

Build from source:

```bash
cargo build --release -p batch-downloader-cli
```

## Usage

### Basic Analysis

Analyze mods without downloading:

```bash
batch-downloader-cli mod-sizes \
    --game skyrimspecialedition \
    --category "Models and Textures" \
    --count 100
```

### Download and Extract

Download and extract all files:

```bash
batch-downloader-cli mod-sizes \
    --game skyrimspecialedition \
    --category "Models and Textures" \
    --count 10 \
    --download \
    --output-path ./downloads \
    --concurrency 6
```

### Filter by Extension

Download only mods containing `.dds` files, extracting only those files:

```bash
batch-downloader-cli mod-sizes \
    --game skyrimspecialedition \
    --category "Models and Textures" \
    --count 10 \
    --download \
    --output-path ./downloads \
    --file-extension dds
```

## Options

| Option                | Description                                                                      |
| --------------------- | -------------------------------------------------------------------------------- |
| `--game`, `-g`        | Game identifier (domain name like `skyrimspecialedition` or game ID like `1704`) |
| `--category`, `-c`    | Category name (e.g., "Models and Textures", "Gameplay", "Weapons")               |
| `--count`, `-n`       | Number of mods to analyze (default: 1000, max: 1000)                             |
| `--main-files-only`   | Only include main files when calculating sizes                                   |
| `--concurrency`, `-j` | Number of concurrent requests (default: 4, max: 20)                              |
| `--api-key`           | API key for authenticated requests (or set `NEXUS_API_KEY` env var)              |
| `--file-extension`    | Filter mods by file extension within archives (e.g., `dds`, `esp`, `esm`)        |
| `--use-risky-method`  | Use faster but potentially less reliable archive inspection method               |
| `--output-path`, `-o` | Output path for downloaded and extracted files                                   |
| `--download`          | Actually download and extract files (default is analyze only)                    |

## Archive Content Inspection

The tool inspects archive contents to provide detailed file type breakdowns:

- Every mod's archive contents are analyzed for comprehensive statistics
- File type breakdown shows total size, count, and average size for each extension
- Processing is slower compared to basic file analysis due to additional API calls

## File Extension Filtering

The `--file-extension` option filters mods based on files contained within their archives:

- Inspects archive contents before downloading using `get_mod_file_contents`
- Returns only mods where at least one archive contains a file with the specified extension
- **When downloading**: Only downloads archives containing the target extension (saves bandwidth)
- Requires an API key for archive content inspection

## Risky Method

The `--use-risky-method` flag enables a faster archive inspection method:

| Aspect      | Standard Method | Risky Method                             |
| ----------- | --------------- | ---------------------------------------- |
| Speed       | Baseline        | ~50% faster                              |
| API Key     | Required        | Not required for content inspection      |
| Reliability | Stable          | May break if Nexus changes URL structure |

The risky method constructs URLs directly from the URI field instead of fetching metadata first. It automatically falls back to the standard method if the URI is empty.

## Sample Output

```text
SUMMARY
=======================================
Game: skyrimspecialedition (ID: 1704)
Category: Models and Textures
Total mods analyzed: 100
File extension filter: .dds
Mods matching filter: 30

ALL MODS STATISTICS
-------------------------------------
Mods with files: 100
Total files: 149
Combined size: 52.4 GiB
Average mod size: 536.3 MiB

FILTERED MODS STATISTICS
-------------------------------------
Filtered mods with files: 30
Filtered total files: 48
Filtered combined size: 16.5 GiB
Filtered average mod size: 562.4 MiB

FILE TYPE BREAKDOWN (Top 10)
============================================================
Extension  Item Count      Total Size      Average Size
------------------------------------------------------------
.dds       17974           64.4 GiB        3.7 MiB
.bsa       68              13.5 GiB        203.3 MiB
.nif       16369           9.6 GiB         611.8 KiB
.tri       3072            1.8 GiB         609.5 KiB
.pdb       69              1.7 GiB         25.8 MiB
```

## Requirements

- **7-zip**: Required for extraction functionality (`7z` command must be in PATH)
- **API Key**: Required for most operations. Get one from [Nexus Mods](https://www.nexusmods.com/users/myaccount?tab=api)

## License

Licensed under [GPL v3 (with Reloaded FAQ)](https://github.com/Sewer56/nexus-gql/blob/main/LICENSE).
