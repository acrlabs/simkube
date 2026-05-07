<!--
template: docs.html
-->

# SimKube Expression Language (SKEL) Reference

This is a quick-reference guide to the SimKube Expression Language (SKEL).  See the [usage instructions](../skel/usage.md)
for more details.  The full SKEL grammar is available [on GitHub](https://github.com/acrlabs/simkube/tree/main/sk-cli/src/skel/skel.pest).

## Transform operations

### Delete

The `delete` operation deletes any (entire) object that matches the selector:

```text
delete(<selector>);
```

Note the difference between `delete` and `remove`, where `remove` only removes fields _within_ a matching resource.

### Modify

The `modify` operation modifies the specified target fields in the matched trace events:

```text
modify(<selectors>, target = value);
```

### Remove

The `remove` operation removes all specified target fields in the matched trace events:

```text
remove(<selectors>, target);
```
