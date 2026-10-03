# Campaign CI check output before and after the root-resolution change

Both checks run from the repository root on the worktree tree.

```
commit=6776fe30ea9af85ad509ddca95926f051421676d
$ python3 dev/scripts/check-campaign-producing-closure.py --self-test
check-campaign-producing-closure: self-test passed
exit=0
$ python3 dev/scripts/check-campaign-producing-closure.py
check-campaign-producing-closure: dev/active/f547c394/producing-inputs.json names all 17 runner sources
exit=0
$ python3 dev/scripts/check-addendum-schema-versions.py --self-test
check-addendum-schema-versions: self-test passed
exit=0
$ python3 dev/scripts/check-addendum-schema-versions.py
check-addendum-schema-versions: every pinned addendum schema digest matches a committed schema file
exit=0
```

## After the change

The changed scripts at the commit below, same tree content otherwise; output is identical.

```
commit=0312f852dd934f3072896d33bb0af8f3a5d89ec3
$ python3 dev/scripts/check-campaign-producing-closure.py --self-test
check-campaign-producing-closure: self-test passed
exit=0
$ python3 dev/scripts/check-campaign-producing-closure.py
check-campaign-producing-closure: dev/active/f547c394/producing-inputs.json names all 17 runner sources
exit=0
$ python3 dev/scripts/check-addendum-schema-versions.py --self-test
check-addendum-schema-versions: self-test passed
exit=0
$ python3 dev/scripts/check-addendum-schema-versions.py
check-addendum-schema-versions: every pinned addendum schema digest matches a committed schema file
exit=0
```
