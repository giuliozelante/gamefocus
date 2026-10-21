# End session: Boost rollback before resume

```text
Goal:
  end_session rolls Boost back before resuming suspended processes.
  soft_stop entries are not resumed.

In:
  Order flip in end_session. Two core tests.

Out:
  New providers.

Done when:
  cargo test -p gamefocus-core --lib is green.
```
