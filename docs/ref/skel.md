<!--
template: docs.html
-->

# SimKube Expression Language (SKEL) Reference

This is a quick-reference guide to the SimKube Expression Language (SKEL).  See the [usage instructions](../skel/usage.md)
for more details.  The full SKEL grammar is available [on GitHub](https://github.com/acrlabs/simkube/tree/main/sk-cli/src/skel/skel.pest).

# Transform operations

## Remove

The `remove` operation removes all specified target fields in the matched trace events:

```
remove(<selectors>, target);
```
