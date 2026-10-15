# Security

## Reporting a vulnerability

Please report security problems privately through GitHub's
[private vulnerability reporting](https://github.com/Alexander-Aghili/Multi-Agent-Investigative-Systems/security/advisories/new), not in public issues. You will get an
acknowledgement within a few days.

In scope: input that makes a reader hang, crash or read files outside the paths it was given, and
redaction that leaves a command, path or message text in a report.

## Logs and reports

Session logs hold prompts, code, file paths and command output. MAIS reads them locally and sends
nothing anywhere. A report holds commands and file names unless it is made with `--redact`; redact
a report before sharing it outside your team.
