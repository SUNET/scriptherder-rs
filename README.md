# scriptherder-rs

Rust port of [SUNET/scriptherder](https://github.com/SUNET/scriptherder), a wrapper that keeps track of the status and output of cron jobs and exposes the results as Nagios/NRPE checks.

The port is a single static binary (musl target by default) with no runtime dependencies.

## How it works

1. `wrap` mode runs your script, records start/end time, exit status and output in a data directory.
2. `ls`, `lastlog` and `lastfaillog` modes inspect the recorded data.
3. `check` mode evaluates recorded runs against OK/WARNING criteria and prints a Nagios-style status with the matching exit code.

## Build

```sh
cargo build --release
```

`.cargo/config.toml` sets the target to `x86_64-unknown-linux-musl`. Install the target first with `rustup target add x86_64-unknown-linux-musl`. The binary ends up in `target/x86_64-unknown-linux-musl/release/scriptherder`.

Run the tests with `cargo test`.

## Usage

```
scriptherder [--debug] [-d DATADIR] [--checkdir CHECKDIR] <mode> [args]
```

| Option | Default | Description |
|---|---|---|
| `--debug` | off | Debug output |
| `-d`, `--datadir` | `/var/cache/scriptherder` | Where job data is stored |
| `--checkdir` | `/etc/scriptherder/check` | Where check definitions are read from |

For compatibility with the Python version, a leading `--mode` is accepted and ignored (`scriptherder --mode wrap ...` equals `scriptherder wrap ...`). Running `scriptherder` with no arguments is the same as `scriptherder ls`.

### Modes

| Mode | Description |
|---|---|
| `wrap` | Run a command and record the result |
| `ls [NAME...]` | List recorded jobs |
| `check [NAME...]` | Evaluate checks, Nagios-style output and exit code |
| `lastlog [NAME...]` | Show output of the last run of each job |
| `lastfaillog [NAME...]` | Show output of the last failed run of each job |

### wrap

```
scriptherder wrap -N NAME [--syslog] [--umask 077] [--random-sleep SECONDS] -- COMMAND [ARGS...]
```

| Option | Default | Description |
|---|---|---|
| `-N`, `--name` | required | Job name |
| `--syslog` | off | Also log to syslog |
| `--umask` | `077` | Umask for the data files, 3 digits |
| `--random-sleep` | `0` | Sleep a random time up to this many seconds before running |

Example cron entry:

```
*/15 * * * *   root   scriptherder --mode wrap --syslog --name my_job -- /usr/local/bin/my_job -v
```

### check

```
scriptherder check my_job                    # one job
scriptherder check --exclude my_job other    # catch-all, skipping the named jobs
```

Example NRPE commands:

```
command[check_my_job]=/usr/local/bin/scriptherder --mode check my_job
command[check_scriptherder]=/usr/local/bin/scriptherder --mode check --exclude my_job
```

Output is one of `OK`, `WARNING`, `CRITICAL` or `UNKNOWN` plus a reason.

## Check definitions

A check for job `NAME` lives in `CHECKDIR/NAME.ini`:

```ini
[check]
ok = exit_status=0,max_age=8h
warning = exit_status=0,max_age=24h
```

If `ok` or `warning` is missing from the file, the defaults above are used. A job with no check file at all is reported as `UNKNOWN` ("Failed to load check"). A job matching neither `ok` nor `warning` is `CRITICAL`. A `[DEFAULT]` section and `%(key)s` interpolation are supported.

Criteria are comma-separated. All must hold, except `OR_` criteria: if any `OR_` criterion holds, the whole list passes. Prefix a criterion with `!` to negate it.

| Criterion | Meaning |
|---|---|
| `exit_status=N` | Job exited with status N |
| `max_age=DURATION` | Job finished within DURATION, a number with unit `s`, `m`, `h` or `d`, for example `30m`, `8h`, `2d`. A bare number is seconds |
| `output_contains=TEXT` | Output contains TEXT |
| `output_matches=REGEX` | Output matches REGEX |
| `OR_running` | Job is currently running |
| `OR_file_exists=PATH` | File exists |

Old names `not_running` and `output_not_contains` still work as `!OR_running` and `!output_contains`.

Example, a job that may exit 0 or 2 and must not report errors:

```ini
[check]
ok = exit_status=0,max_age=26h,!output_contains=ERROR
warning = exit_status=2,max_age=26h
```

## Maintenance

Job data is not pruned automatically. Add a cleanup cron entry, for example:

```
0 3 * * *   root   find /var/cache/scriptherder -type f -mtime +7 -print0 | xargs -0 rm -f
```

## Differences from the Python version

- Single static binary, no Python runtime.
- Check files use the `.ini` extension under `--checkdir`.
- Output formats follow the Python version where practical, including Python-style `True`/`False` in messages.
