#!/usr/bin/env bash
set -euo pipefail

DATASET="/Users/oleksandr/Desktop/Development/BTSL/DATASET"
RESULT_DIR="$HOME/blitz_bench_results_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULT_DIR"

levels=(0 3 7 15 22)
mems=("unl" "350")
codec_threads=("auto" 4 8)
katana_flags=("on" "off")
enc_flags=("plain" "enc")

BIN="$(dirname "$0")/../target/release/blitzarch"

csv_file="$RESULT_DIR/results.csv"
echo "level,memory,codec_threads,katana,enc,create_wall_s,create_cpu_s,ratio,archive_size_mb" > "$csv_file"

echo "Starting benchmark... logs in $RESULT_DIR" | tee "$RESULT_DIR/bench.log"

for lvl in "${levels[@]}"; do
  for mem in "${mems[@]}"; do
    for ct in "${codec_threads[@]}"; do
      for kat in "${katana_flags[@]}"; do
        for enc in "${enc_flags[@]}"; do
          tag="L${lvl}${mem}_ct${ct}_kat${kat}_${enc}"
          out_archive="/tmp/${tag}.blz"
          log_file="$RESULT_DIR/${tag}.log"

          # Build CLI flags
          args=(create "$DATASET" --output "$out_archive" --level "$lvl" --threads 0 --bundle-size 128)

          # katana toggle
          if [[ "$kat" == "off" ]]; then
            args+=(--no-katana)
          fi

          # memory
          if [[ "$mem" == "350" ]]; then
            args+=(--memory-budget 350)
          elif [[ "$mem" == "50%" ]]; then
            args+=(--memory-budget 50%)
          fi

          # codec threads
          if [[ "$ct" != "auto" ]]; then
            args+=(--codec-threads "$ct")
          fi

          # encryption
          if [[ "$enc" == "enc" ]]; then
            args+=(--password test)
          fi

          echo "[RUN] $tag" | tee -a "$RESULT_DIR/bench.log"

          # Measure create time using /usr/bin/time for wall and CPU
          # run and measure time (POSIX-compliant)
          /usr/bin/time -p -o "$log_file.time" "$BIN" "${args[@]}" 2>&1 | tee "$log_file"

          # extract timing
          cwall=$(grep '^real' "$log_file.time" | awk '{print $2}')
          ccpu_user=$(grep '^user' "$log_file.time" | awk '{print $2}')
          ccpu_sys=$(grep '^sys' "$log_file.time" | awk '{print $2}')
          ccpu=$(awk "BEGIN{print $ccpu_user + $ccpu_sys}")

          # Extract ratio and archive size from BlitzArch summary line (assumes pattern "Ratio: X.xX  Archive size: YYY MiB")
          ratio=$(grep -Eo "Ratio:[[:space:]]+[0-9]+\.[0-9x]+" "$log_file" | head -n1 | awk '{print $2}' || true)
          ratio=${ratio:-NA}
          asize=$(grep -Eo "Archive size:[[:space:]]+[0-9]+\.[0-9]+" "$log_file" | head -n1 | awk '{print $3}' || true)
          asize=${asize:-NA}

          echo "$lvl,$mem,$ct,$kat,$enc,$cwall,$ccpu,$ratio,$asize" >> "$csv_file"

          # cleanup
          rm -f "$out_archive"
        done
      done
    done
  done
done

echo "Benchmark finished. CSV at $csv_file" | tee -a "$RESULT_DIR/bench.log"
