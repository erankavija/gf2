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
