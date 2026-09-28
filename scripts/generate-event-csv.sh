#!/usr/bin/env bash
set -euo pipefail

project_root="$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$project_root"

for profile in \
  small-deposits \
  mixed-short \
  withdrawal-heavy \
  dispute-lifecycle \
  tiny-amounts \
  many-clients-long; do
  manifest="samples/generated/$profile.manifest"
  recorded_profile="$(awk -F= '$1 == "profile" { print $2 }' "$manifest")"
  seed="$(awk -F= '$1 == "seed" { print $2 }' "$manifest")"
  if [[ "$recorded_profile" != "$profile" || -z "$seed" ]]; then
    echo "invalid manifest: $manifest" >&2
    exit 1
  fi
  PAYMENT_EXPORT_PROFILE="$profile" \
    PAYMENT_EXPORT_OUTPUT="samples/generated/$profile.csv" \
    PAYMENT_EXPORT_SEED="$seed" \
    cargo test --quiet --lib payment_processor::property_tests::export_generated_case \
      -- --ignored --exact --nocapture
done
