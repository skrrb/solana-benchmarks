#!/bin/bash

tmpfile=$(mktemp)
trap 'rm $tmpfile' EXIT

unbuffer cargo test-sbf 2>&1 | tee $tmpfile

awk '
  /DEBUG.* Program log: Instruction:/ { instruction = $NF }
  /DEBUG.* Program log: #/ {k = 1; split($0, a, "#"); txt = a[2]; next}
  /DEBUG.* Program consumption:/ && k == 1 { comp_budget_before = $6; k = 2; next }
  /DEBUG.* Program consumption:/ && k == 2 { comp_budget_after = $6; k = 0; print instruction, txt, comp_budget_before - comp_budget_after }
' $tmpfile | column -t
