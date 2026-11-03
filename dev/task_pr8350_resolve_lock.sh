#!/usr/bin/env bash
set -euo pipefail
export CARGO_BUILD_JOBS=4
export PATH="$HOME/.cargo/bin:$PATH"
task_log=/home/xuanwo/Code/logs/opendal-pr8350-lock-main263-20261103
mkdir -p "$task_log"
exec 9>/tmp/opendal-pr-delivery-20261031-build.lock
flock 9
trap 'task_exit=$?; printf "%s\n" "$task_exit" > "$task_log/driver.exit"' EXIT
[[ $(git rev-parse HEAD) == 9746ab2dfc245a0605458a2369c2022539c832f6 ]]
[[ -z $(git status --porcelain --untracked-files=all) ]]
git show 26387047f5187fe7ea2fc6a0d8400c65e74dc0b3:core/Cargo.lock > core/Cargo.lock
cargo metadata --offline --manifest-path core/Cargo.toml --format-version 1 > "$task_log/metadata.json" 2> "$task_log/metadata.log"
python3 - <<'CHECK'
import json,subprocess,tomllib
from pathlib import Path
refs=['3cbb0b1a5dbee25fe859efcbec31ef65600a7cbd','26387047f5187fe7ea2fc6a0d8400c65e74dc0b3']
locks=[tomllib.loads(subprocess.check_output(['git','show',r+':core/Cargo.lock'],text=True)) for r in refs]
key=lambda p:(p['name'],p['version'],p.get('source',''))
known={key(p):p for d in locks for p in d['package']}
new=tomllib.loads(Path('core/Cargo.lock').read_text())
assert all(key(p) in known for p in new['package']), 'Resolver introduced a package version outside both accepted parents'
assert all(p.get('checksum')==known[key(p)].get('checksum') for p in new['package'])
assert [p['version'] for p in new['package'] if p['name']=='smb']==['0.12.1']
report={'parents':refs,'packages':len(new['package']),'introduced_versions':[], 'smb_version':'0.12.1'}
Path('/home/xuanwo/Code/logs/opendal-pr8350-lock-main263-20261103/result.json').write_text(json.dumps(report,indent=2)+'\n')
CHECK
git diff --check > "$task_log/diff-check.log" 2>&1
git diff --stat > "$task_log/diff-stat.log"
git switch -c xuanwo/pr-8350-main263-lock-result
git add core/Cargo.lock
git commit -m "build: reconcile Git and SMB locked dependencies" > "$task_log/commit.log" 2>&1
git show --no-patch --format='%H %T %P' HEAD > "$task_log/result-head.txt"
git push origin HEAD:refs/heads/xuanwo/pr-8350-main263-lock-result > "$task_log/push.log" 2>&1
