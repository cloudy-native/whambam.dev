# Markdown job summary for the whambam GitHub Action, from `-o json`.
def r2: . * 100 | round / 100;
def ms: if . >= 1000 then "\(. / 1000 | r2) s" else "\(r2) ms" end;
def fmt($m): if $m == "rps" then "\(r2) req/s"
  elif ($m == "success" or $m == "errors") then "\(r2)%"
  else ms end;

"## whambam: \($url)",
"",
"| | |",
"|---|---|",
"| Requests | \(.summary.requests) in \(.summary.total_secs | r2) s |",
"| Throughput | \(.summary.rps | r2) req/s |",
"| Success | \(.summary.success_rate | r2)% |",
"| Latency p50 / p90 / p99 | \(.latency_ms.percentiles.p50 | ms) / \(.latency_ms.percentiles.p90 | ms) / \(.latency_ms.percentiles.p99 | ms) |",
"| Slowest | \(.latency_ms.max | ms) |",
(if (.errors | length) > 0 then "| Errors | \(.errors | to_entries | map("\(.key): \(.value)") | join(", ")) |" else empty end),
(if .comparison then
  "", "### Compared with baseline", "", "| Metric | Baseline | Current | Change |", "|---|---|---|---|",
  (.comparison[] | .metric as $m | "| \($m) | \(.baseline | fmt($m)) | \(.current | fmt($m)) | \(if .change_pct == null then "n/a" else "\(.change_pct | r2)%" end)\(if .improved then "" else " ⚠️" end) |")
 else empty end),
(if .assertions then
  "", "### Checks", "",
  (.assertions[] | .metric as $m | "- \(if .passed then "✅" else "❌" end) `\(.assertion)`: actual \(.actual | fmt($m)), limit \(.threshold | fmt($m))")
 else empty end)
